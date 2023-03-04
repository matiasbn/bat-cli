use crate::batbelt::metadata::traits_source_code_metadata::TraitMetadataType;
use crate::batbelt::metadata::{BatMetadata, MetadataError, MetadataId, MetadataResult};
use crate::batbelt::parser::trait_parser::TraitParser;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraitMetadataFunction {
    pub function_source_code_metadata_id: MetadataId,
    pub trait_signature: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TraitMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    pub trait_type: TraitMetadataType,
    pub impl_from: String,
    pub impl_to: String,
    pub external_trait: bool,
    pub trait_source_code_metadata_id: MetadataId,
    pub impl_functions: Vec<TraitMetadataFunction>,
}

impl TraitMetadata {
    pub fn new(
        metadata_id: MetadataId,
        name: String,
        trait_source_code_metadata_id: MetadataId,
        impl_functions: Vec<TraitMetadataFunction>,
        trait_type: TraitMetadataType,
        external_trait: bool,
        impl_from: String,
        impl_to: String,
    ) -> Self {
        Self {
            metadata_id,
            name,
            trait_source_code_metadata_id,
            impl_functions,
            trait_type,
            impl_from,
            impl_to,
            external_trait,
        }
    }

    pub fn to_trait_parser(&self) -> MetadataResult<TraitParser> {
        let trait_sc_metadata = BatMetadata::read_metadata()?
            .source_code
            .get_trait_by_id(self.trait_source_code_metadata_id.clone())?;
        TraitParser::new_from_metadata(trait_sc_metadata).change_context(MetadataError)
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position =
            bat_metadata.clone().traits.into_iter().position(|ep| {
                ep.trait_source_code_metadata_id == self.trait_source_code_metadata_id
            });
        match position {
            None => bat_metadata.traits.push(self.clone()),
            Some(pos) => bat_metadata.traits[pos] = self.clone(),
        };
        bat_metadata
            .traits
            .sort_by_key(|trait_meta| trait_meta.name.clone());
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
