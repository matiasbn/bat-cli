use crate::batbelt;
use crate::batbelt::entrypoint::EntrypointParser;
use crate::batbelt::path::FolderPathType;
use crate::config::BatConfig;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel};
use crate::batbelt::metadata::source_code::SourceCodeMetadata;
use crate::batbelt::metadata::{get_metadata_markdown, MetadataSection};
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::batbelt::structs::FileInfo;
use error_stack::{Result, ResultExt};
use inflector::Inflector;
use std::vec;

use super::MetadataError;

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

    pub fn get_functions_metadata_section() -> Result<MarkdownSection, MetadataError> {
        let metadata_markdown = get_metadata_markdown()?;
        let functions_section = metadata_markdown
            .get_section(&MetadataSection::Functions.to_string())
            .change_context(MetadataError)?;
        Ok(functions_section)
    }

    pub fn functions_metadata_is_initialized() -> Result<bool, MetadataError> {
        let metadata_markdown = get_metadata_markdown()?;
        let functions_section = metadata_markdown
            .get_section(&MetadataSection::Structs.to_string())
            .change_context(MetadataError)?;
        // // check if empty
        let functions_subsections =
            metadata_markdown.get_section_subsections(functions_section.clone());
        let is_initialized =
            !functions_section.content.is_empty() || functions_subsections.len() > 0;
        Ok(is_initialized)
    }

    pub fn get_functions_metadata_by_type(
        function_type: FunctionMetadataType,
    ) -> Result<Vec<FunctionMetadata>, MetadataError> {
        let functions_metadata = Self::get_functions_metadata_from_metadata_file()?;
        Ok(functions_metadata
            .into_iter()
            .filter(|function_metadata| function_metadata.function_type == function_type)
            .collect::<Vec<_>>())
    }

    pub fn get_functions_metadata_names_by_type(
        function_type: FunctionMetadataType,
    ) -> Result<Vec<String>, MetadataError> {
        let functions_metadata = Self::get_functions_metadata_from_metadata_file()?;
        Ok(functions_metadata
            .into_iter()
            .filter_map(|function_metadata| {
                if function_metadata.function_type == function_type {
                    Some(function_metadata.name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>())
    }

    pub fn get_functions_markdown_sections_from_metadata_file(
    ) -> Result<Vec<MarkdownSection>, MetadataError> {
        let metadata_markdown = batbelt::metadata::get_metadata_markdown()?;
        let functions_section = metadata_markdown
            .get_section(&MetadataSection::Functions.to_sentence_case())
            .change_context(MetadataError)?;
        let functions_subsections = metadata_markdown.get_section_subsections(functions_section);
        Ok(functions_subsections)
    }

    pub fn get_functions_metadata_from_metadata_file(
    ) -> Result<Vec<FunctionMetadata>, MetadataError> {
        let functions_subsections = Self::get_functions_markdown_sections_from_metadata_file()?;
        let functions_sourcecodes = functions_subsections
            .into_iter()
            .map(|subsection| FunctionMetadata::from_markdown_section(subsection))
            .collect::<Vec<FunctionMetadata>>();
        Ok(functions_sourcecodes)
    }

    pub fn get_functions_sourcecodes_from_metadata_file(
    ) -> Result<Vec<SourceCodeMetadata>, MetadataError> {
        let functions_subsections = Self::get_functions_markdown_sections_from_metadata_file()?;
        let functions_sourcecodes = functions_subsections
            .into_iter()
            .map(|subsection| FunctionMetadata::from_markdown_section(subsection))
            .map(|struct_metadata| {
                SourceCodeMetadata::new(
                    struct_metadata.name,
                    struct_metadata.path,
                    struct_metadata.start_line_index,
                    struct_metadata.end_line_index,
                )
            })
            .collect::<Vec<SourceCodeMetadata>>();
        Ok(functions_sourcecodes)
    }

    pub fn to_source_code(&self, optional_name: Option<String>) -> SourceCodeMetadata {
        SourceCodeMetadata::new(
            if let Some(function_name) = optional_name {
                function_name
            } else {
                self.name.clone()
            },
            self.path.clone(),
            self.start_line_index,
            self.end_line_index,
        )
    }

    pub fn get_markdown_section(&self, section_hash: &str) -> MarkdownSection {
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
    EntryPoint,
    Handler,
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
                FunctionMetadataType::Other => function_type.to_sentence_case().magenta(),
                _ => unimplemented!("color no implemented for given type"),
            })
            .collect::<Vec<_>>();
        functions_type_colorized
    }
}

pub fn get_function_parameters(function_content: String) -> Vec<String> {
    let content_lines = function_content.lines();
    let function_signature = get_function_signature(&function_content);
    //Function parameters
    // single line function
    // info!("function content: \n {}", function_content);
    if content_lines.clone().next().unwrap().contains("{") {
        let function_signature_tokenized = function_signature
            .trim_start_matches("pub (crate) fn ")
            .trim_start_matches("pub fn ")
            .split("(")
            .last()
            .unwrap()
            .trim_end_matches(")")
            .split(" ")
            .collect::<Vec<_>>();
        if function_signature_tokenized.is_empty() || function_signature_tokenized[0].is_empty() {
            return vec![];
        }
        let mut parameters: Vec<String> = vec![];
        function_signature_tokenized
            .iter()
            .enumerate()
            .fold("".to_string(), |total, current| {
                if current.1.contains(":") {
                    if !total.is_empty() {
                        parameters.push(total);
                    }
                    current.1.to_string()
                } else if current.0 == function_signature_tokenized.len() - 1 {
                    parameters.push(format!("{} {}", total, current.1));
                    total
                } else {
                    format!("{} {}", total, current.1)
                }
            });
        parameters
    } else {
        //multiline
        // parameters contains :
        let filtered: Vec<String> = function_signature
            .lines()
            .filter(|line| line.contains(":"))
            .map(|line| line.trim().trim_end_matches(",").to_string())
            .collect();
        filtered
    }
}

pub fn get_function_signature(function_content: &str) -> String {
    let function_signature = function_content.clone();
    let function_signature = function_signature
        .split("{")
        .next()
        .unwrap()
        .split("->")
        .next()
        .unwrap();
    function_signature.trim().to_string()
}

pub fn get_function_body(function_content: &str) -> String {
    let function_body = function_content.clone();
    let mut body = function_body.split("{");
    body.next();
    let body = body.collect::<Vec<_>>().join("{");
    body.trim_end_matches("}").trim().to_string()
}

pub fn get_functions_metadata_from_program() -> Result<Vec<FunctionMetadata>, MetadataError> {
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false)
        .change_context(MetadataError)?;
    let program_folder_files_info = batbelt::helpers::get::get_only_files_from_folder(program_path)
        .change_context(MetadataError)?;
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
) -> Result<Vec<FunctionMetadata>, MetadataError> {
    let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
    println!(
        "starting the review of the {} file",
        function_file_info.path.clone().blue()
    );
    let file_info_content = function_file_info.read_content().unwrap();
    // let function_types_colored = FunctionMetadataType::get_colorized_functions_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Function);
    let entrypoints_names = EntrypointParser::get_entrypoints_names(false).unwrap();
    let context_names = EntrypointParser::get_all_contexts_names();
    for result in bat_sonar.results {
        let function_signature = get_function_signature(&result.content);
        println!(
            "Function found at {}\n{}",
            format!(
                "{}:{}",
                function_file_info.path.clone(),
                result.start_line_index + 1,
            )
            .magenta(),
            function_signature.green()
        );
        if function_file_info.path
            == BatConfig::get_validated_config()
                .unwrap()
                .required
                .program_lib_path
        {
            if entrypoints_names
                .clone()
                .into_iter()
                .any(|ep_name| ep_name == result.name)
            {
                println!("{}", "Function found is Entrypoint type!".yellow());
                let function_type = FunctionMetadataType::EntryPoint;
                let function_metadata = FunctionMetadata::new(
                    function_file_info.path.clone(),
                    result.name.to_string(),
                    function_type,
                    result.start_line_index + 1,
                    result.end_line_index + 1,
                );
                function_metadata_vec.push(function_metadata);
                continue;
            }
        }
        let result_source_code = SourceCodeMetadata::new(
            result.name.clone(),
            function_file_info.path.clone(),
            result.start_line_index.clone() + 1,
            result.end_line_index.clone() + 1,
        );
        let result_content = result_source_code.get_source_code_content();
        let result_parameters = get_function_parameters(result_content.clone());
        if !result_parameters.is_empty() {
            let first_parameter = result_parameters[0].clone();
            if first_parameter.contains("Context")
                && context_names
                    .clone()
                    .into_iter()
                    .any(|cx_name| first_parameter.contains(&cx_name))
            {
                println!("{}", "Function found is Handler type!".yellow());
                let function_type = FunctionMetadataType::Handler;
                let function_metadata = FunctionMetadata::new(
                    function_file_info.path.clone(),
                    result.name.to_string(),
                    function_type,
                    result.start_line_index + 1,
                    result.end_line_index + 1,
                );
                function_metadata_vec.push(function_metadata);
                continue;
            }
        }
        // let prompt_text = "Select the function type:";
        // let selection =
        //     batbelt::cli_inputs::select(prompt_text, function_types_colored.clone(), None)?;
        let function_type = FunctionMetadataType::Other;
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

#[test]

fn test_function_parse() {
    let test_function = "pub(crate) fn get_function_metadata_from_file_info() -> Result<Vec<FunctionMetadata>, String> {
    let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
    let file_info_content = function_file_info.read_content().unwrap();
    let function_types_colored = FunctionMetadataType::get_colorized_functions_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Function);
    for result in bat_sonar.results {
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
    Ok(function_metadata_vec)
}";
    let expected_function_signature = "pub(crate) fn get_function_metadata_from_file_info(
    function_file_info: FileInfo,
    function_file_info2: FileInfo2,
)";
    let expected_function_parameters = vec![
        "function_file_info: FileInfo".to_string(),
        "function_file_info2: FileInfo2".to_string(),
    ];
    let expected_function_body = "let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
    let file_info_content = function_file_info.read_content().unwrap();
    let function_types_colored = FunctionMetadataType::get_colorized_functions_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Function);
    for result in bat_sonar.results {
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
    Ok(function_metadata_vec)";
    let function_parameters = get_function_parameters(test_function.to_string());
    assert_eq!(
        expected_function_parameters, function_parameters,
        "wrong parameters"
    );
    let function_body = get_function_body(test_function);
    assert_eq!(expected_function_body, function_body, "wrong body");
    let function_signature = get_function_signature(test_function);
    assert_eq!(
        expected_function_signature, function_signature,
        "wrong signature"
    );
}

#[test]

fn test_parse_handler_in_entrypoint() {}
