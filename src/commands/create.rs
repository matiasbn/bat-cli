use std::error::Error;
use std::io::Result;
use std::path::Path;
use std::{fs, io, process::Command};

use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Input;
use walkdir::WalkDir;

use crate::commands::git::clone_base_repository;
use crate::config::RequiredConfig;
use crate::constants::{
    AUDITOR_TOML_INITIAL_CONFIG_STR, BASE_REPOSTORY_NAME, BAT_TOML_INITIAL_CONFIG_STR,
};
use crate::structs::FileInfo;
use crate::utils::cli_inputs;

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";

pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub fn create_project() -> Result<()> {
    // get project config
    let project_config = get_project_config()?;
    println!("Creating {project_config:#?} project");
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
    Ok(())
}

fn get_project_config() -> Result<RequiredConfig> {
    let local_folders = fs::read_dir(".")
        .unwrap()
        .map(|f| f.unwrap())
        .filter(|f| {
            f.file_type().unwrap().is_dir()
                && ![".", "target"]
                    .iter()
                    .any(|y| f.file_name().to_str().unwrap().contains(y))
        })
        .map(|f| f.file_name().into_string().unwrap())
        .collect::<Vec<_>>();

    // Folder with the program to audit selection
    let prompt_text = "Select the folder with the program to audit";
    let selection = cli_inputs::select(prompt_text, local_folders.clone(), None)?;
    let selected_folder = &local_folders[selection];
    let cargo_programs_files_info = WalkDir::new(format!("./{}", selected_folder))
        .into_iter()
        .map(|entry| {
            let info = FileInfo {
                path: entry
                    .as_ref()
                    .unwrap()
                    .path()
                    .display()
                    .to_string()
                    .replace("/Cargo.toml", ""),
                name: entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_os_string()
                    .into_string()
                    .unwrap(),
            };
            info
        })
        .filter(|file_info| {
            file_info.name.contains("Cargo.toml") && !file_info.path.contains("target")
        })
        .collect::<Vec<FileInfo>>();

    // Program to audit selection
    let prompt_text = "Select the program to audit";
    let cargo_programs_paths = cargo_programs_files_info
        .iter()
        .map(|f| f.path.clone())
        .collect::<Vec<_>>();
    let selection = cli_inputs::select(prompt_text, cargo_programs_paths, None)?;
    let selected_program = &cargo_programs_files_info[selection];
    let program_name = selected_program.path.split("/").last().unwrap();
    let lib_path = selected_program.path.clone() + "/src/lib.rs";

    if !Path::new(&lib_path).is_file() {
        panic!("lib.rs file not found in selected folder");
    }

    // Project name selection
    let mut project_name: String = program_name.to_owned() + "-audit";
    let prompt_text = format!(
        "Do you want to use the name {} for this project?",
        format!("{project_name}").yellow()
    );
    let options = vec!["yes", "no"];
    let selection = cli_inputs::select(prompt_text.as_str(), options, None)?;
    if selection == 1 {
        project_name = cli_inputs::input("Project name:")?;
    }
    let project_path = format!("./{project_name}");

    if Path::new(&project_path).is_dir() {
        panic!("Folder {} already exists", project_name);
    }

    let auditor_names_prompt: String =
        cli_inputs::input("Auditor names (comma separated, example: alice,bob):")?;
    let auditor_names: Vec<String> = auditor_names_prompt
        .split(',')
        .map(|l| l.to_string())
        .collect();
    let client_name: String = cli_inputs::input("Client name:")?;
    let commit_hash_url: String = cli_inputs::input("Commit hash url:")?;
    let starting_date: String = cli_inputs::input("Starting date, example: (01/01/2023):")?;
    let miro_board_id: String = cli_inputs::input("Miro board url:")?;
    let miro_board_url = "https://miro.com/app/board/".to_string() + &miro_board_id;
    let project_repository_url: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Project repo url, where this audit folder would be pushed:")
        .interact_text()
        .unwrap();

    Ok(RequiredConfig {
        auditor_names,
        project_name,
        client_name,
        miro_board_url,
        miro_board_id,
        starting_date,
        commit_hash_url,
        project_repository_url,
        audit_folder_path: "".to_string(),
        program_lib_path: "".to_string(),
    })
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
        panic!("Bat.toml file already exist in {bat_toml_path:?}, aborting")
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
            &("auditor_names = [\"".to_string() + &auditor_names.join("\",\"")),
        );

    fs::write(bat_toml_path, bat_toml_updated).expect("Could not write to file!");
}

pub fn create_auditor_toml() {
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);

    if auditor_toml_path.exists() {
        panic!("BatAudit.toml file already exist in {auditor_toml_path:?}, aborting")
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
