use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EntrypointMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    #[serde(default)]
    pub handler_id: Option<MetadataId>,
    pub context_accounts_id: MetadataId,
    pub entrypoint_function_id: MetadataId,
    #[serde(default)]
    pub program_name: String,
}

impl EntrypointMetadata {
    pub fn new(
        name: String,
        context_accounts_id: MetadataId,
        entrypoint_function_id: MetadataId,
        metadata_id: MetadataId,
        program_name: String,
    ) -> Self {
        Self {
            name,
            metadata_id,
            handler_id: None,
            context_accounts_id,
            entrypoint_function_id,
            program_name,
        }
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let self_clone = self.clone();
        BatMetadata::update_metadata(|bat_metadata| {
            let position = bat_metadata.entry_points.iter().position(|ep| {
                ep.name == self_clone.name
                    && (self_clone.program_name.is_empty()
                        || ep.program_name.is_empty()
                        || ep.program_name == self_clone.program_name)
            });
            match position {
                None => bat_metadata.entry_points.push(self_clone.clone()),
                Some(pos) => bat_metadata.entry_points[pos] = self_clone.clone(),
            };
            bat_metadata.entry_points.sort_by_key(|ep| ep.name.clone());
        })
    }
}
