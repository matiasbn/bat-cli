use std::path::Path;

use std::{fs, process::Command};

use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

use crate::config::{AUDITOR_TOML_INITIAL_CONFIG_STR, BAT_TOML_INITIAL_CONFIG_STR};

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";
pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub fn create_project() {
    // get project name
    let project_name = get_project_name();
    println!("Creating {} project", project_name);
    // clone repository
    clone_repository(project_name.clone());
    // create config files
    create_bat_toml(project_name.clone());
    create_auditor_toml();
    // move config files to repo
    move_config_files(project_name.clone());
    println!("Project {} succesfully created", project_name);
}

fn get_project_name() -> String {
    let project_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Project name:")
        .interact_text()
        .unwrap();

    if Path::new(&project_name).is_dir() {
        panic!("Project already exists");
    }
    project_name
}

fn clone_repository(project_name: String) {
    // Clone git repository
    Command::new("git")
        .args(["clone", "git@git.kudelski.com:TVRM/bat-base-repository.git"])
        .output()
        .unwrap();
    // change folder name
    Command::new("mv")
        .args(["bat-base-repository", project_name.as_str()])
        .output()
        .unwrap();
    // Remove .git folder
    Command::new("rm")
        .args(["-rf", (project_name + "/.git").as_str()])
        .output()
        .unwrap();
}

fn create_bat_toml(project_name: String) {
    let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);

    if bat_toml_path.exists() {
        panic!(
            "Bat.toml file already exist in {:?}, aborting",
            bat_toml_path
        )
    };

    // set project name
    let bat_toml_updated = BAT_TOML_INITIAL_CONFIG_STR.to_string().replace(
        &String::from("project_name = \""),
        &("project_name = \"".to_string() + &project_name),
    );

    fs::write(bat_toml_path.clone(), bat_toml_updated).expect("Could not write to file!");
}

pub fn create_auditor_toml() {
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

fn move_config_files(project_name: String) {
    Command::new("mv")
        .args(["Bat.toml", &project_name])
        .output()
        .unwrap();
    Command::new("mv")
        .args(["BatAuditor.toml", &project_name])
        .output()
        .unwrap();
}

#[test]
fn test_create_bat_toml() {
    let project_name = "test_project".to_string();
    create_bat_toml(project_name);
}
