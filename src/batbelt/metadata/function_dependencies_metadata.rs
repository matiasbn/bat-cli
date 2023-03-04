use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct FunctionDependenciesMetadata {
    pub function_name: String,
    pub metadata_id: MetadataId,
    pub function_metadata_id: String,
    pub dependencies: Vec<MetadataId>,
    pub external_dependencies: Vec<MetadataId>,
}

impl FunctionDependenciesMetadata {
    pub fn new(
        function_name: String,
        metadata_id: MetadataId,
        function_metadata_id: String,
        dependencies: Vec<MetadataId>,
        external_dependencies: Vec<MetadataId>,
    ) -> Self {
        Self {
            function_name,
            metadata_id,
            function_metadata_id,
            dependencies,
            external_dependencies,
        }
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position = bat_metadata
            .clone()
            .function_dependencies
            .into_iter()
            .position(|ep| ep.function_metadata_id == self.function_metadata_id);
        match position {
            None => bat_metadata.function_dependencies.push(self.clone()),
            Some(pos) => bat_metadata.function_dependencies[pos] = self.clone(),
        };
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
