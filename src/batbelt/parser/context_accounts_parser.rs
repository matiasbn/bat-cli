use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::{SonarResult, SonarResultType};
use error_stack::{Report, Result, ResultExt};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct CAAccountAttributeInfo {
    pub content: String,
    pub is_pda: bool,
    pub is_init: bool,
    pub is_mut: bool,
    pub seeds: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct CAAccountTypeInfo {
    pub content: String,
    pub solana_account_type: SolanaAccountType,
    pub account_struct_name: String,
    pub account_wrapper_name: String,
    pub lifetime_name: String,
}

#[derive(Debug, Clone)]
pub struct CAAccountParser {
    pub name: String,
    pub account_attribute_info: CAAccountAttributeInfo,
    pub account_type_info: CAAccountTypeInfo,
}

impl CAAccountParser {
    fn new(
        name: String,
        account_attribute_info: CAAccountAttributeInfo,
        account_type_info: CAAccountTypeInfo,
    ) -> Self {
        Self {
            name,
            account_attribute_info,
            account_type_info,
        }
    }

    pub fn new_from_sonar_result(sonar_result: SonarResult) -> Result<Self, ParserError> {
        if !sonar_result
            .result_type
            .is_context_accounts_sonar_result_type()
        {
            return Err(Report::new(ParserError).attach_printable(format!(
                "Incorrect SonarResultType. \n expected {:#?} \n received: {}",
                SonarResultType::ContextAccountsAll.get_context_accounts_sonar_result_types(),
                sonar_result.result_type.to_string()
            )));
        }
        let account_attribute_info = Self::get_account_attribute_info(&sonar_result.content)?;
        let account_type_info = Self::get_account_type_info(sonar_result.clone())?;
        let new_parser = Self::new(sonar_result.name, account_attribute_info, account_type_info);
        Ok(new_parser)
    }

    pub fn get_account_type_info(
        sonar_result: SonarResult,
    ) -> Result<CAAccountTypeInfo, ParserError> {
        let last_line = sonar_result
            .content
            .lines()
            .last()
            .unwrap()
            .trim()
            .trim_end_matches(",")
            .trim_end_matches(">")
            .to_string();

        let mut account_type_info = CAAccountTypeInfo {
            content: last_line.clone(),
            solana_account_type: SolanaAccountType::from_sonar_result(sonar_result.clone())?,
            account_struct_name: "".to_string(),
            account_wrapper_name: "".to_string(),
            lifetime_name: "".to_string(),
        };

        account_type_info.account_wrapper_name = last_line
            .trim_start_matches(&format!("pub {}: ", sonar_result.name))
            .split("<")
            .next()
            .unwrap()
            .to_string();
        let wrapper_content = last_line.trim_start_matches(&format!(
            "pub {}: {}<",
            sonar_result.name, account_type_info.account_wrapper_name
        ));
        let (lifetime_name, account_struct_name) = if wrapper_content.contains(",") {
            let results = wrapper_content
                .split(",")
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
        sonar_result_content: &str,
    ) -> Result<CAAccountAttributeInfo, ParserError> {
        let mut account_info = CAAccountAttributeInfo {
            content: "".to_string(),
            is_pda: false,
            is_init: false,
            is_mut: false,
            seeds: None,
        };
        if !sonar_result_content.contains("#[account(") {
            return Ok(account_info);
        }
        let mut result_lines_vec = sonar_result_content.trim().lines().collect::<Vec<_>>();
        result_lines_vec.pop().unwrap();
        // single line
        if result_lines_vec.len() == 1 {
            let result_line = result_lines_vec
                .pop()
                .unwrap()
                .trim()
                .trim_start_matches("#[account(")
                .trim_end_matches(")]")
                .trim();
            account_info.is_pda = result_line.contains("seeds = [");
            account_info.is_mut = result_line.split(",").any(|token| token.trim() == "mut");
            account_info.is_init = result_line
                .split(",")
                .any(|token| token.trim() == "init" || token.trim() == "init_if_necessary");
            let seeds = if account_info.is_pda {
                let seeds = Self::parse_seeds(result_line)?;
                Some(seeds)
            } else {
                None
            };
            account_info.seeds = seeds;
            account_info.content = result_line.to_string();
            Ok(account_info)
        // multiline
        } else {
            let result_string = result_lines_vec.join("\n");
            account_info.is_mut = result_string.lines().any(|line| line.trim() == "mut,");
            account_info.is_pda = result_string.contains("seeds = [");
            account_info.is_init = result_string
                .lines()
                .any(|line| line.trim() == "init," || line.trim() == "init_if_necessary,");
            let seeds = if account_info.is_pda {
                let seeds = Self::parse_seeds(&result_string)?;
                Some(seeds)
            } else {
                None
            };
            account_info.seeds = seeds;
            account_info.content = result_string;
            Ok(account_info)
        }
    }

    fn parse_seeds(seeds_string: &str) -> Result<Vec<String>, ParserError> {
        let single_line_regex = Regex::new(r"seeds = \[(.*?)\]").unwrap();
        let seeds_match_single_line = single_line_regex.find(seeds_string);
        if seeds_match_single_line.is_some() {
            return Ok(seeds_match_single_line
                .unwrap()
                .as_str()
                .trim_start_matches("seeds = [")
                .trim_end_matches("]")
                .trim()
                .trim_end_matches(",")
                .split(",")
                .map(|spl| spl.trim().to_string())
                .collect::<Vec<_>>());
        }
        let multiline_regex =
            Regex::new(r"seeds = \[([\s,\t]{0,}[.\n.?][\s\S]{0,}[\s,\t]{0,}[.\n.?][\s,\t]{0,})\]")
                .unwrap();
        let seeds_match_multiline = multiline_regex.find(&seeds_string);
        if seeds_match_multiline.is_some() {
            return Ok(seeds_match_multiline
                .unwrap()
                .as_str()
                .lines()
                .filter_map(|spl| {
                    let line = spl.trim().to_string();
                    if line != "seeds = [" && line != "]" {
                        Some(line)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>());
        }
        Ok(vec![])
    }
}

#[test]
fn test_get_account_attribute_info() {
    // let mut is_pda = false;
    // let mut is_init = false;
    // let mut is_mut = false;
    let test_text_1 = "
    #[account(
        mut,
        seeds = [
            SEED_1.as_bytes(),
            SEED_2.as_ref(),
            SEED_3.as_ref(),
            SEED_4.to_le_bytes(),
        ],
        bump = account_name.load()?.bump,
    )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_1 = CAAccountParser::get_account_attribute_info(test_text_1).unwrap();
    assert_eq!(result_1.is_mut, true, "incorrect is_mut");
    assert_eq!(result_1.is_pda, true, "incorrect is_pda");
    assert_eq!(result_1.is_init, false, "incorrect is_init");
    assert_eq!(result_1.seeds.is_some(), true, "incorrect seeds Option");
    if result_1.seeds.is_some() {
        assert_eq!(result_1.seeds.unwrap().len(), 4, "incorrect seeds len");
    }
    let test_text_2 = "
    #[account(
        mut,
        seeds = [ SEED_1.as_bytes(), SEED_2.as_ref() ],
        bump = account_name.load()?.bump,
    )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_2 = CAAccountParser::get_account_attribute_info(test_text_2).unwrap();
    assert_eq!(result_2.is_mut, true, "incorrect is_mut");
    assert_eq!(result_2.is_pda, true, "incorrect is_pda");
    assert_eq!(result_2.is_init, false, "incorrect is_init");
    assert_eq!(result_2.seeds.is_some(), true, "incorrect seeds Option");
    if result_2.seeds.is_some() {
        assert_eq!(result_2.seeds.unwrap().len(), 2, "incorrect seeds len");
    }
    let test_text_3 = "
    #[account(
        init,
        mut,
        seeds = [ SEED_1.as_bytes(), SEED_2.as_ref() ],
        bump = account_name.load()?.bump,
    )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_3 = CAAccountParser::get_account_attribute_info(test_text_3).unwrap();
    assert_eq!(result_3.is_mut, true, "incorrect is_mut");
    assert_eq!(result_3.is_pda, true, "incorrect is_pda");
    assert_eq!(result_3.is_init, true, "incorrect is_init");
    assert_eq!(result_3.seeds.is_some(), true, "incorrect seeds Option");
    if result_3.seeds.is_some() {
        assert_eq!(result_3.seeds.unwrap().len(), 2, "incorrect seeds len");
    }
    // is_pda = true
    let test_text_4 = "
    #[account( seeds = [ SEED_1.as_bytes(), SEED_2.as_ref() ])]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_4 = CAAccountParser::get_account_attribute_info(test_text_4).unwrap();
    assert_eq!(result_4.is_mut, false, "incorrect is_mut");
    assert_eq!(result_4.is_pda, true, "incorrect is_pda");
    assert_eq!(result_4.is_init, false, "incorrect is_init");
    assert_eq!(result_4.seeds.is_some(), true, "incorrect seeds Option");
    if result_4.seeds.is_some() {
        assert_eq!(result_4.seeds.unwrap().len(), 2, "incorrect seeds len");
    }
    let test_text_5 = "
    #[account( mut, seeds = [ SEED_1.as_bytes(), SEED_2.as_ref(), ])]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_5 = CAAccountParser::get_account_attribute_info(test_text_5).unwrap();
    assert_eq!(result_5.is_mut, true, "incorrect is_mut");
    assert_eq!(result_5.is_pda, true, "incorrect is_pda");
    assert_eq!(result_5.is_init, false, "incorrect is_init");
    assert_eq!(result_5.seeds.is_some(), true, "incorrect seeds Option");
    if result_5.seeds.is_some() {
        assert_eq!(result_5.seeds.unwrap().len(), 2, "incorrect seeds len");
    }
    let test_text_6 = "
    #[account( mut, init, seeds = [ SEED_1.as_bytes(), SEED_2.as_ref(), ])]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_6 = CAAccountParser::get_account_attribute_info(test_text_6).unwrap();
    assert_eq!(result_6.is_mut, true, "incorrect is_mut");
    assert_eq!(result_6.is_pda, true, "incorrect is_pda");
    assert_eq!(result_6.is_init, true, "incorrect is_init");
    assert_eq!(result_6.seeds.is_some(), true, "incorrect seeds Option");
    if result_6.seeds.is_some() {
        assert_eq!(result_6.seeds.unwrap().len(), 2, "incorrect seeds len");
    }

    let test_text_7 = "
    #[account( mut, init_if_necessary,  seeds = [ SEED_1.as_bytes(), SEED_2.as_ref(), SEED_3.as_ref() ])]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_7 = CAAccountParser::get_account_attribute_info(test_text_7).unwrap();
    assert_eq!(result_7.is_mut, true, "incorrect is_mut");
    assert_eq!(result_7.is_pda, true, "incorrect is_pda");
    assert_eq!(result_7.is_init, true, "incorrect is_init");
    assert_eq!(result_7.seeds.is_some(), true, "incorrect seeds Option");
    if result_7.seeds.is_some() {
        assert_eq!(result_7.seeds.unwrap().len(), 3, "incorrect seeds len");
    }

    // is_pda = false
    let test_text_8 = "
    #[account( mut )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_8 = CAAccountParser::get_account_attribute_info(test_text_8).unwrap();
    assert_eq!(result_8.is_mut, true, "incorrect is_mut");
    assert_eq!(result_8.is_pda, false, "incorrect is_pda");
    assert_eq!(result_8.is_init, false, "incorrect is_init");
    assert_eq!(result_8.seeds.is_some(), false, "incorrect seeds Option");
    if result_8.seeds.is_some() {
        assert_eq!(result_8.seeds.unwrap().len(), 3, "incorrect seeds len");
    }
    let test_text_9 = "
    #[account( mut, init )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_9 = CAAccountParser::get_account_attribute_info(test_text_9).unwrap();
    assert_eq!(result_9.is_mut, true, "incorrect is_mut");
    assert_eq!(result_9.is_pda, false, "incorrect is_pda");
    assert_eq!(result_9.is_init, true, "incorrect is_init");
    assert_eq!(result_9.seeds.is_some(), false, "incorrect seeds Option");
    if result_9.seeds.is_some() {
        assert_eq!(result_9.seeds.unwrap().len(), 3, "incorrect seeds len");
    }
    let test_text_10 = "
    #[account( mut, init_if_necessary )]
    pub account_name: AccountLoader<'info, AccountType>,";
    let result_10 = CAAccountParser::get_account_attribute_info(test_text_10).unwrap();
    assert_eq!(result_10.is_mut, true, "incorrect is_mut");
    assert_eq!(result_10.is_pda, false, "incorrect is_pda");
    assert_eq!(result_10.is_init, true, "incorrect is_init");
    assert_eq!(result_10.seeds.is_some(), false, "incorrect seeds Option");
    if result_10.seeds.is_some() {
        assert_eq!(result_10.seeds.unwrap().len(), 3, "incorrect seeds len");
    }

    let test_text_11 = "pub account_name: AccountLoader<'info, AccountType>,";
    let result_11 = CAAccountParser::get_account_attribute_info(test_text_11).unwrap();
    assert_eq!(result_11.is_mut, false, "incorrect is_mut");
    assert_eq!(result_11.is_pda, false, "incorrect is_pda");
    assert_eq!(result_11.is_init, false, "incorrect is_init");
    assert_eq!(result_11.seeds.is_some(), false, "incorrect seeds Option");
}

#[test]
fn test_get_account_type_info_struct_name() {
    let mut sonar_result = SonarResult {
        name: "account_test".to_string(),
        content: "".to_string(),
        trailing_whitespaces: 0,
        result_type: SonarResultType::ContextAccountsAll,
        start_line_index: 0,
        end_line_index: 0,
        is_public: false,
    };

    sonar_result.content = "pub account_test: Signer<'info>,".to_string();
    let result_1 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_1.account_struct_name, "Signer",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_1.account_wrapper_name, "Signer",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_1.lifetime_name, "'info", "incorrect lifetime_name");

    sonar_result.content = "pub account_test: UncheckedAccount<'info>,".to_string();
    let result_2 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_2.account_struct_name, "UncheckedAccount",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_2.account_wrapper_name, "UncheckedAccount",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_2.lifetime_name, "'info", "incorrect lifetime_name");

    sonar_result.content = "pub account_test: AccountLoader<'info, OwnedAccount1>,".to_string();
    let result_3 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_3.account_struct_name, "OwnedAccount1",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_3.account_wrapper_name, "AccountLoader",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_3.lifetime_name, "'info", "incorrect lifetime_name");

    sonar_result.content = "pub account_test: Account<'info, OwnedAccount2>,".to_string();
    let result_4 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_4.account_struct_name, "OwnedAccount2",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_4.account_wrapper_name, "Account",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_4.lifetime_name, "'info", "incorrect lifetime_name");

    sonar_result.content = "pub account_test: OwnedAccount3<'info>,".to_string();
    let result_5 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_5.account_struct_name, "OwnedAccount3",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_5.account_wrapper_name, "OwnedAccount3",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_5.lifetime_name, "'info", "incorrect lifetime_name");

    sonar_result.content = "pub account_test: AccountLoader<'info, Mint>,".to_string();
    let result_6 = CAAccountParser::get_account_type_info(sonar_result.clone()).unwrap();
    assert_eq!(
        result_6.account_struct_name, "Mint",
        "incorrect account_struct_name"
    );
    assert_eq!(
        result_6.account_wrapper_name, "AccountLoader",
        "incorrect account_wrapper_name"
    );
    assert_eq!(result_6.lifetime_name, "'info", "incorrect lifetime_name");
}
