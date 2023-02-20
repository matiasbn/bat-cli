use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFolder;
use crate::config::BatConfig;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::MarkdownSection;
use crate::batbelt::metadata::source_code_metadata::SourceCodeMetadata;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use error_stack::{Result, ResultExt};
use inflector::Inflector;
use std::{fs, vec};

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
            "# {}\n\n- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.name,
            self.function_type.to_snake_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
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

    pub fn get_functions_metadata_from_program() -> Result<Vec<FunctionMetadata>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files(false, None, None)
            .change_context(MetadataError)?;
        let entrypoints_names = EntrypointParser::get_entrypoints_names(false).unwrap();
        let context_names = EntrypointParser::get_all_contexts_names();
        let mut functions_metadata: Vec<FunctionMetadata> =
            program_dir_entries
                .into_iter()
                .fold(vec![], |mut result_vec, entry| {
                    let entry_path = entry.path().to_str().unwrap().to_string();
                    println!("starting the review of the {} file", entry_path.blue());
                    let file_content = fs::read_to_string(entry.path()).unwrap();
                    let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Function);
                    for result in bat_sonar.results {
                        let function_signature = get_function_signature(&result.content);
                        println!(
                            "Function found at {}\n{}",
                            format!("{}:{}", &entry_path, result.start_line_index + 1).magenta(),
                            function_signature.green()
                        );
                        if entry_path == BatConfig::get_config().unwrap().program_lib_path {
                            if entrypoints_names
                                .clone()
                                .into_iter()
                                .any(|ep_name| ep_name == result.name)
                            {
                                println!("{}", "Function found is Entrypoint type!".yellow());
                                let function_type = FunctionMetadataType::EntryPoint;
                                let function_metadata = FunctionMetadata::new(
                                    entry_path.clone(),
                                    result.name.to_string(),
                                    function_type,
                                    result.start_line_index + 1,
                                    result.end_line_index + 1,
                                );
                                result_vec.push(function_metadata);
                                continue;
                            }
                        }
                        let result_source_code = SourceCodeMetadata::new(
                            result.name.clone(),
                            entry_path.clone(),
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
                                    entry_path.clone(),
                                    result.name.to_string(),
                                    function_type,
                                    result.start_line_index + 1,
                                    result.end_line_index + 1,
                                );
                                result_vec.push(function_metadata);
                                continue;
                            }
                        }
                        let function_type = FunctionMetadataType::Other;
                        let function_metadata = FunctionMetadata::new(
                            entry_path.clone(),
                            result.name.to_string(),
                            function_type,
                            result.start_line_index + 1,
                            result.end_line_index + 1,
                        );
                        result_vec.push(function_metadata);
                    }
                    println!(
                        "finishing the review of the {} file",
                        entry_path.clone().blue()
                    );
                    return result_vec;
                });
        functions_metadata.sort_by(|function_a, function_b| function_a.name.cmp(&function_b.name));
        Ok(functions_metadata)
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
