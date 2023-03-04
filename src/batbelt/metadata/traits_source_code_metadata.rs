use crate::batbelt::path::BatFile;

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType, MetadataId};

use crate::batbelt::parser::trait_parser::TraitParser;
use error_stack::{Result, ResultExt};

use super::MetadataError;
use crate::batbelt::BatEnumerator;
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

        let bat_sonar = BatSonar::new_scanned(&file_content, SonarResultType::TraitImpl);
        let mut metadata_result = vec![];
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
    pub fn to_trait_impl_parser(
        &self,
        optional_function_metadata_vec: Option<Vec<FunctionSourceCodeMetadata>>,
    ) -> Result<TraitParser, MetadataError> {
        TraitParser::new_from_metadata(self.clone()).change_context(MetadataError)
    }

    pub fn get_trait_parser_vec(
        trait_name: Option<&str>,
        trait_type: Option<TraitMetadataType>,
        optional_function_metadata_vec: Option<Vec<FunctionSourceCodeMetadata>>,
    ) -> Result<Vec<TraitParser>, MetadataError> {
        Self::get_filtered_metadata(trait_name, trait_type)?
            .into_iter()
            .map(|impl_meta| impl_meta.to_trait_impl_parser(optional_function_metadata_vec.clone()))
            .collect::<Result<Vec<_>, MetadataError>>()
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
