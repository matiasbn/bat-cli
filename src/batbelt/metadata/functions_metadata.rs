use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::config::BatConfig;
use colored::Colorize;
use strum::IntoEnumIterator;

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::{
    BatMetadataMarkdownContent, BatMetadataParser, BatMetadataType, BatMetadataTypeParser,
    MetadataResult,
};
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::parser::trait_impl_parser::TraitImplParser;
use error_stack::{Report, Result, ResultExt};
use inflector::Inflector;

use std::{fs, vec};
use walkdir::DirEntry;

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

impl BatMetadataParser<FunctionMetadataType> for FunctionMetadata {
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
    fn metadata_sub_type(&self) -> FunctionMetadataType {
        self.function_type
    }
    fn get_bat_metadata_type() -> BatMetadataType {
        BatMetadataType::Function
    }

    fn get_bat_file() -> BatFile {
        BatFile::FunctionsMetadataFile
    }
    fn metadata_name() -> String {
        "Function".to_string()
    }

    fn new(
        path: String,
        name: String,
        metadata_sub_type: FunctionMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        Self {
            path,
            name,
            metadata_id: Self::create_metadata_id(),
            function_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }

    fn get_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError> {
        let mut metadata_result: Vec<FunctionMetadata> = vec![];
        let entry_path = entry.path().to_str().unwrap().to_string();
        // println!("starting the review of the {} file", entry_path.blue());
        let file_content = fs::read_to_string(entry.path()).unwrap();
        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Function);
        for result in bat_sonar.results {
            let function_type = if Self::assert_function_is_entrypoint(&entry_path, result.clone())?
            {
                FunctionMetadataType::EntryPoint
            } else if Self::assert_function_is_handler(entry_path.clone(), result.clone())? {
                FunctionMetadataType::Handler
            } else {
                FunctionMetadataType::Other
            };
            let function_metadata = FunctionMetadata::new(
                entry_path.clone(),
                result.name.to_string(),
                function_type,
                result.start_line_index + 1,
                result.end_line_index + 1,
            );
            metadata_result.push(function_metadata);
        }

        Self::update_markdown_from_metadata_vec(metadata_result.clone())?;

        Ok(metadata_result)
    }
}

impl FunctionMetadata {
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

    fn assert_function_is_entrypoint(
        entry_path: &str,
        sonar_result: SonarResult,
    ) -> MetadataResult<bool> {
        let entrypoints_names = EntrypointParser::get_entrypoints_names(false).unwrap();
        if entry_path == BatConfig::get_config().unwrap().program_lib_path {
            if entrypoints_names
                .clone()
                .into_iter()
                .any(|ep_name| ep_name == sonar_result.name)
            {
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    fn assert_function_is_handler(
        entry_path: String,
        sonar_result: SonarResult,
    ) -> MetadataResult<bool> {
        let context_names = EntrypointParser::get_all_contexts_names();
        let result_source_code = SourceCodeParser::new(
            sonar_result.name.clone(),
            entry_path.clone(),
            sonar_result.start_line_index.clone() + 1,
            sonar_result.end_line_index.clone() + 1,
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
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum FunctionMetadataType {
    EntryPoint,
    Handler,
    Other,
}

impl BatMetadataTypeParser for FunctionMetadataType {}

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
