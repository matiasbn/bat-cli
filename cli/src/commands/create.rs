use std::fs;
use std::path::Path;

use std::string::String;

use crate::utils::get_sam_config_relative_path;

pub fn create_sam_project(toml_path: Option<String>) {
    let sam_config_toml_path = get_sam_config_relative_path(toml_path);
    let sam_toml_path = Path::new(&sam_config_toml_path);

    if sam_toml_path.exists() {
        panic!(
            "BAT.toml file already exist in {:?}, aborting",
            sam_toml_path
        )
    };
    // create BAT default config file
    let toml_str = r#"
    [init]
    auditors_names=[""]
    [path]
    audit_folder_path = "./audit-notes"
    templates_path = "../audit-notes/templates"
    program_path = ""
    program_entrypoints_path = [""]
    "#;

    fs::write(sam_config_toml_path.clone(), toml_str).expect("Could not write to file!");
    println!("BAT.toml created at {:?}", sam_config_toml_path.clone());
}
