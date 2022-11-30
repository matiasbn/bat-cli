use std::fs;
use std::path::Path;
use std::string::String;

use crate::config::TOML_INITIAL_CONFIG_STR;
use crate::utils::get_config_relative_path;

pub fn create_project(toml_path: Option<String>) {
    let sam_config_toml_path = get_config_relative_path(toml_path);
    let sam_toml_path = Path::new(&sam_config_toml_path);

    if sam_toml_path.exists() {
        panic!(
            "Batman.toml file already exist in {:?}, aborting",
            sam_toml_path
        )
    };

    fs::write(sam_config_toml_path.clone(), TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
    println!("Batman.toml created at {:?}", sam_config_toml_path.clone());
}
