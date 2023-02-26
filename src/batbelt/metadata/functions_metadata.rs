use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFolder;
use crate::config::BatConfig;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::MarkdownSection;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, BatMetadataType, MetadataMarkdownContent,
};
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::parser::trait_impl_parser::TraitImplParser;
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use std::fmt::Display;
use std::{fs, vec};

use super::MetadataError;

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionMetadata {
    pub path: String,
    pub name: String,
    pub metadata_id: String,
    pub function_type: FunctionMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser for FunctionMetadata {
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
    fn metadata_sub_type_string(&self) -> String {
        self.function_type.to_string()
    }
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
            metadata_id: Self::create_metadata_id(),
            function_type: function_type,
            start_line_index,
            end_line_index,
        }
    }

    pub fn to_function_parser(
        &self,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
        optional_trait_impl_parser_vec: Option<Vec<TraitImplParser>>,
    ) -> Result<FunctionParser, MetadataError> {
        Ok(FunctionParser::new_from_metadata(
            self.clone(),
            optional_function_metadata_vec,
            optional_trait_impl_parser_vec,
        )
        .change_context(MetadataError)?)
    }

    pub fn from_markdown_section(md_section: MarkdownSection) -> Result<Self, MetadataError> {
        let message = format!(
            "Error parsing function_metadata from markdown_section: \n{:#?}",
            md_section
        );
        let name = md_section.section_header.title;
        let path =
            Self::parse_metadata_info_section(&md_section.content, MetadataMarkdownContent::Path)
                .attach_printable(message.clone())?;
        let function_type_string =
            Self::parse_metadata_info_section(&md_section.content, MetadataMarkdownContent::Type)
                .attach_printable(message.clone())?;
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            MetadataMarkdownContent::StartLineIndex,
        )
        .attach_printable(message.clone())?;
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            MetadataMarkdownContent::EndLineIndex,
        )
        .attach_printable(message.clone())?;
        Ok(FunctionMetadata::new(
            path,
            name,
            FunctionMetadataType::from_str(&function_type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        ))
    }

    pub fn prompt_multiselection(
        select_all: bool,
        force_select: bool,
    ) -> Result<Vec<Self>, MetadataError> {
        let (function_metadata_vec, function_metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", "Function".blue());
        let selections = BatDialoguer::multiselect(
            prompt_text.clone(),
            function_metadata_names.clone(),
            Some(&vec![select_all; function_metadata_names.len()]),
            force_select,
        )
        .change_context(MetadataError)?;

        let filtered_vec = function_metadata_vec
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
        return Ok(filtered_vec);
    }

    fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
        let prompt_text = format!("Please select the {}:", "Function type".blue());
        let function_types_colorized = FunctionMetadataType::get_colorized_functions_type_vec();
        let selection = BatDialoguer::select(prompt_text, function_types_colorized.clone(), None)
            .change_context(MetadataError)?;
        let selected_function_type = FunctionMetadataType::get_functions_type_vec()[selection];
        let function_metadata_vec = Self::get_filtered_metadata(None, Some(selected_function_type))
            .change_context(MetadataError)?;
        let function_metadata_names = function_metadata_vec
            .iter()
            .map(|function_metadata| {
                format!(
                    "{}: {}:{}",
                    function_metadata.name.clone(),
                    function_metadata.path.clone(),
                    function_metadata.start_line_index.clone()
                )
            })
            .collect::<Vec<_>>();
        Ok((function_metadata_vec, function_metadata_names))
    }

    pub fn get_filtered_metadata(
        function_name: Option<&str>,
        function_type: Option<FunctionMetadataType>,
    ) -> Result<Vec<FunctionMetadata>, MetadataError> {
        let function_sections =
            BatMetadataType::Function.get_markdown_sections_from_metadata_file()?;

        let filtered_sections = function_sections
            .into_iter()
            .filter(|section| {
                if function_name.is_some()
                    && function_name.clone().unwrap() != section.section_header.title
                {
                    return false;
                };
                if function_type.is_some() {
                    let type_content = MetadataMarkdownContent::Type
                        .get_info_section_content(function_type.unwrap().to_snake_case());
                    log::debug!("type_content\n{:#?}", type_content);
                    if !section.content.contains(&type_content) {
                        return false;
                    }
                };
                return true;
            })
            .collect::<Vec<_>>();
        log::debug!("function_name\n{:#?}", function_name);
        log::debug!("function_type\n{:#?}", function_type);
        log::debug!("filtered_sections\n{:#?}", filtered_sections);
        if filtered_sections.is_empty() {
            let message = format!(
                "Error finding function sections for:\nfunction_name: {:#?}\nfunction_type: {:#?}",
                function_name, function_type
            );
            return Err(Report::new(MetadataError).attach_printable(message));
        }

        let function_metadata_vec = filtered_sections
            .into_iter()
            .map(|section| FunctionMetadata::from_markdown_section(section))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(function_metadata_vec)
    }

    pub fn get_metadata_from_program_files() -> Result<Vec<FunctionMetadata>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
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
                        let result_source_code = SourceCodeParser::new(
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

    fn get_metadata_vec_from_markdown() -> Result<Vec<FunctionMetadata>, MetadataError> {
        let functions_markdown_file =
            BatMetadataType::Function.get_markdown_sections_from_metadata_file()?;
        let functions_metadata = functions_markdown_file
            .into_iter()
            .map(|markdown_section| {
                FunctionMetadata::from_markdown_section(markdown_section.clone())
            })
            .collect::<Result<Vec<FunctionMetadata>, _>>()?;
        Ok(functions_metadata)
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        function_section: MetadataMarkdownContent,
    ) -> Result<String, MetadataError> {
        let section_prefix = function_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .ok_or(MetadataError)
            .into_report()
            .attach_printable(format!(
                "Error parsing info section {:#?}",
                function_section.to_snake_case()
            ))?
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        Ok(data)
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
                FunctionMetadataType::Handler => function_type.to_sentence_case().bright_green(),
                FunctionMetadataType::EntryPoint => function_type.to_sentence_case().bright_blue(),
                FunctionMetadataType::Other => function_type.to_sentence_case().bright_yellow(),
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
