use std::str;
use std::{fs, path::Path};

use serde::Deserialize;

use crate::{DEFAULT_AUDIT_NOTES_PATH, DEFAULT_CONFIG_FILE_PATH};

pub const TOML_INITIAL_CONFIG_STR: &str = r#"
    [init]
    auditors_names=[""]
    [path]
    audit_folder_path = "./audit-notes"
    program_path = ""
    program_entrypoints_path = [""]
    "#;

#[derive(Debug, Deserialize)]
pub struct BatmanConfig {
    pub init: InitConfig,
    pub path: PathConfig,
}

#[derive(Debug, Deserialize)]
pub struct InitConfig {
    pub auditors_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PathConfig {
    pub audit_folder_path: String,
    pub program_path: String,
    pub program_entrypoints_path: Vec<String>,
}
