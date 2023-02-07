use crate::batbelt::structs::FileInfo;

use crate::batbelt;
use crate::batbelt::path::FolderPathType;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::sonar::{BatSonar, SonarResultType};
use inflector::Inflector;
use std::vec;

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub path: String,
    pub name: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
    pub function_type: FunctionMetadataType,
}

impl FunctionMetadata {
    fn new(
        path: String,
        name: String,
        function_type: FunctionMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        FunctionMetadata {
            path,
            name,
            function_type,
            start_line_index,
            end_line_index,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum FunctionMetadataType {
    Handler,
    EntryPoint,
    Helper,
    Validator,
    Other,
}

impl FunctionMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn get_structs_type_vec() -> Vec<FunctionMetadataType> {
        FunctionMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn from_str(type_str: &str) -> FunctionMetadataType {
        let structs_type_vec = Self::get_structs_type_vec();
        let struct_type = structs_type_vec
            .iter()
            .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
            .unwrap();
        struct_type.clone()
    }

    pub fn get_colorized_structs_type_vec() -> Vec<ColoredString> {
        let struct_type_vec = Self::get_structs_type_vec();
        let structs_type_colorized = struct_type_vec
            .iter()
            .map(|struct_type| match struct_type {
                FunctionMetadataType::Handler => struct_type.to_sentence_case().red(),
                FunctionMetadataType::EntryPoint => struct_type.to_sentence_case().yellow(),
                FunctionMetadataType::Helper => struct_type.to_sentence_case().green(),
                FunctionMetadataType::Validator => struct_type.to_sentence_case().blue(),
                FunctionMetadataType::Other => struct_type.to_sentence_case().magenta(),
                _ => unimplemented!("color no implemented for given type"),
            })
            .collect::<Vec<_>>();
        structs_type_colorized
    }
}

pub fn get_functions_metadata_from_program() -> Result<Vec<FunctionMetadata>, String> {
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
    let mut structs_metadata: Vec<FunctionMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut struct_metadata_result = get_function_metadata_from_file_info(file_info)?;
        structs_metadata.append(&mut struct_metadata_result);
    }
    structs_metadata.sort_by(|struct_a, struct_b| struct_a.name.cmp(&struct_b.name));
    Ok(structs_metadata)
}

pub fn get_function_metadata_from_file_info(
    struct_file_info: FileInfo,
) -> Result<Vec<FunctionMetadata>, String> {
    let mut struct_metadata_vec: Vec<FunctionMetadata> = vec![];
    println!(
        "starting the review of the {} file",
        struct_file_info.path.clone().blue()
    );
    let file_info_content = struct_file_info.read_content().unwrap();
    let struct_types_colored = FunctionMetadataType::get_colorized_structs_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Struct);
    for result in bat_sonar.results {
        println!(
            "Function found at {}\n{}",
            format!(
                "{}:{}",
                struct_file_info.path.clone(),
                result.start_line_index + 1,
            )
            .magenta(),
            result.content.clone().green()
        );
        let prompt_text = "Select the function type:";
        let selection =
            batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
        let struct_type = FunctionMetadataType::get_structs_type_vec()[selection];
        let struct_metadata = FunctionMetadata::new(
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
