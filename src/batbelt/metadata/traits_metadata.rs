use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFolder;
use crate::config::BatConfig;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::MarkdownSection;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, BatMetadataType, MetadataMarkdownContent,
};
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::parse_formatted_path;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::parser::trait_impl_parser::TraitImplParser;
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use std::fmt::Display;
use std::{fs, vec};

use super::MetadataError;

#[derive(Debug, Clone, PartialEq)]
pub struct TraitMetadata {
    pub path: String,
    pub name: String,
    pub trait_type: TraitMetadataType,
    pub metadata_id: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser for TraitMetadata {
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
        self.trait_type.to_string()
    }
}

impl TraitMetadata {
    fn new(
        path: String,
        name: String,
        trait_type: TraitMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        TraitMetadata {
            path,
            name,
            trait_type,
            metadata_id: Self::create_metadata_id(),
            start_line_index,
            end_line_index,
        }
    }

    pub fn to_trait_impl_parser(
        &self,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
    ) -> Result<TraitImplParser, MetadataError> {
        Ok(
            TraitImplParser::new_from_metadata(self.clone(), optional_function_metadata_vec)
                .change_context(MetadataError)?,
        )
    }

    pub fn from_markdown_section(md_section: MarkdownSection) -> Result<Self, MetadataError> {
        let message = format!(
            "Error parsing function_metadata from markdown_section: \n{:#?}",
            md_section
        );
        let name = md_section.section_header.title;
        let trait_type_string =
            Self::parse_metadata_info_section(&md_section.content, MetadataMarkdownContent::Type)
                .attach_printable(message.clone())?;
        let path =
            Self::parse_metadata_info_section(&md_section.content, MetadataMarkdownContent::Path)
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
        Ok(TraitMetadata::new(
            path,
            name,
            TraitMetadataType::from_str(&trait_type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        ))
    }

    pub fn prompt_multiselection(
        select_all: bool,
        force_select: bool,
    ) -> Result<Vec<Self>, MetadataError> {
        let (function_metadata_vec, function_metadata_names) = Self::prompt_types()?;
        let prompt_text = format!("Select the {}:", "TraitImpl".blue());
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
        let function_metadata_vec =
            Self::get_filtered_metadata(None).change_context(MetadataError)?;
        let function_metadata_names = function_metadata_vec
            .iter()
            .map(|function_metadata| {
                parse_formatted_path(
                    function_metadata.name.clone(),
                    function_metadata.path.clone(),
                    function_metadata.start_line_index.clone(),
                )
            })
            .collect::<Vec<_>>();
        Ok((function_metadata_vec, function_metadata_names))
    }

    pub fn get_filtered_metadata(
        trait_name: Option<&str>,
    ) -> Result<Vec<TraitMetadata>, MetadataError> {
        let traits_impl_sections =
            BatMetadataType::Trait.get_markdown_sections_from_metadata_file()?;

        let filtered_sections = traits_impl_sections
            .into_iter()
            .filter(|section| {
                if trait_name.is_some()
                    && trait_name.clone().unwrap() != section.section_header.title
                {
                    return false;
                };
                return true;
            })
            .collect::<Vec<_>>();
        log::debug!("trait_name\n{:#?}", trait_name);
        log::debug!("filtered_sections\n{:#?}", filtered_sections);
        if filtered_sections.is_empty() {
            let message = format!(
                "Error finding trait sections for:\ntrait_name: {:#?}\n",
                trait_name,
            );
            return Err(Report::new(MetadataError).attach_printable(message));
        }

        let trait_metadata_vec = filtered_sections
            .into_iter()
            .map(|section| TraitMetadata::from_markdown_section(section))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trait_metadata_vec)
    }

    pub fn get_metadata_from_program_files() -> Result<Vec<TraitMetadata>, MetadataError> {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
            .change_context(MetadataError)?;
        let mut traits_metadata: Vec<TraitMetadata> =
            program_dir_entries
                .into_iter()
                .fold(vec![], |mut result_vec, entry| {
                    let entry_path = entry.path().to_str().unwrap().to_string();
                    println!("starting the review of the {} file", entry_path.blue());
                    let file_content = fs::read_to_string(entry.path()).unwrap();
                    let bat_sonar =
                        BatSonar::new_scanned(&file_content, SonarResultType::TraitImpl);
                    for result in bat_sonar.results {
                        println!(
                            "Trait implementation found at {}\n{}",
                            format!("{}:{}", &entry_path, result.start_line_index + 1).magenta(),
                            result.content.clone().green()
                        );
                        let function_metadata = TraitMetadata::new(
                            entry_path.clone(),
                            result.name.to_string(),
                            TraitMetadataType::Implementation,
                            result.start_line_index + 1,
                            result.end_line_index + 1,
                        );
                        result_vec.push(function_metadata);
                    }
                    let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Trait);
                    for result in bat_sonar.results {
                        println!(
                            "Trait Definition found at {}\n{}",
                            format!("{}:{}", &entry_path, result.start_line_index + 1).magenta(),
                            result.content.clone().green()
                        );
                        let function_metadata = TraitMetadata::new(
                            entry_path.clone(),
                            result.name.to_string(),
                            TraitMetadataType::Definition,
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
        traits_metadata.sort_by(|function_a, function_b| function_a.name.cmp(&function_b.name));
        Ok(traits_metadata)
    }

    fn get_metadata_vec_from_markdown() -> Result<Vec<TraitMetadata>, MetadataError> {
        let functions_markdown_file =
            BatMetadataType::Trait.get_markdown_sections_from_metadata_file()?;
        let functions_metadata = functions_markdown_file
            .into_iter()
            .map(|markdown_section| TraitMetadata::from_markdown_section(markdown_section.clone()))
            .collect::<Result<Vec<TraitMetadata>, _>>()?;
        Ok(functions_metadata)
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        trait_section: MetadataMarkdownContent,
    ) -> Result<String, MetadataError> {
        let section_prefix = trait_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .ok_or(MetadataError)
            .into_report()
            .attach_printable(format!(
                "Error parsing info section {:#?}",
                trait_section.to_snake_case()
            ))?
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        Ok(data)
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum TraitMetadataType {
    Definition,
    Implementation,
}

impl TraitMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn get_functions_type_vec() -> Vec<TraitMetadataType> {
        TraitMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn from_str(type_str: &str) -> TraitMetadataType {
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
                Self::Implementation => function_type.to_sentence_case().bright_green(),
                Self::Definition => function_type.to_sentence_case().bright_blue(),
            })
            .collect::<Vec<_>>();
        functions_type_colorized
    }
}
