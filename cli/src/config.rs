use std::{fs, path::Path, str};

use serde::Deserialize;

use crate::commands::create::{AUDITOR_TOML_INITIAL_PATH, BAT_TOML_INITIAL_PATH};

#[derive(Debug, Deserialize, Clone)]
pub struct BatConfig {
    pub required: RequiredConfig,
    pub auditor: AuditorConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RequiredConfig {
    pub auditor_names: Vec<String>,
    pub audit_folder_path: String,
    pub program_lib_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuditorConfig {
    pub auditor: String,
}

impl BatConfig {
    pub fn get_config() -> BatConfig {
        // Bat.toml
        let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);
        if !bat_toml_path.is_file() {
            panic!("Bat.toml file not found at {:?}", bat_toml_path);
        }
        let bat_toml_file = fs::read(bat_toml_path).unwrap();
        let bat_tom_file_string = str::from_utf8(bat_toml_file.as_slice()).unwrap();
        let required: RequiredConfig = toml::from_str(bat_tom_file_string).unwrap();

        // BatAuditor.toml
        let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);
        if !auditor_toml_path.is_file() {
            panic!("Bat.toml file not found at {:?}", auditor_toml_path);
        }
        let auditor_toml_file = fs::read(auditor_toml_path).unwrap();
        let auditor_tom_file_string = str::from_utf8(auditor_toml_file.as_slice()).unwrap();
        let auditor: AuditorConfig = toml::from_str(auditor_tom_file_string).unwrap();

        let config = BatConfig { required, auditor };
        config
    }

    pub fn get_auditor_names() -> Vec<String> {
        return Self::get_config().required.auditor_names;
    }

    pub fn get_audit_folder_path() -> String {
        return Self::get_config().required.audit_folder_path;
    }

    pub fn get_program_lib_path() -> String {
        return Self::get_config().required.program_lib_path;
    }

    pub fn get_test_config() -> BatConfig {
        let required = RequiredConfig {
            auditor_names: vec!["matias".to_string(), "porter".to_string()],
            audit_folder_path: "../audit-notes".to_string(),
            program_lib_path:
                "../star-atlas-programs/sol-programs/scream/programs/player_profile/src/lib.rs"
                    .to_string(),
        };
        let auditor = AuditorConfig {
            auditor: "matias".to_string(),
        };
        let bat_config = BatConfig { required, auditor };
        bat_config
    }
}

#[test]

fn test_get_test_config() {
    let batconfig = BatConfig::get_test_config();
    println!("{:#?}", batconfig);
}
