use crate::batbelt::analytics::{AnalyticsError, AnalyticsResult, BatAnalytics};
use error_stack::{IntoReport, ResultExt};
use serde::{Deserialize, Serialize};

use crate::batbelt::metadata::context_accounts_metadata::ContextAccountsMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct EntryPointFlowAnalytics {
    pub context_accounts_name: String,
    pub entry_point_name: String,
    pub priority: usize,
    pub program_accounts: Vec<String>,
    pub init_program_accounts: Vec<String>,
    pub mut_program_accounts: Vec<String>,
    pub close_program_accounts: Vec<String>,
}

impl EntryPointFlowAnalytics {
    pub fn init_analytics_data() -> AnalyticsResult<()> {
        let bat_metadata = BatMetadata::read_metadata().change_context(AnalyticsError)?;
        let context_accounts_metadata = bat_metadata.context_accounts.clone();
        let (mut init_program_ca_metadata, mut not_init_program_ca_metadata): (
            Vec<ContextAccountsMetadata>,
            Vec<ContextAccountsMetadata>,
        ) = context_accounts_metadata
            .into_iter()
            .filter(|ca_metadata| !ca_metadata.program_accounts.is_empty())
            .partition(|ca_metadata| !ca_metadata.init_program_account.is_empty());
        init_program_ca_metadata.sort_by(|ca_meta_a, ca_meta_b| {
            ca_meta_a
                .program_accounts
                .len()
                .cmp(&ca_meta_b.program_accounts.len())
        });
        not_init_program_ca_metadata.sort_by(|ca_meta_a, ca_meta_b| {
            ca_meta_a
                .program_accounts
                .len()
                .cmp(&ca_meta_b.program_accounts.len())
        });
        init_program_ca_metadata.append(&mut not_init_program_ca_metadata);

        let mut bat_analytics = BatAnalytics::read_analytics()?;
        let entry_points_metadata = bat_metadata.entry_points;
        for (ca_meta_id, ca_meta) in init_program_ca_metadata.into_iter().enumerate() {
            let ep_meta = entry_points_metadata
                .clone()
                .into_iter()
                .find(|metadata| {
                    metadata.context_accounts_id == ca_meta.struct_source_code_metadata_id
                })
                .ok_or(AnalyticsError)
                .into_report()
                .attach_printable(format!(
                    "Entry point metadata not found for struct_metadata_id: {}, struct_name: {}",
                    ca_meta.struct_source_code_metadata_id, ca_meta.name
                ));
            if ep_meta.is_err() {
                continue;
            }
            let ep_meta = ep_meta.unwrap();
            bat_analytics
                .entry_points_flow
                .push(EntryPointFlowAnalytics {
                    context_accounts_name: ca_meta.name,
                    entry_point_name: ep_meta.name,
                    priority: ca_meta_id,
                    program_accounts: ca_meta.program_accounts,
                    init_program_accounts: ca_meta
                        .init_program_account
                        .into_iter()
                        .map(|ca_parser| ca_parser.account_struct_name)
                        .collect::<Vec<_>>(),
                    mut_program_accounts: ca_meta
                        .mut_program_account
                        .into_iter()
                        .map(|ca_parser| ca_parser.account_struct_name)
                        .collect::<Vec<_>>(),
                    close_program_accounts: ca_meta
                        .close_program_account
                        .into_iter()
                        .map(|ca_parser| ca_parser.account_struct_name)
                        .collect::<Vec<_>>(),
                })
        }
        bat_analytics.save_analytics()?;
        Ok(())
    }
}
