use crate::batbelt::metadata::structs_source_code_metadata::{
    StructMetadataType, StructSourceCodeMetadata,
};
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};
use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::SonarResult;
use error_stack::{Result, ResultExt};
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
