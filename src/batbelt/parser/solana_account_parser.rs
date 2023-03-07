use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::sonar::SonarResult;
use error_stack::{IntoReport, Report, Result, ResultExt};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum SolanaAccountType {
    TokenAccount,
    Mint,
    Signer,
    UncheckedAccount,
    ProgramStateAccount,
    Other,
}

impl SolanaAccountType {
    pub fn from_sonar_result(sonar_result: SonarResult) -> Result<Self, ParserError> {
        let last_line = sonar_result.content.lines();
        let last_line = last_line.last().unwrap();

        if last_line.contains("Signer<") {
            return Ok(Self::Signer);
        }

        if last_line.contains(&Self::UncheckedAccount.to_string()) {
            return Ok(Self::UncheckedAccount);
        }

        if last_line.contains(&Self::TokenAccount.to_string()) {
            return Ok(Self::TokenAccount);
        }

        if last_line.contains(&Self::Mint.to_string()) {
            return Ok(Self::Mint);
        }

        let mut solana_accounts_metadata = BatMetadata::read_metadata()
            .change_context(ParserError)?
            .source_code
            .structs_source_code
            .into_iter()
            .filter(|s_metda| s_metda.struct_type == StructMetadataType::SolanaAccount);
        if solana_accounts_metadata.any(|solana_account| last_line.contains(&solana_account.name)) {
            return Ok(Self::ProgramStateAccount);
        }

        Ok(Self::Other)
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SolanaAccountParserAccount {
    pub account_name: String,
    pub account_type: String,
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct SolanaAccountParser {
    pub solana_account_type: SolanaAccountType,
    pub account_struct_name: String,
    pub accounts: Vec<SolanaAccountParserAccount>,
}

impl SolanaAccountParser {
    pub fn new_from_struct_name_and_solana_account_type(
        account_struct_name: String,
        solana_account_type: SolanaAccountType,
    ) -> ParserResult<Self> {
        let mut new_solana_account_parser = Self {
            solana_account_type,
            account_struct_name,
            accounts: vec![],
        };
        match solana_account_type {
            SolanaAccountType::ProgramStateAccount => {
                new_solana_account_parser.parse_program_state_account()?;
            }
            _ => unimplemented!(),
        }
        Ok(new_solana_account_parser)
    }

    fn parse_program_state_account(&mut self) -> ParserResult<()> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        return match bat_metadata
            .source_code
            .structs_source_code
            .into_iter()
            .find(|struct_sc| {
                struct_sc.struct_type == StructMetadataType::SolanaAccount
                    && struct_sc.name == self.account_struct_name
            }) {
            None => Err(Report::new(ParserError).attach_printable(format!(
                "No Solana Account was found with name {}",
                self.account_struct_name
            ))),
            Some(struct_metadata) => {
                let account_param_regex = Regex::new(r"pub [A-Za-z0-9_]+: [\w]+,")
                    .into_report()
                    .change_context(ParserError)?;
                let struct_metadata_content = struct_metadata
                    .to_source_code_parser(None)
                    .get_source_code_content();
                let account_vec = struct_metadata_content
                    .lines()
                    .filter_map(|line| {
                        if account_param_regex.is_match(line) {
                            let mut line_split = line
                                .trim()
                                .trim_end_matches(',')
                                .trim_start_matches("pub ")
                                .split(": ");
                            Some(SolanaAccountParserAccount {
                                account_name: line_split.next().unwrap().to_string(),
                                account_type: line_split.next().unwrap().to_string(),
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                self.accounts = account_vec;
                Ok(())
            }
        };
    }
}
