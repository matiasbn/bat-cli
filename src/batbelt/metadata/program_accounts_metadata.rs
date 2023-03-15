use crate::batbelt::git::git_commit::GitCommit;
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use lazy_regex::regex;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, MetadataError, MetadataResult, SourceCodeMetadata,
};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProgramAccountMetadata {
    pub program_account_name: String,
    pub init_account: Vec<InitProgramAccountMetadata>,
    pub mut_account: Vec<MutProgramAccountMetadata>,
    pub close_account_entry_points: Vec<String>,
}

impl ProgramAccountMetadata {
    pub fn create_program_accounts_metadata_file() -> MetadataResult<()> {
        let program_metadata_bat_file = BatFile::ProgramAccountsMetadataFile;
        let file_exists = program_metadata_bat_file
            .file_exists()
            .change_context(MetadataError)?;
        if file_exists {
            return Err(Report::new(MetadataError).attach_printable(format!(
                "{} already exists",
                "program_accounts_metadata.json".bright_green()
            )));
        }
        let sc_names = SourceCodeMetadata::get_filtered_structs(
            None,
            Some(StructMetadataType::SolanaAccount),
        )?
        .into_iter()
        .map(|sc_meta| sc_meta.name)
        .collect::<Vec<_>>();
        let bat_metadata = BatMetadata::read_metadata()?;
        let mut program_account_metadata_vec: Vec<ProgramAccountMetadata> = vec![];
        for program_account_name in sc_names.clone() {
            let mut program_account_metadata = Self {
                program_account_name,
                init_account: vec![],
                mut_account: vec![],
                close_account_entry_points: vec![],
            };
            program_account_metadata.parse_init_data()?;
            program_account_metadata.parse_mut_data()?;
            program_account_metadata.parse_close_entry_points()?;
            program_account_metadata_vec.push(program_account_metadata);
        }
        let mut program_accounts_map = Map::new();
        program_accounts_map.insert("program_accounts_names".to_string(), sc_names.into());

        for program_account_metadata in program_account_metadata_vec {
            let json_value = json!({
                "init_account": program_account_metadata.init_account,
                "mut_account": program_account_metadata.mut_account,
                "close_account_entry_points": program_account_metadata.close_account_entry_points,
            });
            program_accounts_map.insert(program_account_metadata.program_account_name, json_value);
        }
        let serde_value: Value = program_accounts_map.into();
        let json_pretty = serde_json::to_string_pretty(&serde_value)
            .into_report()
            .change_context(MetadataError)?;
        BatFile::ProgramAccountsMetadataFile
            .write_content(false, &json_pretty)
            .change_context(MetadataError)?;
        GitCommit::ProgramAccountMetadataCreated
            .create_commit(true)
            .change_context(MetadataError)?;
        Ok(())
    }

    fn parse_mut_data(&mut self) -> MetadataResult<()> {
        let bat_metadata = BatMetadata::read_metadata()?;
        let mut_context_accounts_id = bat_metadata
            .clone()
            .context_accounts
            .into_iter()
            .filter_map(|ca_metadata| {
                if ca_metadata
                    .context_accounts_info
                    .clone()
                    .into_iter()
                    .any(|ca_info| {
                        ca_info.account_struct_name == self.program_account_name
                            && ca_info.is_mut
                            && !ca_info.is_close
                    })
                {
                    Some(ca_metadata.struct_source_code_metadata_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let entry_point_names = bat_metadata
            .entry_points
            .into_iter()
            .filter_map(|ep_metadata| {
                if mut_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
                    Some(ep_metadata.name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let program_account_fields_vec =
            ProgramAccountField::get_init_vec_from_program_account_name(
                self.program_account_name.clone(),
            )?;
        self.mut_account = entry_point_names
            .into_iter()
            .map(|ep_name| MutProgramAccountMetadata {
                entry_point_name: ep_name.clone(),
                mut_values: program_account_fields_vec.clone(),
            })
            .collect::<Vec<_>>();
        Ok(())
    }

    fn parse_init_data(&mut self) -> MetadataResult<()> {
        let bat_metadata = BatMetadata::read_metadata()?;
        let init_context_accounts_id = bat_metadata
            .clone()
            .context_accounts
            .into_iter()
            .filter_map(|ca_metadata| {
                if ca_metadata
                    .context_accounts_info
                    .clone()
                    .into_iter()
                    .any(|ca_info| {
                        ca_info.account_struct_name == self.program_account_name && ca_info.is_init
                    })
                {
                    Some(ca_metadata.struct_source_code_metadata_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let entry_point_names = bat_metadata
            .entry_points
            .into_iter()
            .filter_map(|ep_metadata| {
                if init_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
                    Some(ep_metadata.name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let program_account_fields_vec =
            ProgramAccountField::get_init_vec_from_program_account_name(
                self.program_account_name.clone(),
            )?;
        self.init_account = entry_point_names
            .into_iter()
            .map(|ep_name| InitProgramAccountMetadata {
                entry_point_name: ep_name.clone(),
                init_values: program_account_fields_vec.clone(),
            })
            .collect::<Vec<_>>();
        Ok(())
    }

    fn parse_close_entry_points(&mut self) -> MetadataResult<()> {
        let bat_metadata = BatMetadata::read_metadata()?;
        let close_context_accounts_id = bat_metadata
            .clone()
            .context_accounts
            .into_iter()
            .filter_map(|ca_metadata| {
                if ca_metadata
                    .context_accounts_info
                    .clone()
                    .into_iter()
                    .any(|ca_info| {
                        ca_info.account_struct_name == self.program_account_name && ca_info.is_close
                    })
                {
                    Some(ca_metadata.struct_source_code_metadata_id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        self.close_account_entry_points = bat_metadata
            .entry_points
            .into_iter()
            .filter_map(|ep_metadata| {
                if close_context_accounts_id.contains(&ep_metadata.context_accounts_id) {
                    Some(ep_metadata.name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        Ok(())
    }

    // pub fn find_program_account_metadata_by_program_account_name(
    //     program_account_name: String,
    // ) -> MetadataResult<Self> {
    //     let bat_metadata = BatMetadata::read_metadata()?;
    //     match bat_metadata
    //         .program_accounts
    //         .into_iter()
    //         .find(|pa_metadata| pa_metadata.program_account_name == program_account_name)
    //     {
    //         None => Err(Report::new(MetadataError).attach_printable(format!(
    //             "Program account metadata not found for {}",
    //             program_account_name.bright_red()
    //         ))),
    //         Some(pa_metadata) => Ok(pa_metadata),
    //     }
    // }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InitProgramAccountMetadata {
    pub entry_point_name: String,
    pub init_values: Vec<ProgramAccountField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MutProgramAccountMetadata {
    pub entry_point_name: String,
    pub mut_values: Vec<ProgramAccountField>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProgramAccountField {
    pub account_key: String,
    pub account_value: Option<String>,
    pub account_type: String,
}

impl ProgramAccountField {
    pub fn get_init_vec_from_program_account_name(
        program_account_name: String,
    ) -> MetadataResult<Vec<Self>> {
        let struct_metadata = SourceCodeMetadata::find_struct(
            program_account_name,
            StructMetadataType::SolanaAccount,
        )?;
        let sc_content = struct_metadata
            .to_source_code_parser(None)
            .get_source_code_content();
        let field_regex = regex!(r#"pub \w+: [\w<>\[\];\s]+"#);
        let field_vec = field_regex
            .find_iter(&sc_content)
            .map(|field_match| {
                let mut field_split = field_match.as_str().trim_start_matches("pub ").split(": ");
                let key = field_split.next().unwrap().to_string();
                let value_type = field_split.next().unwrap().to_string();
                ProgramAccountField {
                    account_key: key,
                    account_type: value_type,
                    account_value: None,
                }
            })
            .collect::<Vec<Self>>();
        Ok(field_vec)
    }
}
