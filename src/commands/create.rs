use std::path::Path;

use std::{fs, process::Command};

use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;

use crate::config::RequiredConfig;
use crate::constants::{
    AUDITOR_TOML_INITIAL_CONFIG_STR, BASE_REPOSTORY_NAME, BAT_TOML_INITIAL_CONFIG_STR,
};
use crate::git::clone_base_repository;

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";

pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub fn create_project() {
    // get project config
    let project_config = get_project_config();
    println!("Creating {:#?} project", project_config);
    // clone repository
    clone_base_repository();
    // change folder name
    Command::new("mv")
        .args([BASE_REPOSTORY_NAME, project_config.project_name.as_str()])
        .output()
        .unwrap();
    // Remove .git folder
    Command::new("rm")
        .args([
            "-rf",
            (project_config.project_name.clone() + "/.git").as_str(),
        ])
        .output()
        .unwrap();
    // create config files
    create_bat_toml(project_config.clone());
    create_auditor_toml();
    // move config files to repo
    move_config_files(project_config.project_name.clone());

    println!(
        "Project {} succesfully created",
        project_config.project_name
    );
}

fn get_project_config() -> RequiredConfig {
    let project_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Project name:")
        .interact_text()
        .unwrap();

    if Path::new(&project_name).is_dir() {
        panic!("Project already exists");
    }

    let auditor_names_prompt: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Auditor names (comma separated, example: alice,bob):")
        .interact_text()
        .unwrap();
    let auditor_names: Vec<String> = auditor_names_prompt
        .split(',')
        .map(|l| l.to_string())
        .collect();
    let client_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Client name:")
        .interact_text()
        .unwrap();
    let commit_hash_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Commit hash url:")
        .interact_text()
        .unwrap();
    let starting_date: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Starting date, example: (01/01/2023):")
        .interact_text()
        .unwrap();
    let miro_board_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Miro board url:")
        .interact_text()
        .unwrap();
    let project_repository_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Project repo url, where this audit folder would be pushed:")
        .interact_text()
        .unwrap();

    RequiredConfig {
        auditor_names,
        project_name,
        client_name,
        miro_board_url,
        starting_date,
        commit_hash_url,
        project_repository_url,
        audit_folder_path: "".to_string(),
        program_lib_path: "".to_string(),
    }
}

fn create_bat_toml(project_config: RequiredConfig) {
    let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);
    let RequiredConfig {
        project_name,
        client_name,
        commit_hash_url,
        starting_date,
        auditor_names,
        program_lib_path,
        project_repository_url,
        miro_board_url,
        ..
    } = project_config;

    if bat_toml_path.exists() {
        panic!(
            "Bat.toml file already exist in {:?}, aborting",
            bat_toml_path
        )
    };

    // set project name
    let bat_toml_updated = BAT_TOML_INITIAL_CONFIG_STR
        .to_string()
        .replace(
            &String::from("project_name = \""),
            &("project_name = \"".to_string() + &project_name),
        )
        .replace(
            &String::from("client_name = \""),
            &("client_name = \"".to_string() + &client_name),
        )
        .replace(
            &String::from("commit_hash_url = \""),
            &("commit_hash_url = \"".to_string() + &commit_hash_url),
        )
        .replace(
            &String::from("starting_date = \""),
            &("starting_date = \"".to_string() + &starting_date),
        )
        .replace(
            &String::from("program_lib_path = \""),
            &("program_lib_path = \"".to_string() + &program_lib_path),
        )
        .replace(
            &String::from("project_repository_url = \""),
            &("project_repository_url = \"".to_string() + &project_repository_url),
        )
        .replace(
            &String::from("miro_board_url = \""),
            &("miro_board_url = \"".to_string() + &miro_board_url),
        )
        .replace(
            &String::from("auditor_names = [\""),
            &("auditor_names = [\"".to_string() + &auditor_names.join(",")),
        );

    fs::write(bat_toml_path, bat_toml_updated).expect("Could not write to file!");
}

pub fn create_auditor_toml() {
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);

    if auditor_toml_path.exists() {
        panic!(
            "BatAudit.toml file already exist in {:?}, aborting",
            auditor_toml_path
        )
    };

    fs::write(auditor_toml_path, AUDITOR_TOML_INITIAL_CONFIG_STR)
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
