use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::parser::solana_account_parser::SolanaAccountParser;
use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use error_stack::Result;

pub struct ContextAccountsParser {
    pub context_name: String,
    pub accounts: String,
}

impl ContextAccountsParser {
    // pub fn new_from_metadata(struct_metadata: StructMetadata) -> Result<Self, ParserError> {
    //     let ca_content = struct_metadata.to_source_code().get_source_code_content();
    //     let ca_accounts = BatSonar::new_scanned(&ca_content,SonarResultType::ContextAccountsAll);
    // }
}

pub struct CAAccount {
    pub is_mut: bool,
    pub is_pda: bool,
    pub account: SolanaAccountParser,
}

// impl CAAccount {
//     pub fn new_from_sonar_result_vec(
//         sonar_result_vec: Vec<SonarResult>,
//     ) -> Result<Vec<Self>, ParserError> {
//     }
// }
