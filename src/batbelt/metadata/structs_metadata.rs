use crate::batbelt::parser::entrypoint_parser::EntrypointParser;

use std::fmt::Debug;

use crate::batbelt;
use crate::batbelt::path::{BatFile, BatFolder};
use colored::{ColoredString, Colorize};

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel};
use crate::batbelt::metadata::source_code_metadata::SourceCodeMetadata;

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use error_stack::{Result, ResultExt};
use inflector::Inflector;
use std::{fs, vec};
use strum::IntoEnumIterator;

use super::MetadataError;

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
enum StructMetadataInfoSection {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
}

impl StructMetadataInfoSection {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }
}

#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl StructMetadata {
    pub fn new(
        path: String,
        name: String,
        struct_type: StructMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        StructMetadata {
            path,
            name,
            struct_type,
            start_line_index,
            end_line_index,
        }
    }

    pub fn get_markdown_section_content_string(&self) -> String {
        format!(
            "# {}\n\n- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.name,
            self.struct_type.to_snake_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
    }

    pub fn to_markdown_section(&self, section_hash: &str) -> MarkdownSection {
        let section_level_header = MarkdownSectionLevel::H2.get_header(&self.name);
        let section_header = MarkdownSectionHeader::new_from_header_and_hash(
            section_level_header,
            section_hash.to_string(),
            0,
        );
        let md_section = MarkdownSection::new(
            section_header,
            self.get_markdown_section_content_string(),
            0,
            0,
        );
        md_section
    }

    pub fn to_source_code(&self, optional_name: Option<String>) -> SourceCodeMetadata {
        SourceCodeMetadata::new(
            if let Some(struct_name) = optional_name {
                struct_name
            } else {
                self.name.clone()
            },
            self.path.clone(),
            self.start_line_index,
            self.end_line_index,
        )
    }

    pub fn from_markdown_section(md_section: MarkdownSection) -> Self {
        let name = md_section.section_header.title;
        let path =
            Self::parse_metadata_info_section(&md_section.content, StructMetadataInfoSection::Path);
        let struct_type_string =
            Self::parse_metadata_info_section(&md_section.content, StructMetadataInfoSection::Type);
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            StructMetadataInfoSection::StartLineIndex,
        );
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            StructMetadataInfoSection::EndLineIndex,
        );
        StructMetadata::new(
            path,
            name,
            StructMetadataType::from_str(&struct_type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        )
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        struct_section: StructMetadataInfoSection,
    ) -> String {
        let section_prefix = struct_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .unwrap()
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        data
    }

    pub fn get_structs_metadata_from_program() -> Result<Vec<Self>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files(false, None, None)
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
                            "Function found at {}\n{}",
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

impl StructMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn from_str(type_str: &str) -> StructMetadataType {
        let structs_type_vec = Self::get_structs_type_vec();
        let struct_type = structs_type_vec
            .iter()
            .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
            .unwrap();
        struct_type.clone()
    }

    pub fn get_structs_type_vec() -> Vec<StructMetadataType> {
        StructMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn get_colorized_structs_type_vec() -> Vec<ColoredString> {
        let struct_type_vec = Self::get_structs_type_vec();
        let structs_type_colorized = struct_type_vec
            .iter()
            .map(|struct_type| match struct_type {
                StructMetadataType::ContextAccounts => struct_type.to_sentence_case().red(),
                StructMetadataType::SolanaAccount => struct_type.to_sentence_case().yellow(),
                StructMetadataType::Other => struct_type.to_sentence_case().magenta(),
                _ => unimplemented!("color no implemented for given type"),
            })
            .collect::<Vec<_>>();
        structs_type_colorized
    }
}
//
// pub fn get_structs_metadata_from_program() -> Result<Vec<StructMetadata>, MetadataError> {
//     let program_path = batbelt::path::get_folder_path(BatFolder::ProgramPath, false)
//         .change_context(MetadataError)?;
//     let program_folder_files_info = batbelt::helpers::get::get_only_files_from_folder(program_path)
//         .change_context(MetadataError)?;
//     let mut structs_metadata: Vec<StructMetadata> = vec![];
//     for file_info in program_folder_files_info {
//         let mut struct_metadata_result =
//             get_struct_metadata_from_file_info(file_info).change_context(MetadataError)?;
//         structs_metadata.append(&mut struct_metadata_result);
//     }
//     structs_metadata.sort_by(|struct_a, struct_b| struct_a.name.cmp(&struct_b.name));
//     Ok(structs_metadata)
// }
//
// pub fn get_struct_metadata_from_file_info(
//     struct_file_info: FileInfo,
// ) -> Result<Vec<StructMetadata>, MetadataError> {
//     let mut struct_metadata_vec: Vec<StructMetadata> = vec![];
//     println!(
//         "starting the review of the {} file",
//         struct_file_info.path.clone().blue()
//     );
//     let file_info_content = struct_file_info.read_content().unwrap();
//     let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Struct);
//     for result in bat_sonar.results {
//         println!(
//             "Struct found at {}\n{}",
//             format!(
//                 "{}:{}",
//                 struct_file_info.path.clone(),
//                 result.start_line_index + 1,
//             )
//             .magenta(),
//             result.content.clone().green()
//         );
//         if assert_struct_is_context_accounts(&file_info_content, result.clone())? {
//             println!("{}", "Struct found is ContextAccounts type!".yellow());
//             let struct_type = StructMetadataType::ContextAccounts;
//             let struct_metadata = StructMetadata::new(
//                 struct_file_info.path.clone(),
//                 result.name.to_string(),
//                 struct_type,
//                 result.start_line_index + 1,
//                 result.end_line_index + 1,
//             );
//             struct_metadata_vec.push(struct_metadata);
//             continue;
//         }
//         if assert_struct_is_solana_account(&file_info_content, result.clone()) {
//             println!("{}", "Struct found is SolanaAccount type!".yellow());
//             let struct_type = StructMetadataType::SolanaAccount;
//             let struct_metadata = StructMetadata::new(
//                 struct_file_info.path.clone(),
//                 result.name.to_string(),
//                 struct_type,
//                 result.start_line_index + 1,
//                 result.end_line_index + 1,
//             );
//             struct_metadata_vec.push(struct_metadata);
//             continue;
//         }
//         // let prompt_text = "Select the struct type:";
//         // let selection =
//         //     batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
//         let struct_type = StructMetadataType::Other;
//         let struct_metadata = StructMetadata::new(
//             struct_file_info.path.clone(),
//             result.name.to_string(),
//             struct_type,
//             result.start_line_index + 1,
//             result.end_line_index + 1,
//         );
//         struct_metadata_vec.push(struct_metadata);
//     }
//     println!(
//         "finishing the review of the {} file",
//         struct_file_info.path.clone().blue()
//     );
//     Ok(struct_metadata_vec)
// }
