use crate::batbelt::parser::entrypoint_parser::EntrypointParser;

use std::fmt::Debug;

use crate::batbelt;
use crate::batbelt::path::{BatFile, BatFolder};
use colored::Colorize;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::{
    BatMetadataMarkdownContent, BatMetadataParser, BatMetadataType, BatMetadataTypeParser,
};

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use error_stack::{Report, Result, ResultExt};
use inflector::Inflector;
use std::{fs, vec};
use strum::IntoEnumIterator;

use super::MetadataError;

#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub metadata_id: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser<StructMetadataType> for StructMetadata {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn path(&self) -> String {
        self.path.clone()
    }
    fn metadata_id(&self) -> String {
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

    fn match_bat_metadata_type() -> BatMetadataType {
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
    ) -> Self {
        StructMetadata {
            path,
            name,
            metadata_id: Self::create_metadata_id(),
            struct_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }
}

impl StructMetadata {
    pub fn get_metadata_from_program_files() -> Result<Vec<Self>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
            .change_context(MetadataError)?;
        let mut structs_metadata: Vec<Self> =
            program_dir_entries
                .into_iter()
                .fold(vec![], |mut result_vec, entry| {
                    let entry_path = entry.path().to_str().unwrap().to_string();
                    println!("starting the review of the {} file", entry_path.blue());
                    let file_content = fs::read_to_string(entry.path()).unwrap();
                    let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Struct);
                    for result in bat_sonar.results {
                        println!(
                            "Struct found at {}\n{}",
                            format!("{}:{}", &entry_path, result.start_line_index + 1).magenta(),
                            result.content.clone().green()
                        );
                        let is_context_accounts =
                            Self::assert_struct_is_context_accounts(&file_content, result.clone())
                                .unwrap();
                        if is_context_accounts {
                            println!("{}", "Struct found is ContextAccounts type!".yellow());
                            let struct_type = StructMetadataType::ContextAccounts;
                            let struct_metadata = StructMetadata::new(
                                entry_path.clone(),
                                result.name.to_string(),
                                struct_type,
                                result.start_line_index + 1,
                                result.end_line_index + 1,
                            );
                            result_vec.push(struct_metadata);
                            continue;
                        }
                        let is_solana_account =
                            Self::assert_struct_is_solana_account(&file_content, result.clone());
                        if is_solana_account {
                            println!("{}", "Struct found is SolanaAccount type!".yellow());
                            let struct_type = StructMetadataType::SolanaAccount;
                            let struct_metadata = StructMetadata::new(
                                entry_path.clone(),
                                result.name.to_string(),
                                struct_type,
                                result.start_line_index + 1,
                                result.end_line_index + 1,
                            );
                            result_vec.push(struct_metadata);
                            continue;
                        }
                        // let prompt_text = "Select the struct type:";
                        // let selection =
                        //     batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
                        let struct_type = StructMetadataType::Other;
                        let struct_metadata = StructMetadata::new(
                            entry_path.clone(),
                            result.name.to_string(),
                            struct_type,
                            result.start_line_index + 1,
                            result.end_line_index + 1,
                        );
                        result_vec.push(struct_metadata);
                    }
                    println!(
                        "finishing the review of the {} file",
                        entry_path.clone().blue()
                    );
                    return result_vec;
                });
        structs_metadata.sort_by(|function_a, function_b| function_a.name.cmp(&function_b.name));
        Ok(structs_metadata)
    }

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
        return Ok(false);
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

        return false;
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum StructMetadataType {
    ContextAccounts,
    SolanaAccount,
    Other,
}

impl BatMetadataTypeParser for StructMetadataType {}
