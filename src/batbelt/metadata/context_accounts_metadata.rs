use crate::batbelt::metadata::{BatMetadata, MetadataError, MetadataId, MetadataResult};
use crate::batbelt::parser::context_accounts_parser::CAAccountParser;
use colored::Colorize;
use error_stack::{IntoReport, ResultExt};

use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContextAccountsMetadata {
    pub name: String,
    pub metadata_id: MetadataId,
    pub struct_source_code_metadata_id: MetadataId,
    #[serde(default)]
    pub init_program_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub mut_program_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub close_program_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub init_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub mut_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub close_account: Vec<CAAccountParser>,
    #[serde(default)]
    pub program_accounts: Vec<String>,
    pub context_accounts_info: Vec<CAAccountParser>,
}

impl ContextAccountsMetadata {
    pub fn new(
        name: String,
        metadata_id: MetadataId,
        struct_source_code_metadata_id: MetadataId,
        context_accounts_info: Vec<CAAccountParser>,
    ) -> Self {
        let mut init_program_account = vec![];
        let mut mut_program_account = vec![];
        let mut close_program_account = vec![];
        let mut init_account = vec![];
        let mut mut_account = vec![];
        let mut close_account = vec![];
        let mut program_accounts = vec![];
        for account_info in context_accounts_info.clone() {
            if account_info.solana_account_type == SolanaAccountType::ProgramStateAccount {
                program_accounts.push(account_info.account_struct_name.clone());
                if account_info.is_init {
                    init_program_account.push(account_info);
                    continue;
                }
                if account_info.is_close {
                    close_program_account.push(account_info);
                    continue;
                }
                if account_info.is_mut {
                    mut_program_account.push(account_info);
                    continue;
                }
            } else {
                if account_info.is_init {
                    init_account.push(account_info);
                    continue;
                }
                if account_info.is_close {
                    close_account.push(account_info);
                    continue;
                }
                if account_info.is_mut {
                    mut_account.push(account_info);
                    continue;
                }
            }
        }
        Self {
            name,
            metadata_id,
            struct_source_code_metadata_id,
            init_program_account,
            mut_program_account,
            close_program_account,
            init_account,
            mut_account,
            close_account,
            program_accounts,
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
        let self_clone = self.clone();
        BatMetadata::update_metadata(|bat_metadata| {
            let position = bat_metadata
                .context_accounts
                .iter()
                .position(|ca_metadata| {
                    ca_metadata.struct_source_code_metadata_id == self_clone.struct_source_code_metadata_id
                });
            match position {
                None => bat_metadata.context_accounts.push(self_clone.clone()),
                Some(pos) => bat_metadata.context_accounts[pos] = self_clone.clone(),
            };
            bat_metadata
                .context_accounts
                .sort_by_key(|ca_meta| ca_meta.name.clone());
        })
    }
}
