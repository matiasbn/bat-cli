use serde::Deserialize;

use crate::{get_path, Cli, CODE_OVERHAUL_TEMPLATE_PATH, TEMPLATES_FOLDER};
use core::panicking::panic;
use std::path::Path;
use std::process::Command;
use std::str;
use std::string::String;

#[derive(Debug, Deserialize)]
pub struct SamConfig {
    path: SamPathConfig,
}

#[derive(Debug, Deserialize)]
struct SamPathConfig {
    pub audit_folder_path: String,
    pub templates_path: String,
    pub program_path: String,
    pub program_entrypoints_path: Vec<String>,
}

pub fn create_sam_project() {
    // create SAM default config file
    let toml_str = r#"
        [path]
        audit_folder_path = "./audit-notes"
        templates_path = "../audit-notes/templates"
        program_path = ""
        program_entrypoints_path = [""]
    "#;
    let decoded: SamConfig = toml::from_str(toml_str).unwrap();
    println!("{:#?}", decoded);
}
