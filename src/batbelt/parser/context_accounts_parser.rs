use crate::batbelt::metadata::{BatMetadata, MetadataId};
use once_cell::sync::Lazy;
use error_stack::{IntoReport, Report, Result, ResultExt};
use lazy_regex::regex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::io::split;

use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::sonar::{SonarResult, SonarResultType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CAAccountAttributeInfo {
    pub is_pda: bool,
    pub is_init: bool,
    pub is_mut: bool,
    pub is_close: bool,
    pub rent_exemption_account: String,
    pub seeds: Vec<String>,
    pub validations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CAAccountTypeInfo {
    pub content: String,
    pub solana_account_type: SolanaAccountType,
    pub account_struct_name: String,
    pub account_wrapper_name: String,
    pub lifetime_name: String,
    pub account_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CAAccountParser {
    pub content: String,
    pub solana_account_type: SolanaAccountType,
    pub account_struct_name: String,
    pub account_wrapper_name: String,
    pub lifetime_name: String,
    pub account_name: String,
    pub is_pda: bool,
    pub is_init: bool,
    pub is_mut: bool,
    pub is_close: bool,
    pub seeds: Vec<String>,
    pub rent_exemption_account: String,
    pub validations: Vec<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub token_mint: Option<String>,
    #[serde(default)]
    pub space: Option<String>,
    #[serde(default)]
    pub rent_exempt: bool,
    #[serde(default)]
    pub realloc: Option<String>,
    #[serde(default)]
    pub bump: Option<String>,
}

impl CAAccountParser {
    fn new(
        acc_type_info: CAAccountTypeInfo,
        acc_attribute: CAAccountAttributeInfo,
        content: &str,
    ) -> Self {
        Self {
            content: content.to_string(),
            solana_account_type: acc_type_info.solana_account_type,
            account_struct_name: acc_type_info.account_struct_name,
            account_wrapper_name: acc_type_info.account_wrapper_name,
            lifetime_name: acc_type_info.lifetime_name,
            account_name: acc_type_info.account_name,
            is_pda: acc_attribute.is_pda,
            is_init: acc_attribute.is_init,
            is_mut: acc_attribute.is_mut,
            is_close: acc_attribute.is_close,
            seeds: acc_attribute.seeds,
            rent_exemption_account: acc_attribute.rent_exemption_account,
            validations: acc_attribute.validations,
            owner: None,
            token_mint: None,
            space: None,
            rent_exempt: false,
            realloc: None,
            bump: None,
        }
    }

    pub fn new_from_context_account_content(
        context_account_content: &str,
    ) -> Result<Self, ParserError> {
        if !Self::get_context_account_lazy_regex().is_match(context_account_content) {
            return Err(Report::new(ParserError).attach_printable(format!(
                "Incorrect context account content\n{context_account_content}"
            )));
        }
        let account_attribute_info = Self::get_account_attribute_info(context_account_content)?;
        let account_type_info = Self::get_account_type_info(context_account_content)?;
        let new_parser = Self::new(
            account_type_info,
            account_attribute_info,
            &context_account_content,
        );
        Ok(new_parser)
    }

    pub fn get_context_account_lazy_regex<'a>() -> &'a Lazy<Regex, fn() -> Regex> {
        regex!(
            r#"([ ]+#\[account\([\s\w,()?.= @:><!&{};\*\[\]+|]+\)\][\s]*)?[ ]+pub [\w]+: (\w+<)*([\w ,']+)(>)*"#
        )
    }

    pub fn get_account_type_info(
        context_account_content: &str,
    ) -> Result<CAAccountTypeInfo, ParserError> {
        let mut last_line = context_account_content
            .lines()
            .last()
            .unwrap()
            .trim()
            .trim_end_matches(',')
            .to_string();

        let account_name = last_line
            .trim()
            .trim_start_matches("pub ")
            .split(":")
            .next()
            .ok_or(ParserError)
            .into_report()?
            .to_string();

        let mut account_type_info = CAAccountTypeInfo {
            content: context_account_content.to_string(),
            solana_account_type: SolanaAccountType::from_context_account_content(
                context_account_content,
            )?,
            account_struct_name: "".to_string(),
            account_wrapper_name: "".to_string(),
            lifetime_name: "".to_string(),
            account_name: account_name.clone(),
        };

        account_type_info.account_wrapper_name = last_line
            .trim_start_matches(&format!("pub {}: ", account_name.clone()))
            .trim_start_matches("Box<")
            .split('<')
            .next()
            .unwrap()
            .to_string();

        let wrapper_content_regex = regex!(r"<[\w',_ ]+>");
        let wrapper_content = wrapper_content_regex
            .find(&last_line)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .trim_start_matches("<")
            .trim_end_matches(">")
            .to_string();

        let (lifetime_name, account_struct_name) = if wrapper_content.contains(',') {
            let results = wrapper_content
                .split(',')
                .filter_map(|w_content| {
                    if w_content != "'_" {
                        Some(w_content.trim().to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            (results[0].clone(), results[1].clone())
        } else {
            // is is not comma separated, then the only content is the lifetime
            (
                wrapper_content.to_string(),
                account_type_info.account_wrapper_name.clone(),
            )
        };
        account_type_info.lifetime_name = lifetime_name;
        account_type_info.account_struct_name = account_struct_name;
        Ok(account_type_info)
    }

    pub fn get_account_attribute_info(
        context_account_content: &str,
    ) -> Result<CAAccountAttributeInfo, ParserError> {
        let mut account_info = CAAccountAttributeInfo {
            is_pda: false,
            is_init: false,
            is_mut: false,
            is_close: false,
            rent_exemption_account: "".to_string(),
            seeds: vec![],
            validations: vec![],
        };
        if !context_account_content.contains("#[account(") {
            return Ok(account_info);
        }
        account_info.seeds = Self::get_seeds(context_account_content)?;
        account_info.is_pda = !account_info.seeds.is_empty();
        account_info.is_mut = Self::get_is_mut(context_account_content)?;
        account_info.is_init = Self::get_is_init(context_account_content)?;
        account_info.is_close = Self::get_is_close(context_account_content)?;
        account_info.rent_exemption_account =
            Self::get_rent_exemption_account(context_account_content)?;
        account_info.validations = Self::get_validations(context_account_content)?;

        Ok(account_info)
    }

    fn get_is_close(sonar_result_content: &str) -> ParserResult<bool> {
        let close_regex = regex!(r"(close = [\w_:]+)");
        Ok(close_regex.is_match(sonar_result_content))
    }

    fn get_is_mut(sonar_result_content: &str) -> ParserResult<bool> {
        let mut_regex_1 = regex!(r#"\(mut,"#);
        let mut_regex_2 = regex!(r#"\s+mut,"#);
        let mut_regex_3 = regex!(r#"\(mut\)"#);
        Ok(mut_regex_1.is_match(sonar_result_content)
            || mut_regex_2.is_match(sonar_result_content)
            || mut_regex_3.is_match(sonar_result_content))
    }

    fn get_seeds(sonar_result_content: &str) -> Result<Vec<String>, ParserError> {
        let seeds_array_regex = regex!(r"seeds = \[\s?[\w()._?,&:\s]+\s?\]");
        let seeds_separator_regex = regex!(r"\s*[\w()._?&:]+");
        if !seeds_array_regex.is_match(sonar_result_content) {
            return Ok(vec![]);
        };
        let seeds_array = seeds_array_regex
            .find(sonar_result_content)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .replace("seeds = ", "")
            .to_string();
        let seeds = seeds_separator_regex
            .find_iter(&seeds_array)
            .map(|seed| seed.as_str().trim().to_string())
            .collect::<Vec<_>>();
        Ok(seeds)
    }

    fn get_is_init(sonar_result_content: &str) -> ParserResult<bool> {
        let init_regex = regex!(r#"\(?\s?init(_if_necessary)?,"#);
        Ok(init_regex.is_match(sonar_result_content))
    }

    fn get_validations(sonar_result_content: &str) -> ParserResult<Vec<String>> {
        // let validation_regex = Regex::new(r"constraint = [\sA-Za-z0-9()?._= @:><!&{}*]+[,\n]?")
        let validation_regex =
            regex!(r#"(constraint|has_one|address) = [\w()?.= @:><!&{}\*\s;|]+\n?"#);
        if validation_regex.is_match(sonar_result_content) {
            let matches = validation_regex
                .find_iter(sonar_result_content)
                .map(|reg_match| reg_match.as_str().trim_end_matches(')').trim().to_string())
                .collect::<Vec<_>>();
            log::debug!("validation_matches:\n{matches:#?}");
            return Ok(matches);
        }
        Ok(vec![])
    }

    fn get_rent_exemption_account(sonar_result_content: &str) -> ParserResult<String> {
        let rent_exemption_payer_regex = regex!(r#"payer = [A-Za-z0-9_.]+"#);
        if rent_exemption_payer_regex.is_match(sonar_result_content) {
            let payer_match = rent_exemption_payer_regex
                .find(sonar_result_content)
                .unwrap()
                .as_str()
                .trim()
                .to_string();
            return Ok(payer_match.split(" = ").last().unwrap().to_string());
        }

        let rent_exemption_close_regex = regex!(r#"close = [A-Za-z0-9_.]+"#);

        if rent_exemption_close_regex.is_match(sonar_result_content.clone()) {
            let close_match = rent_exemption_close_regex
                .find(sonar_result_content)
                .unwrap()
                .as_str()
                .trim()
                .to_string();
            return Ok(close_match.split(" = ").last().unwrap().to_string());
        }

        Ok("".to_string())
    }
}


// #[test]
// fn test_get_account_type_info_struct_name() {
//     let mut sonar_result = SonarResult {
//         name: "account_test".to_string(),
//         content: "".to_string(),
//         trailing_whitespaces: 0,
//         result_type: SonarResultType::ContextAccountsAll,
//         start_line_index: 0,
//         end_line_index: 0,
//         is_public: false,
//     };
//
//     sonar_result.content = "pub account_test: Signer<'info>,".to_string();
//     let result_1 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
//     assert_eq!(
//         result_1.account_struct_name, "Signer",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_1.account_wrapper_name, "Signer",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_1.lifetime_name, "'info", "incorrect lifetime_name");
//
//     sonar_result.content = "pub account_test: UncheckedAccount<'info>,".to_string();
//     let result_2 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
//     assert_eq!(
//         result_2.account_struct_name, "UncheckedAccount",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_2.account_wrapper_name, "UncheckedAccount",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_2.lifetime_name, "'info", "incorrect lifetime_name");
//
//     sonar_result.content = "pub account_test: AccountLoader<'info, OwnedAccount1>,".to_string();
//     let result_3 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
//     assert_eq!(
//         result_3.account_struct_name, "OwnedAccount1",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_3.account_wrapper_name, "AccountLoader",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_3.lifetime_name, "'info", "incorrect lifetime_name");
//
//     sonar_result.content = "pub account_test: Account<'info, OwnedAccount2>,".to_string();
//     let result_4 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
//     assert_eq!(
//         result_4.account_struct_name, "OwnedAccount2",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_4.account_wrapper_name, "Account",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_4.lifetime_name, "'info", "incorrect lifetime_name");
//
//     sonar_result.content = "pub account_test: OwnedAccount3<'info>,".to_string();
//     let result_5 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
//     assert_eq!(
//         result_5.account_struct_name, "OwnedAccount3",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_5.account_wrapper_name, "OwnedAccount3",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_5.lifetime_name, "'info", "incorrect lifetime_name");
//
//     sonar_result.content = "pub account_test: AccountLoader<'info, Mint>,".to_string();
//     let result_6 = CAAccountParser::get_account_type_info(sonar_result).unwrap();
//     assert_eq!(
//         result_6.account_struct_name, "Mint",
//         "incorrect account_struct_name"
//     );
//     assert_eq!(
//         result_6.account_wrapper_name, "AccountLoader",
//         "incorrect account_wrapper_name"
//     );
//     assert_eq!(result_6.lifetime_name, "'info", "incorrect lifetime_name");
// }
//
// #[test]
//
// fn test_get_seeds() {
//     let seeds_array_regex = Regex::new(r"seeds = \[\s?[\w()._?,&:\s]+\s?\]").unwrap();
//     let seeds_separator_regex = Regex::new(r"\s*[\w()._?&:]+").unwrap();
//     let test_text_1 = "    #[account(
//         mut,
//         close = funds_to,
//         has_one = fleet_ships,
//         seeds = [
//             DISBANDED_FLEET,
//             disbanded_fleet.load()?.game_id.as_ref(),
//             disbanded_fleet.load()?.owner_profile.as_ref(),
//             &disbanded_fleet.load()?.fleet_label,
//         ],
//         bump = disbanded_fleet.load()?.bump,
//     )]";
//     let is_seeds_match = seeds_array_regex.is_match(test_text_1);
//     assert!(is_seeds_match);
//     let seeds_array = seeds_array_regex
//         .find(test_text_1)
//         .unwrap()
//         .as_str()
//         .replace("seeds = ", "")
//         .to_string();
//     println!("{seeds_array}");
//     let seeds = seeds_separator_regex
//         .find_iter(&seeds_array)
//         .map(|seed| seed.as_str().trim().to_string())
//         .collect::<Vec<_>>();
//     println!("{seeds:?}");
// }
