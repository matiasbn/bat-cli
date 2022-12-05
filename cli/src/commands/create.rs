use std::path::Path;
use std::{fs, process::Command};

use crate::config::{AUDITOR_TOML_INITIAL_CONFIG_STR, BAT_TOML_INITIAL_CONFIG_STR};

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";
pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub const GIT_IGNORE_STR: &str = r#"BatAuditor.toml"#;

pub fn create_project() {
    // command line Bat.toml and BatAuditor.toml
    // create config files
    // clone the repo
    // move config files to repo
    create_bat_toml();
    create_auditor_toml();
    create_gitignore();
    println!("Bat.toml created at {:?}", BAT_TOML_INITIAL_PATH.clone());
    println!(
        "BatAuditor.toml created at {:?}",
        AUDITOR_TOML_INITIAL_PATH.clone()
    );
}

fn create_bat_toml() {
    let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);

    if bat_toml_path.exists() {
        panic!(
            "Bat.toml file already exist in {:?}, aborting",
            bat_toml_path
        )
    };

    fs::write(bat_toml_path.clone(), BAT_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
}

fn create_auditor_toml() {
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);

    if auditor_toml_path.exists() {
        panic!(
            "BatAudit.toml file already exist in {:?}, aborting",
            auditor_toml_path
        )
    };

    fs::write(auditor_toml_path.clone(), AUDITOR_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
}

fn create_gitignore() {
    let gitignore_toml_path = Path::new(&".gitignore");

    if gitignore_toml_path.exists() {
        println!(
            ".gitignore file already exist in {:?}, please add BatAuditor.toml",
            gitignore_toml_path
        );
        // Command::new("echo").args([
        //     "-n".to_string(),
        //     "BatAuditor.toml'".to_string(),
        //     ">>".to_string(),
        //     ".gitignore".to_string(),
        // ]);
    };

    fs::write(gitignore_toml_path.clone(), GIT_IGNORE_STR).expect("Could not write to file!");
}
