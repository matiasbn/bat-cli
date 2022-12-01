use std::str;

use serde::Deserialize;

pub const TOML_INITIAL_CONFIG_STR: &str = r#"
    [required]
    auditor_names=[""]
    audit_folder_path = "./audit-notes"
    program_lib_path = ""
    "#;

#[derive(Debug, Deserialize, Clone)]
pub struct BatConfig {
    pub required: RequiredConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RequiredConfig {
    pub auditor_names: Vec<String>,
    pub audit_folder_path: String,
    pub program_lib_path: String,
}
