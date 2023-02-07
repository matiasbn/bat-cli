use crate::batbelt::structs::FileInfo;
use std::fmt::{Debug, Display};

use crate::batbelt;
use crate::batbelt::path::{canonicalize_path, FolderPathType};
use colored::{ColoredString, Colorize};

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel};
use crate::batbelt::metadata::functions::FunctionMetadata;
use crate::batbelt::metadata::structs::StructMetadata;
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use inflector::Inflector;
use std::vec;
use strum::IntoEnumIterator;

#[derive(Debug, Clone)]
pub struct CodeOverhaulMetadata {
    pub signers: Vec<String>,
    pub entry_point_function: FunctionMetadata,
    pub handler_function: FunctionMetadata,
    pub context_accounts: StructMetadata,
    pub dependencies: Vec<FunctionMetadata>,
    pub miro_board_url: String,
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
enum CodeOverhaulInfoSection {
    ContextAccounts,
    Signers,
    EntryPointFunction,
    StartLineIndex,
    EndLineIndex,
}

impl CodeOverhaulInfoSection {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }
}

impl CodeOverhaulMetadata {
    pub fn new(
        signers: Vec<String>,
        entry_point_function: FunctionMetadata,
        handler_function: FunctionMetadata,
        context_accounts: StructMetadata,
        dependencies: Vec<FunctionMetadata>,
        miro_board_url: String,
    ) -> Self {
        CodeOverhaulMetadata {
            signers,
            entry_point_function,
            handler_function,
            context_accounts,
            dependencies,
            miro_board_url,
        }
    }

    // pub fn get_markdown_section_content_string(&self) -> String {
    //     format!(
    //         "- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
    //         self.struct_type.to_snake_case(),
    //         self.path,
    //         self.start_line_index,
    //         self.end_line_index
    //     )
    // }
    //
    // pub fn get_markdown_section(&self, section_hash: &str) -> MarkdownSection {
    //     let section_level_header = MarkdownSectionLevel::H2.get_header(&self.name);
    //     let section_header = MarkdownSectionHeader::new_from_header_and_hash(
    //         section_level_header,
    //         section_hash.to_string(),
    //     );
    //     let md_section = MarkdownSection::new(
    //         section_header,
    //         self.get_markdown_section_content_string(),
    //         0,
    //         0,
    //     );
    //     md_section
    // }
    //
    // pub fn from_markdown_section(md_section: MarkdownSection) -> Self {
    //     let name = md_section.section_header.title;
    //     let path =
    //         Self::parse_metadata_info_section(&md_section.content, CodeOverhaulInfoSection::Path);
    //     let struct_type_string =
    //         Self::parse_metadata_info_section(&md_section.content, CodeOverhaulInfoSection::Type);
    //     let start_line_index = Self::parse_metadata_info_section(
    //         &md_section.content,
    //         CodeOverhaulInfoSection::StartLineIndex,
    //     );
    //     let end_line_index = Self::parse_metadata_info_section(
    //         &md_section.content,
    //         CodeOverhaulInfoSection::EndLineIndex,
    //     );
    //     CodeOverhaulMetadata::new(
    //         path,
    //         name,
    //         CodeOverhaulMetadataType::from_str(&struct_type_string),
    //         start_line_index.parse::<usize>().unwrap(),
    //         end_line_index.parse::<usize>().unwrap(),
    //     )
    // }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        struct_section: CodeOverhaulInfoSection,
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
}

// #[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
// pub enum CodeOverhaulMetadataType {
//     ContextAccounts,
//     SolanaAccount,
//     Event,
//     Input,
//     Other,
// }
//
// impl CodeOverhaulMetadataType {
//     pub fn to_snake_case(&self) -> String {
//         self.to_string().to_snake_case()
//     }
//
//     pub fn to_sentence_case(&self) -> String {
//         self.to_string().to_sentence_case()
//     }
//
//     pub fn get_structs_type_vec() -> Vec<CodeOverhaulMetadataType> {
//         CodeOverhaulMetadataType::iter().collect::<Vec<_>>()
//     }
//
//     pub fn from_str(type_str: &str) -> CodeOverhaulMetadataType {
//         let structs_type_vec = Self::get_structs_type_vec();
//         let struct_type = structs_type_vec
//             .iter()
//             .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
//             .unwrap();
//         struct_type.clone()
//     }
//
//     pub fn get_colorized_structs_type_vec() -> Vec<ColoredString> {
//         let struct_type_vec = Self::get_structs_type_vec();
//         let structs_type_colorized = struct_type_vec
//             .iter()
//             .map(|struct_type| match struct_type {
//                 CodeOverhaulMetadataType::ContextAccounts => struct_type.to_sentence_case().red(),
//                 CodeOverhaulMetadataType::SolanaAccount => struct_type.to_sentence_case().yellow(),
//                 CodeOverhaulMetadataType::Event => struct_type.to_sentence_case().green(),
//                 CodeOverhaulMetadataType::Input => struct_type.to_sentence_case().blue(),
//                 CodeOverhaulMetadataType::Other => struct_type.to_sentence_case().magenta(),
//                 _ => unimplemented!("color no implemented for given type"),
//             })
//             .collect::<Vec<_>>();
//         structs_type_colorized
//     }
// }
//
// pub fn get_structs_metadata_from_program() -> Result<Vec<CodeOverhaulMetadata>, String> {
//     let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
//     let program_folder_files_info =
//         batbelt::helpers::get::get_only_files_from_folder(program_path)?;
//     let mut structs_metadata: Vec<CodeOverhaulMetadata> = vec![];
//     for file_info in program_folder_files_info {
//         let mut struct_metadata_result = get_code_overhaul_metadata_from_file_info(file_info)?;
//         structs_metadata.append(&mut struct_metadata_result);
//     }
//     structs_metadata.sort_by(|struct_a, struct_b| struct_a.name.cmp(&struct_b.name));
//     Ok(structs_metadata)
// }
//
// pub fn get_code_overhaul_metadata_from_file_info(
//     struct_file_info: FileInfo,
// ) -> Result<Vec<CodeOverhaulMetadata>, String> {
//     let mut struct_metadata_vec: Vec<CodeOverhaulMetadata> = vec![];
//     println!(
//         "starting the review of the {} file",
//         struct_file_info.path.clone().blue()
//     );
//     let file_info_content = struct_file_info.read_content().unwrap();
//     let struct_types_colored = CodeOverhaulMetadataType::get_colorized_structs_type_vec();
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
//         let prompt_text = "Select the struct type:";
//         let selection =
//             batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
//         let struct_type = CodeOverhaulMetadataType::get_structs_type_vec()[selection];
//         let struct_metadata = CodeOverhaulMetadata::new(
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
