use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFolder;
use crate::config::BatConfig;
use colored::{ColoredString, Colorize};
use strum::IntoEnumIterator;

use crate::batbelt::markdown::MarkdownSection;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::BatMetadataType;
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use std::fmt::Display;
use std::{fs, vec};

use super::MetadataError;

#[derive(Debug, Clone, PartialEq)]
pub struct TraitImplMetadata {
    pub path: String,
    pub name: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl TraitImplMetadata {
    fn new(path: String, name: String, start_line_index: usize, end_line_index: usize) -> Self {
        TraitImplMetadata {
            path,
            name,
            start_line_index,
            end_line_index,
        }
    }

    pub fn get_markdown_section_content_string(&self) -> String {
        format!(
            "# {}\n\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.name, self.path, self.start_line_index, self.end_line_index
        )
    }

    pub fn to_source_code_parser(&self, optional_name: Option<String>) -> SourceCodeParser {
        SourceCodeParser::new(
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

    pub fn from_markdown_section(md_section: MarkdownSection) -> Result<Self, MetadataError> {
        let message = format!(
            "Error parsing function_metadata from markdown_section: \n{:#?}",
            md_section
        );
        let name = md_section.section_header.title;
        let path = Self::parse_metadata_info_section(
            &md_section.content,
            TraitImplMetadataInfoSection::Path,
        )
        .attach_printable(message.clone())?;
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            TraitImplMetadataInfoSection::StartLineIndex,
        )
        .attach_printable(message.clone())?;
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            TraitImplMetadataInfoSection::EndLineIndex,
        )
        .attach_printable(message.clone())?;
        Ok(TraitImplMetadata::new(
            path,
            name,
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        ))
    }
    //
    // pub fn prompt_multiselection(
    //     select_all: bool,
    //     force_select: bool,
    // ) -> Result<Vec<Self>, MetadataError> {
    //     let (function_metadata_vec, function_metadata_names) = Self::prompt_types()?;
    //     let prompt_text = format!("Please select the {}:", "Function".blue());
    //     let selections = BatDialoguer::multiselect(
    //         prompt_text.clone(),
    //         function_metadata_names.clone(),
    //         Some(&vec![select_all; function_metadata_names.len()]),
    //         force_select,
    //     )
    //     .change_context(MetadataError)?;
    //
    //     let filtered_vec = function_metadata_vec
    //         .into_iter()
    //         .enumerate()
    //         .filter_map(|(sc_index, sc_metadata)| {
    //             if selections.iter().any(|selection| &sc_index == selection) {
    //                 Some(sc_metadata)
    //             } else {
    //                 None
    //             }
    //         })
    //         .collect::<Vec<_>>();
    //     return Ok(filtered_vec);
    // }
    //
    // fn prompt_types() -> Result<(Vec<Self>, Vec<String>), MetadataError> {
    //     let prompt_text = format!("Please select the {}:", "Function type".blue());
    //     let function_types_colorized = TraitImplMetadataType::get_colorized_functions_type_vec();
    //     let selection = BatDialoguer::select(prompt_text, function_types_colorized.clone(), None)
    //         .change_context(MetadataError)?;
    //     let selected_function_type = TraitImplMetadataType::get_functions_type_vec()[selection];
    //     let function_metadata_vec = Self::get_filtered_metadata(None, Some(selected_function_type))
    //         .change_context(MetadataError)?;
    //     let function_metadata_names = function_metadata_vec
    //         .iter()
    //         .map(|function_metadata| {
    //             format!(
    //                 "{}: {}:{}",
    //                 function_metadata.name.clone(),
    //                 function_metadata.path.clone(),
    //                 function_metadata.start_line_index.clone()
    //             )
    //         })
    //         .collect::<Vec<_>>();
    //     Ok((function_metadata_vec, function_metadata_names))
    // }

    pub fn get_filtered_metadata(
        trait_name: Option<&str>,
    ) -> Result<Vec<TraitImplMetadata>, MetadataError> {
        let function_sections =
            BatMetadataType::Functions.get_markdown_sections_from_metadata_file()?;

        let filtered_sections = function_sections
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
            .map(|section| TraitImplMetadata::from_markdown_section(section))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(trait_metadata_vec)
    }

    pub fn get_traits_impl_metadata_from_program() -> Result<Vec<TraitImplMetadata>, MetadataError>
    {
        let program_dir_entries = BatFolder::ProgramPath
            .get_all_files_dir_entries(false, None, None)
            .change_context(MetadataError)?;
        let mut traits_metadata: Vec<TraitImplMetadata> =
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
                            "TraitImpl found at {}\n{}",
                            format!("{}:{}", &entry_path, result.start_line_index + 1).magenta(),
                            result.content.clone().green()
                        );
                        let function_metadata = TraitImplMetadata::new(
                            entry_path.clone(),
                            result.name.to_string(),
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

    fn get_metadata_vec_from_markdown() -> Result<Vec<TraitImplMetadata>, MetadataError> {
        let functions_markdown_file =
            BatMetadataType::TraitImpl.get_markdown_sections_from_metadata_file()?;
        let functions_metadata = functions_markdown_file
            .into_iter()
            .map(|markdown_section| {
                TraitImplMetadata::from_markdown_section(markdown_section.clone())
            })
            .collect::<Result<Vec<TraitImplMetadata>, _>>()?;
        Ok(functions_metadata)
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        function_section: TraitImplMetadataInfoSection,
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
pub enum TraitImplMetadataInfoSection {
    Path,
    Name,
    StartLineIndex,
    EndLineIndex,
}

impl TraitImplMetadataInfoSection {
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
