use std::{fs, path::Path, str};

use serde::Deserialize;

use crate::commands::create::{AUDITOR_TOML_INITIAL_PATH, BAT_TOML_INITIAL_PATH};

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
auditor_names = [""]
audit_folder_path = ""
program_lib_path = ""
base_repository_url = "git@github.com:matiasbn/base-repository.git"
notes_repository_url = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name=""
"#;

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
    pub base_repository_url: String,
    pub notes_repository_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuditorConfig {
    pub auditor_name: String,
}

trait AuditorConfigValidation {
    fn validate_auditor_config_exists() -> bool {
        true
    }
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

    pub fn get_auditors_names() -> Vec<String> {
        Self::get_config().required.auditor_names
    }

    pub fn get_auditor_name() -> String {
        Self::get_config().auditor.auditor_name
    }

    pub fn get_audit_folder_path() -> String {
        Self::get_config().required.audit_folder_path
    }

    pub fn get_program_lib_path() -> String {
        Self::get_config().required.program_lib_path
    }

    pub fn get_notes_path() -> String {
        Self::get_audit_folder_path() + "/notes/"
    }

    pub fn get_auditor_notes_path() -> String {
        Self::get_notes_path() + &Self::get_auditor_name() + "-notes/"
    }

    // Findings paths
    pub fn get_auditor_findings_path() -> String {
        Self::get_auditor_notes_path() + "findings/"
    }

    pub fn get_auditor_findings_to_review_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_findings_path() + "to-review/" + &name.replace(".md", "") + ".md"
            }
            None => Self::get_auditor_findings_path() + "to-review/",
        }
    }

    pub fn get_auditor_findings_accepted_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_findings_path() + "accepted/" + &name.replace(".md", "") + ".md"
            }
            None => Self::get_auditor_findings_path() + "accepted/",
        }
    }

    pub fn get_auditor_findings_rejected_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_findings_path() + "rejected/" + &name.replace(".md", "") + ".md"
            }
            None => Self::get_auditor_findings_path() + "rejected/",
        }
    }

    // Code overhaul paths
    pub fn get_auditor_code_overhaul_path() -> String {
        Self::get_auditor_notes_path() + "code-overhaul/"
    }

    pub fn get_auditor_code_overhaul_to_review_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_code_overhaul_path()
                    + "to-review/"
                    + &name.replace(".md", "")
                    + ".md"
            }
            None => Self::get_auditor_code_overhaul_path() + "to-review/",
        }
    }

    pub fn get_auditor_code_overhaul_finished_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_code_overhaul_path()
                    + "finished/"
                    + &name.replace(".md", "")
                    + ".md"
            }
            None => Self::get_auditor_code_overhaul_path() + "finished/",
        }
    }

    // Templates path
    pub fn get_templates_path() -> String {
        Self::get_audit_folder_path() + "/templates"
    }

    pub fn get_finding_template_path() -> String {
        Self::get_templates_path() + "/finding.md"
    }

    pub fn get_informational_template_path() -> String {
        Self::get_templates_path() + "/informational.md"
    }

    pub fn get_code_overhaul_template_path() -> String {
        Self::get_templates_path() + "/code-overhaul.md"
    }
}

pub trait BatConfigValidation {
    fn validate_bat_config();
}

impl BatConfigValidation for BatConfig {
    fn validate_bat_config() {
        let bat_config = BatConfig::get_config();
        let BatConfig { required, auditor } = bat_config;
        // Validate required
        if required.program_lib_path.is_empty() {
            panic!("required parameter program_lib_path is empty at Bat.toml");
        }
        if required.audit_folder_path.is_empty() {
            panic!("required parameter audit_folder_path is empty at Bat.toml");
        }
        if required.auditor_names.is_empty() {
            panic!("required parameter auditor_names is empty at Bat.toml");
        }
        if required.base_repository_url.is_empty() {
            panic!("required parameter base_repository is empty at Bat.toml");
        }
        if required.notes_repository_url.is_empty() {
            panic!("required parameter notes_repository_url is empty at Bat.toml");
        }

        // Validate auditor
        if auditor.auditor_name.is_empty() {
            panic!("required parameter auditor_name is empty at BatAuditor.toml");
        }
    }
}

pub trait InitConfigValidation {
    fn validate_init_config();

    fn validate_audit_folder();

    fn validate_auditor_notes_folder();

    fn validate_program_path();
}

impl InitConfigValidation for BatConfig {
    fn validate_init_config() {
        Self::validate_audit_folder();
        Self::validate_auditor_notes_folder();
        Self::validate_program_path();
    }

    // audit notes folder should not exist
    fn validate_audit_folder() {
        let bat_config = BatConfig::get_config().required;
        if Path::new(&bat_config.audit_folder_path).is_dir() {
            panic!(
                "audit folder {:?} already exists, abortings",
                &bat_config.audit_folder_path
            );
        }
    }

    // auditors notes folders should not exist and not empty
    fn validate_auditor_notes_folder() {
        let bat_config = BatConfig::get_config().required;
        if bat_config.auditor_names.is_empty() {
            panic!("required parameter auditors_names is empty in Bat.toml file, aborting",);
        }
        for auditor_name in &bat_config.auditor_names {
            let auditor_folder_path =
                bat_config.audit_folder_path.clone() + "/" + auditor_name + "-notes";
            if Path::new(&auditor_folder_path).is_dir() {
                panic!(
                    "auditor folder {:?} already exist, aborting",
                    &auditor_folder_path
                );
            }
        }
    }

    // program_path not empty and program_path exists
    fn validate_program_path() {
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
}

pub trait TestConfig {
    fn get_test_config() -> BatConfig;
}

impl TestConfig for BatConfig {
    fn get_test_config() -> BatConfig {
        let required = RequiredConfig {
            auditor_names: vec!["matias".to_string(), "porter".to_string()],
            audit_folder_path: "../audit-notes".to_string(),
            program_lib_path:
                "../star-atlas-programs/sol-programs/scream/programs/player_profile/src/lib.rs"
                    .to_string(),
            base_repository_url: "git@github.com:matiasbn/base-repository.git".to_string(),
            notes_repository_url: "git@github.com:bad-user/bad-url.git".to_string(),
        };
        let auditor = AuditorConfig {
            auditor_name: "matias".to_string(),
        };

        BatConfig { required, auditor }
    }
}

#[test]
fn test_get_test_config() {
    let batconfig = BatConfig::get_test_config();
    println!("{:#?}", batconfig);
}
