use crate::batbelt;
use crate::batbelt::path::FolderPathType;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel};
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::batbelt::structs::FileInfo;
use inflector::Inflector;
use std::vec;

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
enum FunctionMetadataInfoSection {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
}

impl FunctionMetadataInfoSection {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }
}

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub path: String,
    pub name: String,
    pub function_type: FunctionMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
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

    pub fn get_markdown_section_content_string(&self) -> String {
        format!(
            "- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.function_type.to_snake_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
    }

    pub fn get_markdown_section(&self, section_hash: &str) -> MarkdownSection {
        let section_level_header = MarkdownSectionLevel::H2.get_header(&self.name);
        let section_header = MarkdownSectionHeader::new_from_header_and_hash(
            section_level_header,
            section_hash.to_string(),
        );
        let md_section = MarkdownSection::new(
            section_header,
            self.get_markdown_section_content_string(),
            0,
            0,
        );
        md_section
    }

    pub fn from_markdown_section(md_section: MarkdownSection) -> Self {
        let name = md_section.section_header.title;
        let path = Self::parse_metadata_info_section(
            &md_section.content,
            FunctionMetadataInfoSection::Path,
        );
        let function_type_string = Self::parse_metadata_info_section(
            &md_section.content,
            FunctionMetadataInfoSection::Type,
        );
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            FunctionMetadataInfoSection::StartLineIndex,
        );
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            FunctionMetadataInfoSection::EndLineIndex,
        );
        FunctionMetadata::new(
            path,
            name,
            FunctionMetadataType::from_str(&function_type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        )
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        function_section: FunctionMetadataInfoSection,
    ) -> String {
        let section_prefix = function_section.get_prefix();
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

    pub fn get_functions_type_vec() -> Vec<FunctionMetadataType> {
        FunctionMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn from_str(type_str: &str) -> FunctionMetadataType {
        let functions_type_vec = Self::get_functions_type_vec();
        let function_type = functions_type_vec
            .iter()
            .find(|function_type| function_type.to_snake_case() == type_str.to_snake_case())
            .unwrap();
        function_type.clone()
    }

    pub fn get_colorized_functions_type_vec() -> Vec<ColoredString> {
        let function_type_vec = Self::get_functions_type_vec();
        let functions_type_colorized = function_type_vec
            .iter()
            .map(|function_type| match function_type {
                FunctionMetadataType::Handler => function_type.to_sentence_case().red(),
                FunctionMetadataType::EntryPoint => function_type.to_sentence_case().yellow(),
                FunctionMetadataType::Helper => function_type.to_sentence_case().green(),
                FunctionMetadataType::Validator => function_type.to_sentence_case().blue(),
                FunctionMetadataType::Other => function_type.to_sentence_case().magenta(),
                _ => unimplemented!("color no implemented for given type"),
            })
            .collect::<Vec<_>>();
        functions_type_colorized
    }
}

pub fn get_functions_metadata_from_program() -> Result<Vec<FunctionMetadata>, String> {
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
    let mut functions_metadata: Vec<FunctionMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut function_metadata_result = get_function_metadata_from_file_info(file_info)?;
        functions_metadata.append(&mut function_metadata_result);
    }
    functions_metadata.sort_by(|function_a, function_b| function_a.name.cmp(&function_b.name));
    Ok(functions_metadata)
}

pub fn get_function_metadata_from_file_info(
    function_file_info: FileInfo,
) -> Result<Vec<FunctionMetadata>, String> {
    let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
    println!(
        "starting the review of the {} file",
        function_file_info.path.clone().blue()
    );
    let file_info_content = function_file_info.read_content().unwrap();
    let function_types_colored = FunctionMetadataType::get_colorized_functions_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Function);
    for result in bat_sonar.results {
        println!(
            "Function found at {}\n{}",
            format!(
                "{}:{}",
                function_file_info.path.clone(),
                result.start_line_index + 1,
            )
            .magenta(),
            result.content.clone().green()
        );
        let prompt_text = "Select the function type:";
        let selection =
            batbelt::cli_inputs::select(prompt_text, function_types_colored.clone(), None)?;
        let function_type = FunctionMetadataType::get_functions_type_vec()[selection];
        let function_metadata = FunctionMetadata::new(
            function_file_info.path.clone(),
            result.name.to_string(),
            function_type,
            result.start_line_index + 1,
            result.end_line_index + 1,
        );
        function_metadata_vec.push(function_metadata);
    }
    println!(
        "finishing the review of the {} file",
        function_file_info.path.clone().blue()
    );
    Ok(function_metadata_vec)
}
