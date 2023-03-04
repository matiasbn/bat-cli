pub mod entrypoint_metadata;
pub mod function_dependencies_metadata;
pub mod functions_metadata;
pub mod structs_metadata;
pub mod traits_metadata;

use colored::Colorize;
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display};
use std::path::Path;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSection};

use crate::batbelt::path::{BatFile, BatFolder};

use inflector::Inflector;

use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::function_dependencies_metadata::FunctionDependenciesMetadata;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::traits_metadata::TraitMetadata;
use crate::batbelt::parser::parse_formatted_path;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::BatEnumerator;
use crate::Suggestion;
use error_stack::{FutureExt, IntoReport, Report, Result, ResultExt};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value::Null;
use serde_json::{json, Value};
use strum::IntoEnumIterator;
use walkdir::DirEntry;

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Metadata error")
    }
}

impl Error for MetadataError {}

pub type MetadataResult<T> = Result<T, MetadataError>;

pub type MetadataId = String;

enum MetadataErrorReports {
    MetadataIdNotFound { metadata_id: MetadataId },
    EntryPointsMetadataNotInitialized,
    EntryPointNameNotFound { entry_point_name: String },
    FunctionDependenciesMetadataNotInitialized,
    FunctionDependenciesNotFound { function_metadata_id: MetadataId },
}

impl MetadataErrorReports {
    pub fn get_error_report(&self) -> Report<MetadataError> {
        let message = match self {
            MetadataErrorReports::MetadataIdNotFound { metadata_id } => {
                format!("Metadata not found for {}", metadata_id.red())
            }
            MetadataErrorReports::EntryPointsMetadataNotInitialized => {
                format!("Entry point metadata has not been initialized")
            }
            MetadataErrorReports::EntryPointNameNotFound { entry_point_name } => {
                format!(
                    "Entry point metadata not found for {}",
                    entry_point_name.red()
                )
            }
            MetadataErrorReports::FunctionDependenciesMetadataNotInitialized => {
                format!("Function dependencies metadata has not been initialized")
            }
            MetadataErrorReports::FunctionDependenciesNotFound {
                function_metadata_id,
            } => {
                format!(
                    "Entry point metadata not found for {} id",
                    function_metadata_id.red()
                )
            }
        };
        Report::new(MetadataError).attach_printable(message)
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BatMetadata {
    pub source_code: SourceCodeMetadata,
    pub entry_points: Vec<EntrypointMetadata>,
    pub function_dependencies: Vec<FunctionDependenciesMetadata>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SourceCodeMetadata {
    pub initialized: bool,
    pub functions: Vec<FunctionMetadata>,
    pub structs: Vec<StructMetadata>,
    pub traits: Vec<TraitMetadata>,
}

impl SourceCodeMetadata {
    pub fn get_function_by_id(&self, metadata_id: MetadataId) -> MetadataResult<FunctionMetadata> {
        let result = self
            .functions
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        return match result {
            Some(f_metadata) => Ok(f_metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        };
    }

    pub fn get_struct_by_id(&self, metadata_id: MetadataId) -> MetadataResult<StructMetadata> {
        let result = self
            .structs
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        return match result {
            Some(metadata) => Ok(metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        };
    }

    pub fn get_trait_by_id(&self, metadata_id: MetadataId) -> MetadataResult<TraitMetadata> {
        let result = self
            .traits
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        return match result {
            Some(metadata) => Ok(metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        };
    }

    pub fn update_functions(&self, new_vec: Vec<FunctionMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        bat_metadata.source_code.initialized = true;
        let mut metadata_vec = new_vec.clone();
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.functions = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }

    pub fn update_structs(&self, new_vec: Vec<StructMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        bat_metadata.source_code.initialized = true;
        let mut metadata_vec = new_vec.clone();
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.structs = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }
    pub fn update_traits(&self, new_vec: Vec<TraitMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        bat_metadata.source_code.initialized = true;
        let mut metadata_vec = new_vec.clone();
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.traits = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }
}

impl BatMetadata {
    pub fn new_empty() -> Self {
        Self {
            source_code: SourceCodeMetadata {
                initialized: false,
                functions: vec![],
                structs: vec![],
                traits: vec![],
            },
            entry_points: vec![],
            function_dependencies: vec![],
        }
    }

    pub fn create_metadata_id() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        s
    }

    pub fn read_metadata() -> MetadataResult<Self> {
        let metadata_json_bat_file = BatFile::MetadataJsonFile;
        let bat_metadata_value: Value = serde_json::from_str(
            &metadata_json_bat_file
                .read_content(true)
                .change_context(MetadataError)?,
        )
        .into_report()
        .change_context(MetadataError)?;
        let bat_metadata: BatMetadata = serde_json::from_value(bat_metadata_value)
            .into_report()
            .change_context(MetadataError)?;
        Ok(bat_metadata)
    }

    pub fn save_metadata(&self) -> MetadataResult<()> {
        let metadata_json_bat_file = BatFile::MetadataJsonFile;
        metadata_json_bat_file
            .create_empty(false)
            .change_context(MetadataError)?;
        let metadata_json = json!(&self);
        let metadata_json_pretty = serde_json::to_string_pretty(&metadata_json)
            .into_report()
            .change_context(MetadataError)?;
        metadata_json_bat_file
            .write_content(false, &metadata_json_pretty)
            .change_context(MetadataError)?;
        Ok(())
    }

    pub fn get_entrypoint_metadata_by_name(
        &self,
        entry_point_name: String,
    ) -> MetadataResult<EntrypointMetadata> {
        if self.entry_points.is_empty() {
            return Err(MetadataErrorReports::EntryPointsMetadataNotInitialized.get_error_report());
        }
        match self
            .entry_points
            .clone()
            .into_iter()
            .find(|ep| ep.name == entry_point_name)
        {
            None => Err(
                MetadataErrorReports::EntryPointNameNotFound { entry_point_name }
                    .get_error_report(),
            ),
            Some(ep) => Ok(ep),
        }
    }

    pub fn get_functions_dependencies_metadata_by_function_metadata_id(
        &self,
        function_metadata_id: String,
    ) -> MetadataResult<FunctionDependenciesMetadata> {
        if self.function_dependencies.is_empty() {
            return Err(
                MetadataErrorReports::FunctionDependenciesMetadataNotInitialized.get_error_report(),
            );
        }
        match self
            .function_dependencies
            .clone()
            .into_iter()
            .find(|ep| ep.function_metadata_id == function_metadata_id)
        {
            None => Err(MetadataErrorReports::FunctionDependenciesNotFound {
                function_metadata_id,
            }
            .get_error_report()),
            Some(metadata) => Ok(metadata),
        }
    }

    pub fn metadata_is_initialized() -> Result<bool, MetadataError> {
        let mut metadata_initialized = true;
        for metadata_type in BatMetadataType::get_type_vec() {
            let section_initialized = metadata_type.is_initialized()?;
            if !section_initialized {
                metadata_initialized = false;
            }
        }
        Ok(metadata_initialized)
    }

    pub fn check_metadata_is_initialized() -> Result<(), MetadataError> {
        let metadata_types = BatMetadataType::get_type_vec();
        // check file exists
        for metadata_type in metadata_types.clone() {
            if !Path::new(&metadata_type.get_path()?).is_file() {
                return Err(Report::new(MetadataError).attach_printable(format!(
                    "Metadata file not found: {}",
                    metadata_type.get_path()?
                )));
            }
        }
        // check markdown exists
        for metadata_type in metadata_types {
            metadata_type.check_is_initialized()?;
        }
        Ok(())
    }
}

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    Default,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum BatMetadataType {
    #[default]
    Struct,
    Function,
    Trait,
}

impl BatMetadataType {
    pub fn get_path(&self) -> Result<String, MetadataError> {
        let path = match self {
            BatMetadataType::Struct => BatFile::StructsMetadataFile
                .get_path(false)
                .change_context(MetadataError)?,
            BatMetadataType::Function => BatFile::FunctionsMetadataFile
                .get_path(false)
                .change_context(MetadataError)?,
            BatMetadataType::Trait => BatFile::TraitsMetadataFile
                .get_path(false)
                .change_context(MetadataError)?,
        };
        Ok(path)
    }

    pub fn get_cache_file(&self) -> BatFile {
        BatFile::MetadataCacheFile {
            metadata_cache_type: *self,
        }
    }

    pub fn get_markdown(&self) -> Result<MarkdownFile, MetadataError> {
        let file_path = self.get_path()?;
        log::debug!("markdown file path: {}", file_path);
        let markdown_file = MarkdownFile::new(&file_path).change_context(MetadataError)?;
        Ok(markdown_file)
    }

    pub fn get_markdown_sections_from_metadata_file(
        &self,
    ) -> Result<Vec<MarkdownSection>, MetadataError> {
        let markdown_file = self.get_markdown()?;
        if markdown_file.sections.is_empty() {
            return Err(Report::new(MetadataError)
                .attach_printable(format!("Markdown file is empty:\n{:#?}", markdown_file)));
        }
        Ok(markdown_file.sections)
    }

    pub fn is_initialized(&self) -> Result<bool, MetadataError> {
        let markdown = self.get_markdown()?;
        Ok(!markdown.sections.is_empty())
    }

    pub fn check_is_initialized(&self) -> Result<(), MetadataError> {
        if !self.is_initialized()? {
            return Err(Report::new(MetadataError)
                .attach_printable(format!(
                    "{} metadata is required to be initialized to execute this action",
                    self.to_string().red()
                ))
                .attach(Suggestion(format!(
                    "run {} to initialize the metadata file",
                    "bat-cli sonar".green()
                ))));
        }
        Ok(())
    }
    pub fn prompt_metadata_type_selection() -> Result<Self, MetadataError> {
        let metadata_types_vec = BatMetadataType::get_type_vec();
        let metadata_types_colorized_vec = BatMetadataType::get_colorized_type_vec(true);
        // Choose metadata section selection
        let prompt_text = format!("Please select the {}", "Metadata type".bright_purple());
        let selection =
            BatDialoguer::select(prompt_text, metadata_types_colorized_vec, None).unwrap();
        let metadata_type_selected = &metadata_types_vec[selection];
        Ok(*metadata_type_selected)
    }
}

pub trait BatMetadataParser<U>
where
    Self: Sized + Clone,
    U: BatEnumerator,
{
    fn name(&self) -> String;
    fn path(&self) -> String;
    fn metadata_id(&self) -> MetadataId;
    fn start_line_index(&self) -> usize;
    fn end_line_index(&self) -> usize;
    fn metadata_sub_type(&self) -> U;
    fn get_bat_metadata_type() -> BatMetadataType;
    fn get_bat_file() -> BatFile;

    fn metadata_name() -> String;

    fn value_to_vec_string(value: Value) -> MetadataResult<Vec<String>> {
        Ok(value
            .as_array()
            .ok_or(MetadataError)
            .into_report()?
            .iter()
            .map(|val| val.as_str().ok_or(MetadataError).into_report())
            .collect::<Result<Vec<_>, MetadataError>>()?
            .into_iter()
            .map(|val| val.to_string())
            .collect::<Vec<_>>())
    }

    fn new(
        path: String,
        name: String,
        metadata_sub_type: U,
        start_line_index: usize,
        end_line_index: usize,
        metadata_id: MetadataId,
    ) -> Self;

    fn create_metadata_id() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        s
    }

    fn update_markdown_from_metadata_vec(metadata_vec: &mut Vec<Self>) -> MetadataResult<()> {
        if metadata_vec.is_empty() {
            return Ok(());
        }
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        let metadata_bat_file = Self::get_bat_file();
        let metadata_markdown_content = metadata_bat_file
            .read_content(true)
            .change_context(MetadataError)?;

        let new_markdown_content = metadata_vec
            .iter_mut()
            .map(|metadata| metadata.get_markdown_section_content_string())
            .collect::<Vec<_>>()
            .join("\n\n");

        let result_metadata_content = if metadata_markdown_content.is_empty() {
            new_markdown_content
        } else {
            format!("{}\n\n{}", metadata_markdown_content, new_markdown_content)
        };

        let mut markdown_file = Self::get_bat_metadata_type().get_markdown()?;
        markdown_file.content = result_metadata_content;
        markdown_file.save().change_context(MetadataError)?;

        Ok(())
    }

    fn to_source_code_parser(&self, optional_name: Option<String>) -> SourceCodeParser {
        SourceCodeParser::new(
            if let Some(function_name) = optional_name {
                function_name
            } else {
                self.name()
            },
            self.path(),
            self.start_line_index(),
            self.end_line_index(),
        )
    }

    fn get_markdown_section_content_string(&self) -> String {
        format!(
            "# {}\n\n- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}\n- metadata_id: {}",
            self.name(),
            self.metadata_sub_type().to_snake_case(),
            self.path(),
            self.start_line_index(),
            self.end_line_index(),
            self.metadata_id()
        )
    }

    fn from_markdown_section(md_section: MarkdownSection) -> Result<Self, MetadataError> {
        let message = format!(
            "Error parsing function_metadata from markdown_section: \n{:#?}",
            md_section
        );
        let name = md_section.section_header.title;
        let type_string = Self::parse_metadata_info_section(
            &md_section.content,
            BatMetadataMarkdownContent::Type,
        )
        .attach_printable(message.clone())?;
        let path = Self::parse_metadata_info_section(
            &md_section.content,
            BatMetadataMarkdownContent::Path,
        )
        .attach_printable(message.clone())?;
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            BatMetadataMarkdownContent::StartLineIndex,
        )
        .attach_printable(message.clone())?;
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            BatMetadataMarkdownContent::EndLineIndex,
        )
        .attach_printable(message.clone())?;
        let metadata_id = Self::parse_metadata_info_section(
            &md_section.content,
            BatMetadataMarkdownContent::MetadataId,
        )
        .attach_printable(message)?;
        Ok(Self::new(
            path,
            name,
            U::from_str(&type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
            metadata_id,
        ))
    }
    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError>;

    fn create_metadata_from_program_files() -> Result<Vec<Self>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
            .change_context(MetadataError)?;
        let mut metadata_vec: Vec<Self> = program_dir_entries
            .into_iter()
            .map(|entry| Self::create_metadata_from_dir_entry(entry))
            .collect::<Result<Vec<_>, MetadataError>>()?
            .into_iter()
            .fold(vec![], |mut result_vec, mut entry| {
                result_vec.append(&mut entry);
                result_vec
            });
        metadata_vec.sort_by_key(|function_a| function_a.name());
        Ok(metadata_vec)
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        metadata_section: BatMetadataMarkdownContent,
    ) -> Result<String, MetadataError> {
        let section_prefix = metadata_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .ok_or(MetadataError)
            .into_report()
            .attach_printable(format!(
                "Error parsing info section {}, with metadata_info_content:\n{}",
                metadata_section.to_snake_case(),
                metadata_info_content
            ))?
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        Ok(data)
    }

    fn prompt_selection() -> Result<Self, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selection = BatDialoguer::select(prompt_text, metadata_names, None)
            .change_context(MetadataError)?;

        Ok(metadata_vec[selection].clone())
    }

    fn prompt_multiselection(
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

    fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
        let prompt_text = format!(
            "Please select the {} {}:",
            Self::metadata_name().blue(),
            "type".blue()
        );
        let selection = BatDialoguer::select(prompt_text, U::get_colorized_type_vec(true), None)
            .change_context(MetadataError)?;
        let selected_sub_type = U::get_type_vec()[selection].clone();
        let metadata_vec_filtered = Self::get_filtered_metadata(None, Some(selected_sub_type))
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

    fn find_by_metadata_id(metadata_id: MetadataId) -> MetadataResult<Self> {
        let match_metadata = Self::get_bat_metadata_type()
            .get_markdown_sections_from_metadata_file()?
            .into_iter()
            .map(|section| Self::from_markdown_section(section))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .find(|metadata| metadata.metadata_id() == metadata_id);
        match match_metadata {
            None => Err(Report::new(MetadataError).attach_printable(format!(
                "No match for metadata with metadata_id:{}",
                metadata_id
            ))),
            Some(metadata) => Ok(metadata),
        }
    }

    fn get_filtered_metadata(
        metadata_name: Option<&str>,
        metadata_type: Option<U>,
    ) -> Result<Vec<Self>, MetadataError> {
        let markdown_sections =
            Self::get_bat_metadata_type().get_markdown_sections_from_metadata_file()?;

        let filtered_sections = markdown_sections
            .into_iter()
            .filter(|section| {
                if metadata_name.is_some() && metadata_name.unwrap() != section.section_header.title
                {
                    return false;
                };
                if metadata_type.is_some() {
                    let type_content = BatMetadataMarkdownContent::Type
                        .get_info_section_content(metadata_type.clone().unwrap().to_snake_case());
                    log::debug!("type_content\n{:#?}", type_content);
                    if !section.content.contains(&type_content) {
                        return false;
                    }
                };
                true
            })
            .collect::<Vec<_>>();
        log::debug!("metadata_name\n{:#?}", metadata_name);
        log::debug!("metadata_type\n{:#?}", metadata_type);
        log::debug!("filtered_sections\n{:#?}", filtered_sections);
        if filtered_sections.is_empty() {
            let message = format!(
                "Error finding metadata sections for:\nmetadata_name: {:#?}\nmetadata_type: {:#?}",
                metadata_name, metadata_type
            );
            return Err(Report::new(MetadataError).attach_printable(message));
        }

        let function_metadata_vec = filtered_sections
            .into_iter()
            .map(|section| Self::from_markdown_section(section))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(function_metadata_vec)
    }
}

impl BatEnumerator for BatMetadataType {}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatMetadataMarkdownContent {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
    MetadataId,
}

impl BatMetadataMarkdownContent {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn get_info_section_content<T: Display>(&self, content_value: T) -> String {
        format!("- {}: {}", self.to_snake_case(), content_value)
    }
}

// #[cfg(debug_assertions)]
// mod metadata_test {
//     use assert_fs::prelude::FileWriteStr;
//     use serde_json::{json, Value};
//     use std::fs;
//
//     const TEMP_PATH: &'static str = "./test.json";
//
//     // #[test]
//     // fn test_metadata() {
//     //     //save to json
//     //     let key = "hello";
//     //     let value = vec!["world".to_string()];
//     //     let json_content = json!({ key: value });
//     //
//     //     let pretty_content = serde_json::to_string_pretty(&json_content).unwrap();
//     //     assert_fs::NamedTempFile::new(TEMP_PATH).unwrap();
//     //     fs::write(TEMP_PATH, &pretty_content).unwrap();
//     //
//     //     let vec_value = read_key(key);
//     //     let vec_read = value_to_vec_string(vec_value);
//     //
//     //     assert_eq!(value, vec_read);
//     //
//     //     let value_2 = vec!["chai".to_string()];
//     //     let vec_value = json!(value_2);
//     //     save_key(key, vec_value);
//     //
//     //     let vec_value_read = read_key(key);
//     //     let vec_read = value_to_vec_string(vec_value_read);
//     //
//     //     assert_eq!(vec_read, value_2);
//     // }
// }
