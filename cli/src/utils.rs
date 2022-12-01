use std::str;
use std::{fs, path::Path};

use crate::config::BatmanConfig;
use crate::{DEFAULT_AUDIT_NOTES_PATH, DEFAULT_CONFIG_FILE_PATH};

pub fn get_notes_path(path: Option<String>) -> String {
    match path {
        Some(audit_path) => audit_path,
        None => String::from(DEFAULT_AUDIT_NOTES_PATH),
    }
}

pub fn get_config() -> BatmanConfig {
    let batman_toml_path = Path::new(&"./Bat.toml");
    if !batman_toml_path.is_file() {
        panic!("Bat.toml file not found at {:?}", batman_toml_path);
    }
    let toml_file = fs::read(batman_toml_path).unwrap();
    let tom_file_string = str::from_utf8(toml_file.as_slice()).unwrap();
    let decoded: BatmanConfig = toml::from_str(tom_file_string).unwrap();
    decoded
}

pub fn get_config_relative_path(relative_path: Option<String>) -> String {
    String::from(DEFAULT_CONFIG_FILE_PATH)
    // match relative_path {
    //     Some(sam_config_path) => sam_config_path + &String::from("/Bat.toml"),
    //     None => String::from(DEFAULT_SAM_CONFIG_PATH),
    // }
}
