use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntrypointMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    pub handler_id: Option<MetadataId>,
    pub context_accounts_id: MetadataId,
    pub entrypoint_function_id: MetadataId,
}

impl EntrypointMetadata {
    pub fn new(
        name: String,
        handler_id: Option<MetadataId>,
        context_accounts_id: MetadataId,
        entrypoint_function_id: MetadataId,
        metadata_id: MetadataId,
    ) -> Self {
        Self {
            name,
            metadata_id,
            handler_id,
            context_accounts_id,
            entrypoint_function_id,
        }
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position = bat_metadata
            .clone()
            .entry_points
            .into_iter()
            .position(|ep| ep.name == self.name);
        match position {
            None => bat_metadata.entry_points.push(self.clone()),
            Some(pos) => bat_metadata.entry_points[pos] = self.clone(),
        };
        bat_metadata.entry_points.sort_by_key(|ep| ep.name.clone());
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
