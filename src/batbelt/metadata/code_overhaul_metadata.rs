use crate::batbelt::metadata::{BatMetadata, MetadataId, MetadataResult};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulSignerMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulMetadata {
    pub metadata_id: MetadataId,
    pub entry_point_name: String,
    pub validations: Vec<String>,
    pub context_accounts_content: String,
    pub signers: Vec<CodeOverhaulSignerMetadata>,
}

impl CodeOverhaulMetadata {
    pub fn new(
        metadata_id: MetadataId,
        entry_point_name: String,
        validations: Vec<String>,
        context_accounts_content: String,
        signers: Vec<CodeOverhaulSignerMetadata>,
    ) -> Self {
        Self {
            metadata_id,
            entry_point_name,
            validations,
            context_accounts_content,
            signers,
        }
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position = bat_metadata
            .clone()
            .code_overhaul
            .into_iter()
            .position(|co_meta| co_meta.entry_point_name == self.entry_point_name);
        match position {
            None => bat_metadata.code_overhaul.push(self.clone()),
            Some(pos) => bat_metadata.code_overhaul[pos] = self.clone(),
        };
        bat_metadata
            .code_overhaul
            .sort_by_key(|co_meta| co_meta.entry_point_name.clone());
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
