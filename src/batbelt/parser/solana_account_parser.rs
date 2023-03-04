use crate::batbelt::metadata::structs_metadata::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::BatMetadataParser;
use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::SonarResult;
use error_stack::{Result, ResultExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter, Serialize, Deserialize)]
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

        let solana_accounts_metadata =
            StructMetadata::get_filtered_metadata(None, Some(StructMetadataType::SolanaAccount))
                .change_context(ParserError)?;
        if solana_accounts_metadata
            .into_iter()
            .any(|solana_account| last_line.contains(&solana_account.name))
        {
            return Ok(Self::ProgramStateAccount);
        }

        Ok(Self::Other)
    }
}
