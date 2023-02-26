use crate::batbelt::path::BatFolder;

use colored::Colorize;
use strum::IntoEnumIterator;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType, BatMetadataTypeParser};

use crate::batbelt::parser::parse_formatted_path;

use crate::batbelt::parser::trait_impl_parser::TraitImplParser;
use error_stack::{Report, Result, ResultExt};

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

impl BatMetadataParser<TraitMetadataType> for TraitMetadata {
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
    fn metadata_sub_type(&self) -> TraitMetadataType {
        self.trait_type
    }

    fn match_bat_metadata_type() -> BatMetadataType {
        BatMetadataType::Trait
    }
    fn metadata_name() -> String {
        "Trait".to_string()
    }

    fn new(
        path: String,
        name: String,
        metadata_sub_type: TraitMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        Self {
            path,
            name,
            metadata_id: Self::create_metadata_id(),
            trait_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }
}

impl TraitMetadata {
    pub fn to_trait_impl_parser(
        &self,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
    ) -> Result<TraitImplParser, MetadataError> {
        Ok(
            TraitImplParser::new_from_metadata(self.clone(), optional_function_metadata_vec)
                .change_context(MetadataError)?,
        )
    }

    pub fn get_trait_parser_vec(
        trait_name: Option<&str>,
        trait_type: Option<TraitMetadataType>,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
    ) -> Result<Vec<TraitImplParser>, MetadataError> {
        Self::get_filtered_metadata(trait_name, trait_type)?
            .into_iter()
            .map(|impl_meta| impl_meta.to_trait_impl_parser(optional_function_metadata_vec.clone()))
            .collect::<Result<Vec<_>, MetadataError>>()
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
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum TraitMetadataType {
    Definition,
    Implementation,
}

impl BatMetadataTypeParser for TraitMetadataType {}
