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
    pub auditor_name: String,
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

        // BatAuditor.toml
        let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);
        if !auditor_toml_path.is_file() {
            panic!("BatAuditor.toml file not found at {:?}", auditor_toml_path);
        }
        let auditor_toml_file = fs::read(auditor_toml_path).unwrap();
        let auditor_tom_file_string = str::from_utf8(auditor_toml_file.as_slice()).unwrap();

        // Get the BatConfig complete
        let config: BatConfig =
            toml::from_str((bat_tom_file_string.to_string() + auditor_tom_file_string).as_str())
                .unwrap();
        config
    }

    pub fn validate_initial_config() {
        Self::validate_audit_folder();
        Self::validate_auditor_notes_folder();
        Self::validate_program_path();
    }

    // audit notes folder should not exist
    pub fn validate_audit_folder() {
        let bat_config = BatConfig::get_config().required;
        if Path::new(&bat_config.audit_folder_path).is_dir() {
            panic!(
                "audit folder {:?} already exists, abortings",
                &bat_config.audit_folder_path
            );
        }
    }

    // auditors notes folders should not exist and not empty
    pub fn validate_auditor_notes_folder() {
        let bat_config = BatConfig::get_config().required;
        if bat_config.auditor_names.is_empty() {
            panic!("required parameter auditors_names is empty in Bat.toml file, aborting",);
        }
        for auditor_name in &bat_config.auditor_names {
            let auditor_folder_path =
                bat_config.audit_folder_path.clone() + "/".as_ref() + &auditor_name + "-notes";
            if Path::new(&auditor_folder_path).is_dir() {
                panic!(
                    "auditor folder {:?} already exist, aborting",
                    &auditor_folder_path
                );
            }
        }
    }

    // program_path not empty and program_path exists
    pub fn validate_program_path() {
        let bat_config = BatConfig::get_config().required;
        if bat_config.program_lib_path.is_empty() {
            panic!("required parameter program_path is empty in Bat.toml file, aborting",);
        } else if !Path::new(&bat_config.program_lib_path).is_file() {
            panic!(
                "program file at path \"{:?}\" does not exist, aborting, please update Bat.toml file",
                &bat_config.program_lib_path
            );
        }
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
            auditor_name: "matias".to_string(),
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
