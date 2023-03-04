use super::*;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile;
use crate::config::BatConfig;
use strum::IntoEnumIterator;

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};

use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType, MetadataResult};
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::parser::trait_parser::TraitParser;
use crate::batbelt::BatEnumerator;
use error_stack::{FutureExt, Result, ResultExt};
use serde::{Deserialize, Serialize};

use serde_json::json;
use std::{fs, vec};
use walkdir::DirEntry;

use super::MetadataError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionSourceCodeMetadata {
    pub path: String,
    pub name: String,
    pub metadata_id: MetadataId,
    pub function_type: FunctionMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser<FunctionMetadataType> for FunctionSourceCodeMetadata {
    fn name(&self) -> String {
        self.name.clone()
    }
    fn path(&self) -> String {
        self.path.clone()
    }
    fn metadata_id(&self) -> MetadataId {
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
        metadata_id: MetadataId,
    ) -> Self {
        Self {
            path,
            name,
            metadata_id,
            function_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }

    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError> {
        let mut metadata_result: Vec<FunctionSourceCodeMetadata> = vec![];
        let entry_path = entry.path().to_str().unwrap().to_string();
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
            let function_metadata = FunctionSourceCodeMetadata::new(
                entry_path.clone(),
                result.name.to_string(),
                function_type,
                result.start_line_index + 1,
                result.end_line_index + 1,
                Self::create_metadata_id(),
            );
            metadata_result.push(function_metadata);
        }

        Ok(metadata_result)
    }
}

impl FunctionSourceCodeMetadata {
    pub fn to_function_parser(&self) -> Result<FunctionParser, MetadataError> {
        FunctionParser::new_from_metadata(self.clone()).change_context(MetadataError)
    }

    fn assert_function_is_entrypoint(
        entry_path: &str,
        sonar_result: SonarResult,
    ) -> MetadataResult<bool> {
        let entrypoints_names = EntrypointParser::get_entrypoint_names(false).unwrap();
        if entry_path == BatConfig::get_config().unwrap().program_lib_path {
            if entrypoints_names
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
            entry_path,
            sonar_result.start_line_index + 1,
            sonar_result.end_line_index + 1,
        );
        let result_content = result_source_code.get_source_code_content();
        let result_parameters = get_function_parameters(result_content);
        if !result_parameters.is_empty() {
            let first_parameter = result_parameters[0].clone();
            if first_parameter.contains("Context")
                && context_names
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
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FunctionMetadataCache {
    dependencies: Vec<MetadataId>,
    external_dependencies: Vec<String>,
}

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum FunctionMetadataType {
    EntryPoint,
    Handler,
    Other,
}

impl BatEnumerator for FunctionMetadataType {}

pub fn get_function_parameters(function_content: String) -> Vec<String> {
    let content_lines = function_content.lines();
    let function_signature = get_function_signature(&function_content);
    //Function parameters
    // single line function
    // info!("function content: \n {}", function_content);
    if content_lines.clone().next().unwrap().contains('{') {
        let function_signature_tokenized = function_signature
            .trim_start_matches("pub (crate) fn ")
            .trim_start_matches("pub fn ")
            .split('(')
            .last()
            .unwrap()
            .trim_end_matches(')')
            .split(' ')
            .collect::<Vec<_>>();
        if function_signature_tokenized.is_empty() || function_signature_tokenized[0].is_empty() {
            return vec![];
        }
        let mut parameters: Vec<String> = vec![];
        function_signature_tokenized
            .iter()
            .enumerate()
            .fold("".to_string(), |total, current| {
                if current.1.contains(':') {
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
            .filter(|line| line.contains(':'))
            .map(|line| line.trim().trim_end_matches(',').to_string())
            .collect();
        filtered
    }
}

pub fn get_function_signature(function_content: &str) -> String {
    let function_signature = function_content.clone();
    let function_signature = function_signature
        .split('{')
        .next()
        .unwrap()
        .split("->")
        .next()
        .unwrap();
    function_signature.trim().to_string()
}

pub fn get_function_body(function_content: &str) -> String {
    let function_body = function_content.clone();
    let mut body = function_body.split('{');
    body.next();
    let body = body.collect::<Vec<_>>().join("{");
    body.trim_end_matches('}').trim().to_string()
}

#[cfg(debug_assertions)]

mod test_function_metadata {
    use crate::batbelt::metadata::functions_source_code_metadata::{
        get_function_body, get_function_parameters, get_function_signature, FunctionMetadataCache,
        FunctionMetadataType, FunctionSourceCodeMetadata,
    };
    use serde_json::{json, Value};
    use std::fs;

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
        let expected_function_body =
            "let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
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
    fn test_handle_cache() {
        let test_path = "./test.json";
        let metadata_id = "1234";
        let dependencies = vec!["asdasd".to_string()];
        let external_dependencies = vec!["asdasdasidhasjd".to_string()];
        let function_metadata_cache = FunctionMetadataCache {
            dependencies,
            external_dependencies,
        };
        let json = json!({ metadata_id: function_metadata_cache });
        println!("{}", json);
        let pretty = serde_json::to_string_pretty(&json).unwrap();
        assert_fs::NamedTempFile::new(test_path).unwrap();
        fs::write(test_path, &pretty).unwrap();

        let read_value = fs::read_to_string(test_path).unwrap();
        let value: Value = serde_json::from_str(&read_value).unwrap();
        let f_val: FunctionMetadataCache =
            serde_json::from_value(value[metadata_id].clone()).unwrap();

        let test = value["bad_key"].clone();

        println!("fval: {:#?}", f_val);
    }
}
