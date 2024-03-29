use crate::batbelt::parser::entrypoint_parser::EntrypointParser;

use std::fmt::Debug;

use crate::batbelt;
use crate::batbelt::path::BatFile;

use crate::batbelt::metadata::{
    BatMetadataParser, BatMetadataType, MetadataId, SourceCodeMetadata,
};

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use crate::batbelt::BatEnumerator;
use error_stack::{Result, ResultExt};

use super::MetadataError;
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::parser::parse_formatted_path;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{fs, vec};
use strum::IntoEnumIterator;
use walkdir::DirEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructSourceCodeMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub metadata_id: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser<StructMetadataType> for StructSourceCodeMetadata {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn path(&self) -> String {
        self.path.clone()
    }
    fn metadata_id(&self) -> MetadataId {
        self.metadata_id.clone()
    }
    fn start_line_index(&self) -> usize {
        self.start_line_index
    }
    fn end_line_index(&self) -> usize {
        self.end_line_index
    }
    fn metadata_sub_type(&self) -> StructMetadataType {
        self.struct_type
    }
    fn get_bat_metadata_type() -> BatMetadataType {
        BatMetadataType::Struct
    }
    fn metadata_name() -> String {
        "Struct".to_string()
    }
    fn new(
        path: String,
        name: String,
        metadata_sub_type: StructMetadataType,
        start_line_index: usize,
        end_line_index: usize,
        metadata_id: MetadataId,
    ) -> Self {
        StructSourceCodeMetadata {
            path,
            name,
            metadata_id,
            struct_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }

    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError> {
        let entry_path = entry.path().to_str().unwrap().to_string();
        let file_content = fs::read_to_string(entry.path()).unwrap();
        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Struct);
        let mut metadata_result = vec![];
        for result in bat_sonar.results {
            let struct_type =
                if Self::assert_struct_is_solana_account(&file_content, result.clone()) {
                    StructMetadataType::SolanaAccount
                } else if Self::assert_struct_is_context_accounts(&file_content, result.clone())? {
                    StructMetadataType::ContextAccounts
                } else {
                    StructMetadataType::Other
                };
            let struct_metadata = StructSourceCodeMetadata::new(
                entry_path.clone(),
                result.name.to_string(),
                struct_type,
                result.start_line_index + 1,
                result.end_line_index + 1,
                Self::create_metadata_id(),
            );
            metadata_result.push(struct_metadata);
        }
        // let bat_metadata = BatMetadata::read_metadata()?;
        // bat_metadata
        //     .source_code
        //     .update_structs(metadata_result.clone())?;

        Ok(metadata_result)
    }
}

impl StructSourceCodeMetadata {
    fn assert_struct_is_context_accounts(
        file_info_content: &str,
        sonar_result: SonarResult,
    ) -> Result<bool, MetadataError> {
        if sonar_result.start_line_index > 0 {
            let previous_line =
                file_info_content.lines().collect::<Vec<_>>()[sonar_result.start_line_index - 1];
            let filtered_previous_line = previous_line
                .trim()
                .trim_end_matches(")]")
                .trim_start_matches("#[derive(");
            let mut tokenized = filtered_previous_line.split(", ");
            if tokenized.any(|token| token == "Acccounts") {
                return Ok(true);
            }
        }
        let context_accounts_content = vec![
            "Signer<",
            "AccountLoader<",
            "UncheckedAccount<",
            "#[account(",
        ];
        if context_accounts_content
            .iter()
            .any(|content| sonar_result.content.contains(content))
        {
            return Ok(true);
        }
        let lib_file_path = batbelt::path::get_file_path(BatFile::ProgramLib, false)
            .change_context(MetadataError)?;
        let entrypoints = BatSonar::new_from_path(
            &lib_file_path,
            Some("#[program]"),
            SonarResultType::Function,
        );
        let mut entrypoints_context_accounts_names = entrypoints
            .results
            .iter()
            .map(|result| EntrypointParser::get_context_name(&result.name).unwrap());
        if entrypoints_context_accounts_names.any(|name| name == sonar_result.name) {
            return Ok(true);
        }
        Ok(false)
    }

    fn assert_struct_is_solana_account(file_info_content: &str, sonar_result: SonarResult) -> bool {
        if sonar_result.start_line_index > 3 {
            let previous_line_1 =
                file_info_content.lines().collect::<Vec<_>>()[sonar_result.start_line_index - 1];
            let previous_line_2 =
                file_info_content.lines().collect::<Vec<_>>()[sonar_result.start_line_index - 2];
            let previous_line_3 =
                file_info_content.lines().collect::<Vec<_>>()[sonar_result.start_line_index - 3];
            if previous_line_1.contains("#[account")
                || previous_line_2.contains("#[account")
                || previous_line_3.contains("#[account")
            {
                return true;
            }
        }

        false
    }

    pub fn prompt_selection() -> Result<Self, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selection = BatDialoguer::select(prompt_text, metadata_names, None)
            .change_context(MetadataError)?;

        Ok(metadata_vec[selection].clone())
    }

    pub fn prompt_multiselection(
        select_all: bool,
        force_select: bool,
    ) -> Result<Vec<Self>, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selections = BatDialoguer::multiselect(
            prompt_text,
            metadata_names.clone(),
            Some(&vec![select_all; metadata_names.len()]),
            force_select,
        )
        .change_context(MetadataError)?;

        let filtered_vec = metadata_vec
            .into_iter()
            .enumerate()
            .filter_map(|(sc_index, sc_metadata)| {
                if selections.iter().any(|selection| &sc_index == selection) {
                    Some(sc_metadata)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Ok(filtered_vec)
    }

    pub fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
        let prompt_text = format!(
            "Please select the {} {}:",
            Self::metadata_name().blue(),
            "type".blue()
        );
        let selection = BatDialoguer::select(
            prompt_text,
            StructMetadataType::get_colorized_type_vec(true),
            None,
        )
        .change_context(MetadataError)?;
        let selected_sub_type = StructMetadataType::get_type_vec()[selection];
        let metadata_vec_filtered =
            SourceCodeMetadata::get_filtered_structs(None, Some(selected_sub_type))
                .change_context(MetadataError)?;
        let metadata_names = metadata_vec_filtered
            .iter()
            .map(|metadata| {
                parse_formatted_path(
                    metadata.name(),
                    metadata.path(),
                    metadata.start_line_index(),
                )
            })
            .collect::<Vec<_>>();
        Ok((metadata_vec_filtered, metadata_names))
    }
}

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
pub enum StructMetadataType {
    ContextAccounts,
    SolanaAccount,
    Other,
}

impl BatEnumerator for StructMetadataType {}
