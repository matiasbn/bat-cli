use crate::batbelt::structs::FileInfo;

use crate::batbelt;
use crate::batbelt::path::FolderPathType;
use colored::Colorize;

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionLevel};
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use inflector::Inflector;
use std::vec;

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

    pub fn get_markdown_section_string(&self) -> String {
        format!(
            "{}\n\n- type: {}\n- path:{}\n- start_line_index:{}\n- end_line_index:{}",
            MarkdownSectionLevel::H2.get_header(&self.name),
            self.struct_type.to_snake_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display)]
pub enum StructMetadataType {
    ContextAccounts,
    SolanaAccount,
    Input,
    Other,
}

impl StructMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn get_structs_type_vec() -> Vec<StructMetadataType> {
        vec![
            StructMetadataType::ContextAccounts,
            StructMetadataType::SolanaAccount,
            StructMetadataType::Input,
            StructMetadataType::Other,
        ]
    }
}

pub fn get_structs_metadata_from_program() -> Result<Vec<StructMetadata>, String> {
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
    let mut structs_metadata: Vec<StructMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut struct_metadata_result = get_struct_metadata_from_file_info(file_info)?;
        structs_metadata.append(&mut struct_metadata_result);
    }
    Ok(structs_metadata)
}

pub fn get_struct_metadata_from_file_info(
    struct_file_info: FileInfo,
) -> Result<Vec<StructMetadata>, String> {
    let mut struct_metadata_vec: Vec<StructMetadata> = vec![];
    println!(
        "starting the review of the {} file",
        struct_file_info.path.clone().blue()
    );
    let file_info_content = struct_file_info.read_content().unwrap();
    let struct_types_colored = StructMetadataType::get_structs_type_vec()
        .iter()
        .map(|struct_type| struct_type.to_sentence_case())
        .enumerate()
        .map(|f| match f.0 {
            0 => f.1.red(),
            1 => f.1.yellow(),
            2 => f.1.green(),
            3 => f.1.blue(),
            _ => f.1.magenta(),
        })
        .collect::<Vec<_>>();
    let bat_sonar = BatSonar::new(&file_info_content, SonarResultType::Struct);
    for result in bat_sonar.results {
        println!(
            "Struct found at {}",
            format!(
                "{}:{}",
                struct_file_info.path.clone(),
                result.end_line_index + 1
            )
            .magenta()
        );
        println!("{}", result.content.clone().green());
        let prompt_text = "Select the type of struct:";
        let selection =
            batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
        let struct_type = StructMetadataType::get_structs_type_vec()[selection];
        let struct_metadata = StructMetadata::new(
            struct_file_info.path.clone(),
            result.name.to_string(),
            struct_type,
            result.start_line_index + 1,
            result.end_line_index + 1,
        );
        struct_metadata_vec.push(struct_metadata);
    }
    println!(
        "finishing the review of the {} file",
        struct_file_info.path.clone().blue()
    );
    Ok(struct_metadata_vec)
}
