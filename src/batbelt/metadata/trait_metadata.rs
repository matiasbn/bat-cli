use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct TraitMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    pub trait_source_code_metadata_id: MetadataId,
    pub impl_functions_id: Vec<MetadataId>,
}

impl TraitMetadata {
    pub fn new(
        metadata_id: MetadataId,
        name: String,
        trait_source_code_metadata_id: MetadataId,
        impl_functions_id: Vec<MetadataId>,
    ) -> Self {
        Self {
            metadata_id,
            name,
            trait_source_code_metadata_id,
            impl_functions_id,
        }
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
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
