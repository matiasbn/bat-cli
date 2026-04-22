use super::*;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType};
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::syn_struct_classifier;

use crate::batbelt::BatEnumerator;
use error_stack::{Result, ResultExt};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

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
    #[serde(default)]
    pub program_name: String,
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
        use crate::batbelt::metadata::derive_program_name_from_path;
        let program_name = derive_program_name_from_path(&path);
        Self {
            path,
            name,
            metadata_id,
            function_type: metadata_sub_type,
            start_line_index,
            end_line_index,
            program_name,
        }
    }

    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError> {
        let mut metadata_result: Vec<FunctionSourceCodeMetadata> = vec![];
        let entry_path = entry.path().to_str().unwrap().to_string();
        let file_content = fs::read_to_string(entry.path()).unwrap();
        let classification = syn_struct_classifier::classify_file(&file_content);
        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Function);
        for result in bat_sonar.results {
            let function_type =
                if classification.entrypoint_function_names.contains(&result.name) {
                    FunctionMetadataType::EntryPoint
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
    pub fn create_metadata_from_content(
        entry_path: &str,
        file_content: &str,
    ) -> Result<Vec<Self>, MetadataError> {
        let mut metadata_result: Vec<FunctionSourceCodeMetadata> = vec![];
        let classification = syn_struct_classifier::classify_file(file_content);
        let bat_sonar = BatSonar::new_scanned(file_content, SonarResultType::Function);
        for result in bat_sonar.results {
            let function_type =
                if classification.entrypoint_function_names.contains(&result.name) {
                    FunctionMetadataType::EntryPoint
                } else {
                    FunctionMetadataType::Other
                };
            let function_metadata = FunctionSourceCodeMetadata::new(
                entry_path.to_string(),
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

    pub fn to_function_parser(&self) -> Result<FunctionParser, MetadataError> {
        FunctionParser::new_from_metadata(self.clone()).change_context(MetadataError)
    }

    pub fn prompt_selection() -> Result<Self, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selection = BatDialoguer::select(prompt_text, metadata_names, None)
            .change_context(MetadataError)?;

        Ok(metadata_vec[selection].clone())
    }

    pub fn prompt_multiselection(
        select_all: bool,
        force_select: bool,
    ) -> Result<Vec<Self>, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selections = BatDialoguer::multiselect(
            prompt_text,
            metadata_names.clone(),
            Some(&vec![select_all; metadata_names.len()]),
            force_select,
        )
        .change_context(MetadataError)?;

        let filtered_vec = metadata_vec
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
        Ok(filtered_vec)
    }

    pub fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
        let prompt_text = format!(
            "Please select the {} {}:",
            Self::metadata_name().blue(),
            "type".blue()
        );
        let selection = BatDialoguer::select(
            prompt_text,
            FunctionMetadataType::get_colorized_type_vec(true),
            None,
        )
        .change_context(MetadataError)?;
        let selected_sub_type = FunctionMetadataType::get_type_vec()[selection];
        let metadata_vec_filtered =
            SourceCodeMetadata::get_filtered_functions(None, Some(selected_sub_type))
                .change_context(MetadataError)?;
        let metadata_names = metadata_vec_filtered
            .iter()
            .map(|metadata| {
                parse_formatted_path(
                    metadata.name(),
                    metadata.path(),
                    metadata.start_line_index(),
                )
            })
            .collect::<Vec<_>>();
        Ok((metadata_vec_filtered, metadata_names))
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
    /// Legacy variant kept for backwards compatibility with existing
    /// `BatMetadata.json` files that contain `"Handler"`.
    /// New scans no longer classify functions as Handler.
    Handler,
    Other,
}

impl BatEnumerator for FunctionMetadataType {
    fn get_type_vec() -> Vec<Self> {
        Self::iter()
            .filter(|v| !matches!(v, FunctionMetadataType::Handler))
            .collect()
    }
}

pub fn get_function_parameters(function_content: String) -> Vec<String> {
    use quote::ToTokens;

    let item_fn = syn::parse_str::<syn::ItemFn>(&function_content).or_else(|_| {
        let wrapped = format!("fn __wrapper() {{ {} }}", function_content);
        syn::parse_str::<syn::ItemFn>(&wrapped)
    });

    let Ok(item_fn) = item_fn else {
        // Fallback to legacy string parsing if syn fails
        return get_function_parameters_legacy(function_content);
    };

    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => {
                let name = pat_type.pat.to_token_stream().to_string();
                let ty = pat_type.ty.to_token_stream().to_string();
                Some(format!("{}: {}", name, ty))
            }
        })
        .collect()
}

fn get_function_parameters_legacy(function_content: String) -> Vec<String> {
    let content_lines = function_content.lines();
    let function_signature = get_function_signature(&function_content);
    if content_lines.clone().next().unwrap().contains('{') {
        let function_signature_tokenized = function_signature
            .trim_start_matches("pub (crate) fn ")
            .trim_start_matches("pub fn ")
            .split('(')
            .next_back()
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
        let filtered: Vec<String> = function_signature
            .lines()
            .filter(|line| line.contains(':'))
            .map(|line| line.trim().trim_end_matches(',').to_string())
            .collect();
        filtered
    }
}

pub fn get_function_signature(function_content: &str) -> String {
    let function_signature = function_content;
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
    let function_body = function_content;
    let mut body = function_body.split('{');
    body.next();
    let body = body.collect::<Vec<_>>().join("{");
    body.trim_end_matches('}').trim().to_string()
}
