use crate::batbelt::path::BatFile;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, BatMetadataType, MetadataId, SourceCodeMetadata,
};

use crate::batbelt::parser::trait_parser::TraitParser;
use error_stack::{Result, ResultExt};

use super::MetadataError;
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::parser::parse_formatted_path;
use crate::batbelt::BatEnumerator;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::{fs, vec};
use walkdir::DirEntry;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitSourceCodeMetadata {
    pub path: String,
    pub name: String,
    pub trait_type: TraitMetadataType,
    pub metadata_id: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl BatMetadataParser<TraitMetadataType> for TraitSourceCodeMetadata {
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
    fn metadata_sub_type(&self) -> TraitMetadataType {
        self.trait_type
    }
    fn get_bat_metadata_type() -> BatMetadataType {
        BatMetadataType::Trait
    }
    fn get_bat_file() -> BatFile {
        BatFile::TraitsMetadataFile
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
        metadata_id: MetadataId,
    ) -> Self {
        Self {
            path,
            name,
            metadata_id,
            trait_type: metadata_sub_type,
            start_line_index,
            end_line_index,
        }
    }

    //noinspection DuplicatedCode
    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError> {
        let entry_path = entry.path().to_str().unwrap().to_string();
        let file_content = fs::read_to_string(entry.path()).unwrap();
        log::debug!("entry_path:{}", &entry_path);
        log::debug!("file_content:\n{}", &file_content);

        let mut metadata_result = vec![];
        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::TraitImpl);
        log::debug!("sonar_TraitImpl_results:\n{:#?}", bat_sonar.results);
        for result in bat_sonar.results {
            let function_metadata = TraitSourceCodeMetadata::new(
                entry_path.clone(),
                result.name.to_string(),
                TraitMetadataType::Implementation,
                result.start_line_index + 1,
                result.end_line_index + 1,
                Self::create_metadata_id(),
            );
            metadata_result.push(function_metadata);
        }

        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::Trait);
        log::debug!("sonar_Trait_results:\n{:#?}", bat_sonar.results);
        for result in bat_sonar.results {
            let function_metadata = TraitSourceCodeMetadata::new(
                entry_path.clone(),
                result.name.to_string(),
                TraitMetadataType::Definition,
                result.start_line_index + 1,
                result.end_line_index + 1,
                Self::create_metadata_id(),
            );
            metadata_result.push(function_metadata);
        }

        Ok(metadata_result)
    }
}

impl TraitSourceCodeMetadata {
    pub fn to_trait_impl_parser(&self) -> Result<TraitParser, MetadataError> {
        TraitParser::new_from_metadata(self.clone()).change_context(MetadataError)
    }

    pub fn get_trait_parser_vec() -> Result<Vec<TraitParser>, MetadataError> {
        BatMetadata::read_metadata()?
            .source_code
            .traits_source_code
            .into_iter()
            .map(|impl_meta| impl_meta.to_trait_impl_parser())
            .collect::<Result<Vec<_>, MetadataError>>()
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
            TraitMetadataType::get_colorized_type_vec(true),
            None,
        )
        .change_context(MetadataError)?;
        let selected_sub_type = TraitMetadataType::get_type_vec()[selection].clone();
        let metadata_vec_filtered =
            SourceCodeMetadata::get_filtered_traits(None, Some(selected_sub_type))
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
pub enum TraitMetadataType {
    Definition,
    Implementation,
}

impl BatEnumerator for TraitMetadataType {}
