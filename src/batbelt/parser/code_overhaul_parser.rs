use crate::batbelt::metadata::code_overhaul_metadata::CodeOverhaulSignerMetadata;
use crate::batbelt::metadata::{BatMetadata, MetadataId};
use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::sonar::{SonarResult, SonarResultType};
use error_stack::{IntoReport, Report, Result, ResultExt};
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulParser {
    pub entry_point_name: String,
    pub validations: Vec<String>,
    pub signers: Vec<CodeOverhaulSignerMetadata>,
}

impl CodeOverhaulParser {
    pub fn new_from_entry_point_name(entry_point_name: String) -> ParserResult<Self> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let co_metadata = bat_metadata
            .get_code_overhaul_metadata_by_entry_point_name(entry_point_name.clone())
            .change_context(ParserError)?;
        Ok(Self {
            entry_point_name,
            validations: co_metadata.validations,
            signers: co_metadata.signers,
        })
    }
}
