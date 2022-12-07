use std::{fs, path::Path, str};

use serde::Deserialize;

use crate::commands::create::{
    create_auditor_toml, AUDITOR_TOML_INITIAL_PATH, BAT_TOML_INITIAL_PATH,
};

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
project_name = ""
auditor_names = [""]
audit_folder_path = "."
program_lib_path = ""
project_repository_url = ""
[optional]
program_instructions_path = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name = ""
"#;

#[derive(Debug, Deserialize, Clone)]
pub struct BatConfig {
    pub required: RequiredConfig,
    pub optional: OptionalConfig,
    pub auditor: AuditorConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RequiredConfig {
    pub project_name: String,
    pub auditor_names: Vec<String>,
    pub audit_folder_path: String,
    pub program_lib_path: String,
    pub project_repository_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct OptionalConfig {
    pub program_instructions_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuditorConfig {
    pub auditor_name: String,
}

impl BatConfig {
    pub fn get_validated_config() -> BatConfig {
        let bat_config = Self::get_bat_config();
        Self::validate_bat_config(bat_config.clone(), true);
        bat_config
    }

    pub fn get_init_config() -> BatConfig {
        let bat_config: BatConfig = Self::get_bat_config();
        Self::validate_bat_config(bat_config.clone(), false);
        bat_config
    }

    fn get_bat_config() -> BatConfig {
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
            // if BatAuditor does not exist, create it
            create_auditor_toml();
            println!("BatAuditor.toml file not detected, creating")
        }
        let auditor_toml_file = fs::read(auditor_toml_path).unwrap();
        let auditor_tom_file_string = str::from_utf8(auditor_toml_file.as_slice()).unwrap();

        // Get the BatConfig complete
        let config: BatConfig =
            toml::from_str((bat_tom_file_string.to_string() + auditor_tom_file_string).as_str())
                .unwrap();
        config
        // // Get the BatConfig complete
        // let config =
        //     toml::from_str((bat_tom_file_string.to_string() + auditor_tom_file_string).as_str());
        // println!("err {:#?}", config);
        // match config {
        //     Ok(bat_config) => bat_config,
        //     Err(e) => {
        //         println!("error {}", e);
        //         Err(())
        //     }
        // }
    }

    fn validate_bat_config(bat_config: BatConfig, validate_auditor: bool) {
        let BatConfig {
            required, auditor, ..
        } = bat_config;
        // Validate required
        if required.project_name.is_empty() {
            panic!("required parameter project_name is empty at Bat.toml");
        }
        if required.program_lib_path.is_empty() {
            panic!("required parameter program_lib_path is empty at Bat.toml");
        }
        if required.audit_folder_path.is_empty() {
            panic!("required parameter audit_folder_path is empty at Bat.toml");
        }
        if required.auditor_names.is_empty() {
            panic!("required parameter auditor_names is empty at Bat.toml");
        }
        if required.project_repository_url.is_empty() {
            panic!("required parameter notes_repository_url is empty at Bat.toml");
        }

        // Validate auditor
        if validate_auditor {
            if auditor.auditor_name.is_empty() {
                panic!("required parameter auditor_name is empty at BatAuditor.toml");
            }
        }
    }

    pub fn get_auditors_names() -> Vec<String> {
        Self::get_validated_config().required.auditor_names
    }

    pub fn get_auditor_name() -> String {
        Self::get_validated_config().auditor.auditor_name
    }

    pub fn get_audit_folder_path() -> String {
        Self::get_validated_config().required.audit_folder_path
    }

    pub fn get_program_lib_path() -> String {
        Self::get_validated_config().required.program_lib_path
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

    pub fn get_auditor_code_overhaul_started_path(file_name: Option<String>) -> String {
        match file_name {
            Some(name) => {
                Self::get_auditor_code_overhaul_path()
                    + "started/"
                    + &name.replace(".md", "")
                    + ".md"
            }
            None => Self::get_auditor_code_overhaul_path() + "started/",
        }
    }

    // Templates path
    pub fn get_templates_path() -> String {
        Self::get_audit_folder_path() + "/templates"
    }

    pub fn get_notes_folder_template_path() -> String {
        Self::get_templates_path() + "/notes-folder-template"
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

pub trait TestConfig {
    fn get_test_config() -> BatConfig;
}

impl TestConfig for BatConfig {
    fn get_test_config() -> BatConfig {
        let required = RequiredConfig {
            project_name: "test_project".to_string(),
            auditor_names: vec!["matias".to_string(), "porter".to_string()],
            audit_folder_path: "../audit-notes".to_string(),
            program_lib_path:
                "../star-atlas-programs/sol-programs/scream/programs/player_profile/src/lib.rs"
                    .to_string(),
            project_repository_url: "git@github.com:bad-user/bad-url.git".to_string(),
        };
        let optional = OptionalConfig {
            program_instructions_path: "".to_string(),
        };
        let auditor = AuditorConfig {
            auditor_name: "matias".to_string(),
        };

        BatConfig {
            required,
            optional,
            auditor,
        }
    }
}

#[test]
fn test_get_test_config() {
    let batconfig = BatConfig::get_test_config();
    println!("{:#?}", batconfig);
}
