pub mod functions_metadata;
pub mod structs_metadata;
pub mod traits_metadata;

use colored::{ColoredString, Colorize};
use std::error::Error;
use std::fmt;
use std::fmt::{Debug, Display};

use crate::batbelt::markdown::{MarkdownFile, MarkdownSection};

use crate::batbelt::path::BatFile;

use inflector::Inflector;

use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::traits_metadata::TraitMetadata;
use crate::batbelt::parser::parse_formatted_path;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use error_stack::{IntoReport, Report, Result, ResultExt};
use rand::distributions::Alphanumeric;
use rand::Rng;
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Metadata error")
    }
}

impl Error for MetadataError {}

pub struct BatMetadata;

impl BatMetadata {
    pub fn metadata_is_initialized() -> Result<bool, MetadataError> {
        let mut metadata_initialized = true;
        for metadata_type in BatMetadataType::get_metadata_type_vec() {
            let section_initialized = metadata_type.is_initialized()?;
            if !section_initialized {
                metadata_initialized = false;
            }
        }
        Ok(metadata_initialized)
    }

    pub fn check_metadata_is_initialized() -> Result<(), MetadataError> {
        let metadata_types = BatMetadataType::get_metadata_type_vec();
        for metadata_type in metadata_types {
            metadata_type.check_is_initialized()?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatMetadataType {
    Struct,
    Function,
    Trait,
}

impl BatMetadataTypeParser for BatMetadataType {}

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

    pub fn get_markdown(&self) -> Result<MarkdownFile, MetadataError> {
        let file_path = self.get_path()?;
        log::debug!("markdown file path: {}", file_path);
        let markdown_file = MarkdownFile::new(&file_path);
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
            return Err(Report::new(MetadataError).attach_printable(format!(
                "{} metadata is required to be initialized to execute this action",
                self.to_string().red()
            )));
        }
        Ok(())
    }
    pub fn prompt_metadata_type_selection() -> Result<Self, MetadataError> {
        let metadata_types_vec = BatMetadataType::get_metadata_type_vec();
        let metadata_types_colorized_vec = BatMetadataType::get_colorized_type_vec();
        // Choose metadata section selection
        let prompt_text = format!("Please select the {}", "Metadata type".bright_purple());
        let selection =
            BatDialoguer::select(prompt_text, metadata_types_colorized_vec.clone(), None).unwrap();
        let metadata_type_selected = &metadata_types_vec[selection];
        Ok(metadata_type_selected.clone())
    }

    fn get_metadata_vec_from_markdown<T: BatMetadataParser<BatMetadataType>>(
    ) -> Result<Vec<T>, MetadataError> {
        let metadata_markdown_file =
            BatMetadataType::Trait.get_markdown_sections_from_metadata_file()?;
        let metadata_vec = metadata_markdown_file
            .into_iter()
            .map(|markdown_section| T::from_markdown_section(markdown_section.clone()))
            .collect::<Result<Vec<T>, _>>()?;
        Ok(metadata_vec)
    }
}

pub trait BatMetadataParser<U>
where
    Self: Sized,
    U: BatMetadataTypeParser,
{
    fn name(&self) -> String;
    fn path(&self) -> String;
    fn metadata_id(&self) -> String;
    fn start_line_index(&self) -> usize;
    fn end_line_index(&self) -> usize;
    fn metadata_sub_type(&self) -> U;
    fn match_bat_metadata_type() -> BatMetadataType;

    fn metadata_name() -> String;

    fn new(
        path: String,
        name: String,
        metadata_sub_type: U,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self;

    fn create_metadata_id() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        s
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
            "# {}\n\n- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.name(),
            self.metadata_sub_type().to_snake_case(),
            self.path(),
            self.start_line_index(),
            self.end_line_index()
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
        Ok(Self::new(
            path,
            name,
            U::from_str(&type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        ))
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

    fn prompt_multiselection(
        select_all: bool,
        force_select: bool,
    ) -> Result<Vec<Self>, MetadataError> {
        let (metadata_vec, metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Please select the {}:", Self::metadata_name().blue());
        let selections = BatDialoguer::multiselect(
            prompt_text.clone(),
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
        return Ok(filtered_vec);
    }

    fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
        let prompt_text = format!(
            "Please select the {} {}:",
            Self::metadata_name().blue(),
            "type".blue()
        );
        let selection = BatDialoguer::select(prompt_text, U::get_colorized_type_vec(), None)
            .change_context(MetadataError)?;
        let selected_sub_type = U::get_metadata_type_vec()[selection].clone();
        let metadata_vec_filtered = Self::get_filtered_metadata(None, Some(selected_sub_type))
            .change_context(MetadataError)?;
        let metadata_names = metadata_vec_filtered
            .iter()
            .map(|metadata| {
                parse_formatted_path(
                    metadata.name().clone(),
                    metadata.path().clone(),
                    metadata.start_line_index().clone(),
                )
            })
            .collect::<Vec<_>>();
        Ok((metadata_vec_filtered, metadata_names))
    }

    fn get_filtered_metadata(
        metadata_name: Option<&str>,
        metadata_type: Option<U>,
    ) -> Result<Vec<Self>, MetadataError> {
        let markdown_sections =
            Self::match_bat_metadata_type().get_markdown_sections_from_metadata_file()?;

        let filtered_sections = markdown_sections
            .into_iter()
            .filter(|section| {
                if metadata_name.is_some()
                    && metadata_name.clone().unwrap() != section.section_header.title
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
                return true;
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

pub trait BatMetadataTypeParser
where
    Self: ToString + Display + IntoEnumIterator + Clone + Sized + Debug,
{
    fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    fn from_str(type_str: &str) -> Self {
        let structs_type_vec = Self::get_metadata_type_vec();
        let struct_type = structs_type_vec
            .into_iter()
            .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
            .unwrap();
        struct_type.clone()
    }

    fn get_metadata_type_vec() -> Vec<Self> {
        Self::iter().collect::<Vec<_>>()
    }

    fn colored_from_index(type_str: &str, idx: usize) -> ColoredString {
        match idx {
            0 => type_str.bright_green(),
            1 => type_str.bright_blue(),
            2 => type_str.bright_yellow(),
            3 => type_str.bright_cyan(),
            4 => type_str.bright_purple(),
            _ => type_str.bright_white(),
        }
    }

    fn get_colorized_type_vec() -> Vec<ColoredString> {
        let metadata_type_vec = Self::get_metadata_type_vec();
        let metadata_type_colorized = metadata_type_vec
            .iter()
            .enumerate()
            .map(|metadata_type| {
                Self::colored_from_index(
                    &(*metadata_type.1).to_sentence_case().clone(),
                    metadata_type.0,
                )
            })
            .collect::<Vec<_>>();
        metadata_type_colorized
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatMetadataMarkdownContent {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
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
