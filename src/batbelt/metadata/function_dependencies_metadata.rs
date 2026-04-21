use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FunctionDependencyInfo {
    pub function_name: String,
    pub function_metadata_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct FunctionDependenciesMetadata {
    pub function_name: String,
    pub metadata_id: MetadataId,
    pub function_metadata_id: String,
    pub dependencies: Vec<FunctionDependencyInfo>,
    pub external_dependencies: Vec<MetadataId>,
    #[serde(default)]
    pub program_name: String,
}

impl FunctionDependenciesMetadata {
    pub fn new(
        function_name: String,
        metadata_id: MetadataId,
        function_metadata_id: String,
        dependencies: Vec<FunctionDependencyInfo>,
        external_dependencies: Vec<MetadataId>,
        program_name: String,
    ) -> Self {
        Self {
            function_name,
            metadata_id,
            function_metadata_id,
            dependencies,
            external_dependencies,
            program_name,
        }
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let self_clone = self.clone();
        BatMetadata::update_metadata(|bat_metadata| {
            let position = bat_metadata
                .function_dependencies
                .iter()
                .position(|ep| ep.function_metadata_id == self_clone.function_metadata_id);
            match position {
                None => bat_metadata.function_dependencies.push(self_clone.clone()),
                Some(pos) => bat_metadata.function_dependencies[pos] = self_clone.clone(),
            };
            bat_metadata
                .function_dependencies
                .sort_by_key(|func_dep| func_dep.function_name.clone());
        })
    }
}
