use crate::batbelt::metadata::{BatMetadata, MetadataError, MetadataId, MetadataResult};
use crate::batbelt::parser::context_accounts_parser::CAAccountParser;
use colored::Colorize;
use error_stack::{IntoReport, ResultExt};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContextAccountsMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    pub struct_source_code_metadata_id: MetadataId,
    pub context_accounts_info: Vec<CAAccountParser>,
}

impl ContextAccountsMetadata {
    pub fn new(
        name: String,
        metadata_id: MetadataId,
        struct_source_code_metadata_id: MetadataId,
        context_accounts_info: Vec<CAAccountParser>,
    ) -> Self {
        Self {
            name,
            metadata_id,
            struct_source_code_metadata_id,
            context_accounts_info,
        }
    }

    pub fn find_context_accounts_metadata_by_struct_metadata_id(
        struct_source_code_metadata_id: MetadataId,
    ) -> MetadataResult<ContextAccountsMetadata> {
        let bat_metadata = BatMetadata::read_metadata()?;
        let context_accounts_metadata = bat_metadata
            .context_accounts
            .clone()
            .into_iter()
            .find(|ca_metadata| {
                ca_metadata.struct_source_code_metadata_id == struct_source_code_metadata_id
            })
            .ok_or(MetadataError)
            .into_report()
            .attach_printable(format!(
                "Context accounts metadata not found for struct metadata id: {}",
                struct_source_code_metadata_id.green()
            ))?;
        Ok(context_accounts_metadata)
    }

    pub fn update_metadata_file(&self) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let position = bat_metadata
            .clone()
            .context_accounts
            .into_iter()
            .position(|ca_metadata| {
                ca_metadata.struct_source_code_metadata_id == self.struct_source_code_metadata_id
            });
        match position {
            None => bat_metadata.context_accounts.push(self.clone()),
            Some(pos) => bat_metadata.context_accounts[pos] = self.clone(),
        };
        bat_metadata
            .context_accounts
            .sort_by_key(|ca_meta| ca_meta.name.clone());
        bat_metadata.save_metadata()?;
        Ok(())
    }
}
