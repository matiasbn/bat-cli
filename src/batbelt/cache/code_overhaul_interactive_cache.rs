use crate::batbelt::cache::{BatCache, CacheError, CacheResult};
use crate::batbelt::git::git_commit::GitCommit;
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use lazy_regex::regex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

use crate::batbelt::metadata::context_accounts_metadata::ContextAccountsMetadata;
use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, MetadataError, MetadataResult, SourceCodeMetadata,
};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CodeOverhaulInteractiveCache {
    #[serde(default)]
    pub context_accounts_name: String,
    pub entry_point_name: String,
    pub priority: usize,
    pub started: bool,
    #[serde(default)]
    pub parsed: bool,
    pub program_accounts: Vec<String>,
    pub init_program_accounts: Vec<String>,
    pub mut_program_accounts: Vec<String>,
    pub close_program_accounts: Vec<String>,
}

impl CodeOverhaulInteractiveCache {
    pub fn get_suggested_next_entry_point() -> CacheResult<Self> {
        let bat_cache = BatCache::read_cache()?;
        if bat_cache.co_interactive.is_empty() {
            Self::create_cache_data()?;
        }
        Self::update_cache_data()?;
        Ok(bat_cache.co_interactive[0].clone())
    }

    fn create_cache_data() -> CacheResult<Vec<Self>> {
        let bat_metadata = BatMetadata::read_metadata().change_context(CacheError)?;
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
        let mut bat_cache = BatCache::read_cache()?;
        let entry_points_metadata = bat_metadata.entry_points;
        for (ca_meta_id, ca_meta) in init_program_ca_metadata.into_iter().enumerate() {
            let ep_meta = entry_points_metadata
                .clone()
                .into_iter()
                .find(|metadata| {
                    metadata.context_accounts_id == ca_meta.struct_source_code_metadata_id
                })
                .ok_or(CacheError)
                .into_report()
                .attach_printable(format!(
                    "Entry point metadata not found for struct_metadata_id: {}, struct_name: {}",
                    ca_meta.struct_source_code_metadata_id, ca_meta.name
                ));
            if ep_meta.is_err() {
                continue;
            }
            let ep_meta = ep_meta.unwrap();
            bat_cache.co_interactive.push(CodeOverhaulInteractiveCache {
                context_accounts_name: ca_meta.name,
                entry_point_name: ep_meta.name,
                priority: ca_meta_id,
                started: false,
                parsed: false,
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
        bat_cache.save_metadata()?;
        bat_cache.commit_cache()?;
        Ok(bat_cache.co_interactive)
    }

    fn update_cache_data() -> CacheResult<()> {
        let mut bat_cache = BatCache::read_cache().change_context(CacheError)?;
        let co_bat_folder = BatFolder::CodeOverhaulToReview;
        let co_all_file_names = co_bat_folder
            .get_all_files_names(false, None, None)
            .change_context(CacheError)?
            .into_iter()
            .map(|file_name| file_name.trim_end_matches(".md").to_string())
            .collect::<Vec<_>>();
        bat_cache
            .co_interactive
            .iter_mut()
            .map(|co_cache| {
                co_cache.started = !co_all_file_names.contains(&co_cache.entry_point_name.clone());
                co_cache
            })
            .collect::<Vec<_>>();
        bat_cache.save_metadata()?;
        bat_cache.commit_cache()?;
        Ok(())
    }
}
//
// impl StateChangeMetadata {
//     pub fn create_program_accounts_metadata_file() -> MetadataResult<()> {
//         let program_metadata_bat_file = BatFile::ProgramAccountsMetadataFile;
//         let file_exists = program_metadata_bat_file
//             .file_exists()
//             .change_context(MetadataError)?;
//         if file_exists {
//             return Err(Report::new(MetadataError).attach_printable(format!(
//                 "{} already exists",
//                 "program_accounts_metadata.json".bright_green()
//             )));
//         }
//         let sc_names = SourceCodeMetadata::get_filtered_structs(
//             None,
//             Some(StructMetadataType::SolanaAccount),
//         )?
//         .into_iter()
//         .map(|sc_meta| sc_meta.name)
//         .collect::<Vec<_>>();
//         let bat_metadata = BatMetadata::read_metadata()?;
//         let mut program_account_metadata_vec: Vec<StateChangeMetadata> = vec![];
//         for program_account_name in sc_names.clone() {
//             let mut program_account_metadata = Self {
//                 program_account_name,
//                 init_account_contexts: vec![],
//                 mut_account_contexts: vec![],
//                 close_account_entry_points: vec![],
//             };
//             program_account_metadata.parse_init_data()?;
//             program_account_metadata.parse_mut_data()?;
//             program_account_metadata.parse_close_entry_points()?;
//             program_account_metadata_vec.push(program_account_metadata);
//         }
//         let mut program_accounts_map = Map::new();
//         program_accounts_map.insert("program_accounts_names".to_string(), sc_names.into());
//
//         for program_account_metadata in program_account_metadata_vec {
//             let json_value = json!({
//                 "init_account": program_account_metadata.init_account,
//                 "mut_account": program_account_metadata.mut_account,
//                 "close_account_entry_points": program_account_metadata.close_account_entry_points,
//             });
//             program_accounts_map.insert(program_account_metadata.program_account_name, json_value);
//         }
//         let serde_value: Value = program_accounts_map.into();
//         let json_pretty = serde_json::to_string_pretty(&serde_value)
//             .into_report()
//             .change_context(MetadataError)?;
//         BatFile::ProgramAccountsMetadataFile
//             .write_content(false, &json_pretty)
//             .change_context(MetadataError)?;
//         GitCommit::ProgramAccountMetadataCreated
//             .create_commit(true)
//             .change_context(MetadataError)?;
//         Ok(())
//     }
//
//     pub fn update_program_accounts_metadata_file() -> MetadataResult<()> {
//         let pa_bat_file = BatFile::ProgramAccountsMetadataFile;
//         let content = pa_bat_file
//             .read_content(false)
//             .change_context(MetadataError)?;
//         let mut content_value: Value = serde_json::from_str(&content)
//             .into_report()
//             .change_context(MetadataError)?;
//         let pa_field = "program_accounts_names";
//
//         let program_accounts_names = content_value[pa_field]
//             .as_array()
//             .ok_or(MetadataError)
//             .into_report()
//             .attach_printable(format!(
//                 "Error reading {} on {}",
//                 pa_field.bright_green(),
//                 "programs_accounts_metadata.json".bright_green()
//             ))?;
//
//         // program account state change map
//         let mut pa_sc_map = Map::new();
//         let mut entry_points_map = HashSet::new();
//
//         for program_account_name_value in program_accounts_names.clone() {
//             let program_account_name = program_account_name_value
//                 .as_str()
//                 .ok_or(MetadataError)
//                 .into_report()?
//                 .to_string();
//             let mut pa_metadata: StateChangeMetadata =
//                 serde_json::from_value(content_value[&program_account_name].clone())
//                     .into_report()
//                     .change_context(MetadataError)?;
//             pa_metadata.program_account_name = program_account_name;
//             // for every account, create a map to insert into the pa json
//             let mut state_change_map = HashMap::new();
//
//             for value_change in pa_metadata.init_account_contexts {
//                 let ep_name = value_change.entry_point_name;
//                 // save entry_points to check deprecated eps
//                 entry_points_map.insert(ep_name.clone());
//                 for init_value in value_change.init_values {
//                     let account_key = init_value.account_key;
//                     let state_change = StateChange {
//                         entry_point: ep_name.clone(),
//                         value: init_value.account_value.unwrap_or("".to_string()),
//                     };
//                     let map_value = state_change_map.get_mut(&account_key);
//                     match map_value {
//                         None => {
//                             state_change_map.insert(account_key.clone(), vec![state_change]);
//                         }
//                         Some(state_change_vec) => {
//                             state_change_vec.push(state_change);
//                         }
//                     }
//                 }
//             }
//
//             // at this point, all values of the account exists in map, so is not necessary to test if exists
//             for value_change in pa_metadata.mut_account_contexts {
//                 let ep_name = value_change.entry_point_name;
//                 // save entry_points to check deprecated eps
//                 entry_points_map.insert(ep_name.clone());
//                 for mut_value in value_change.mut_values {
//                     let account_key = mut_value.account_key.clone();
//                     if mut_value.account_value.is_none() {
//                         continue;
//                     }
//                     let state_change = StateChange {
//                         entry_point: ep_name.clone(),
//                         value: mut_value.account_value.unwrap(),
//                     };
//
//                     // the state changes for the given account key -> pub account_key: account_value;
//                     let mut state_change_vec = state_change_map
//                         .get_mut(&account_key)
//                         .ok_or(MetadataError)
//                         .into_report()?;
//                     state_change_vec.push(state_change);
//                 }
//             }
//
//             // here we have all the changes, insert state_change_map into pa_sc_map <AccountName,Map<AccountKey, Vec<StateChanges>>>
//             pa_sc_map.insert(pa_metadata.program_account_name, json!(state_change_map));
//         }
//
//         // insert into content_value, prettify and save to pa json file
//         let mut ep_vec = entry_points_map.into_iter().collect::<Vec<_>>();
//         ep_vec.sort();
//         content_value["state_changes"] = json!(pa_sc_map);
//         content_value["entry_points"] = json!(ep_vec);
//         let pretty_content = serde_json::to_string_pretty(&content_value)
//             .into_report()
//             .change_context(MetadataError)?;
//         pa_bat_file
//             .write_content(false, &pretty_content)
//             .change_context(MetadataError)?;
//         GitCommit::ProgramAccountMetadataUpdated
//             .create_commit(true)
//             .change_context(MetadataError)?;
//         Ok(())
//     }
//
//     fn parse_mut_data(&mut self) -> MetadataResult<()> {
//         let bat_metadata = BatMetadata::read_metadata()?;
//         let mut_context_accounts_id = bat_metadata
//             .clone()
//             .context_accounts
//             .into_iter()
//             .filter_map(|ca_metadata| {
//                 if ca_metadata
//                     .context_accounts_info
//                     .clone()
//                     .into_iter()
//                     .any(|ca_info| {
//                         ca_info.account_struct_name == self.program_account_name
//                             && ca_info.is_mut
//                             && !ca_info.is_close
//                     })
//                 {
//                     Some(ca_metadata.struct_source_code_metadata_id)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         let entry_point_names = bat_metadata
//             .entry_points
//             .into_iter()
//             .filter_map(|ep_metadata| {
//                 if mut_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
//                     Some(ep_metadata.name)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         let program_account_fields_vec = AccountField::get_init_vec_from_program_account_name(
//             self.program_account_name.clone(),
//         )?;
//         self.mut_account_contexts = entry_point_names
//             .into_iter()
//             .map(|ep_name| MutAccountMetadata {
//                 entry_point_name: ep_name.clone(),
//                 mut_values: program_account_fields_vec.clone(),
//             })
//             .collect::<Vec<_>>();
//         Ok(())
//     }
//
//     fn parse_init_data(&mut self) -> MetadataResult<()> {
//         let bat_metadata = BatMetadata::read_metadata()?;
//         let init_context_accounts_id = bat_metadata
//             .clone()
//             .context_accounts
//             .into_iter()
//             .filter_map(|ca_metadata| {
//                 if ca_metadata
//                     .context_accounts_info
//                     .clone()
//                     .into_iter()
//                     .any(|ca_info| {
//                         ca_info.account_struct_name == self.program_account_name && ca_info.is_init
//                     })
//                 {
//                     Some(ca_metadata.struct_source_code_metadata_id)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         let entry_point_names = bat_metadata
//             .entry_points
//             .into_iter()
//             .filter_map(|ep_metadata| {
//                 if init_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
//                     Some(ep_metadata.name)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         let program_account_fields_vec = AccountField::get_init_vec_from_program_account_name(
//             self.program_account_name.clone(),
//         )?;
//         self.init_account_contexts = entry_point_names
//             .into_iter()
//             .map(|ep_name| InitAccountMetadata {
//                 entry_point_name: ep_name.clone(),
//                 init_values: program_account_fields_vec.clone(),
//             })
//             .collect::<Vec<_>>();
//         Ok(())
//     }
//
//     fn parse_close_entry_points(&mut self) -> MetadataResult<()> {
//         let bat_metadata = BatMetadata::read_metadata()?;
//         let close_context_accounts_id = bat_metadata
//             .clone()
//             .context_accounts
//             .into_iter()
//             .filter_map(|ca_metadata| {
//                 if ca_metadata
//                     .context_accounts_info
//                     .clone()
//                     .into_iter()
//                     .any(|ca_info| {
//                         ca_info.account_struct_name == self.program_account_name && ca_info.is_close
//                     })
//                 {
//                     Some(ca_metadata.struct_source_code_metadata_id)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         self.close_account_entry_points = bat_metadata
//             .entry_points
//             .into_iter()
//             .filter_map(|ep_metadata| {
//                 if close_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
//                     Some(ep_metadata.name)
//                 } else {
//                     None
//                 }
//             })
//             .collect::<Vec<_>>();
//         Ok(())
//     }
//
//     // pub fn find_program_account_metadata_by_program_account_name(
//     //     program_account_name: String,
//     // ) -> MetadataResult<Self> {
//     //     let bat_metadata = BatMetadata::read_metadata()?;
//     //     match bat_metadata
//     //         .program_accounts
//     //         .into_iter()
//     //         .find(|pa_metadata| pa_metadata.program_account_name == program_account_name)
//     //     {
//     //         None => Err(Report::new(MetadataError).attach_printable(format!(
//     //             "Program account metadata not found for {}",
//     //             program_account_name.bright_red()
//     //         ))),
//     //         Some(pa_metadata) => Ok(pa_metadata),
//     //     }
//     // }
// }
//
// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct InitAccountMetadata {
//     pub entry_point_name: String,
//     pub init_values: Vec<AccountField>,
// }
//
// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct MutAccountMetadata {
//     pub entry_point_name: String,
//     pub mut_values: Vec<AccountField>,
// }
//
// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct AccountField {
//     pub account_key: String,
//     pub account_value: Option<String>,
//     pub account_type: String,
// }
//
// impl AccountField {
//     pub fn get_init_vec_from_program_account_name(
//         program_account_name: String,
//     ) -> MetadataResult<Vec<Self>> {
//         let struct_metadata = SourceCodeMetadata::find_struct(
//             program_account_name,
//             StructMetadataType::SolanaAccount,
//         )?;
//         let sc_content = struct_metadata
//             .to_source_code_parser(None)
//             .get_source_code_content();
//         let field_regex = regex!(r#"pub \w+: [\w<>\[\];\s]+"#);
//         let field_vec = field_regex
//             .find_iter(&sc_content)
//             .map(|field_match| {
//                 let mut field_split = field_match.as_str().trim_start_matches("pub ").split(": ");
//                 let key = field_split.next().unwrap().to_string();
//                 let value_type = field_split.next().unwrap().to_string();
//                 AccountField {
//                     account_key: key,
//                     account_type: value_type,
//                     account_value: None,
//                 }
//             })
//             .collect::<Vec<Self>>();
//         Ok(field_vec)
//     }
// }
//
// #[derive(Serialize, Deserialize, Clone, Debug)]
// pub struct StateChange {
//     pub entry_point: String,
//     pub value: String,
// }
