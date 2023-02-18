use super::CommandError;
use crate::batbelt::bash::execute_command;
use crate::batbelt::constants::BASE_REPOSTORY_NAME;
use crate::batbelt::git::clone_base_repository;
use crate::batbelt::helpers::fs_read_dir;
use crate::batbelt::structs::FileInfo;
use crate::batbelt::{cli_inputs, helpers};
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use std::path::Path;
use std::{fs, process::Command};
use walkdir::WalkDir;

pub fn create_project() -> error_stack::Result<(), CommandError> {
    // get project config
    let bat_config = create_bat_config_file().change_context(CommandError)?;
    println!("Creating {:#?} project", bat_config);
    // clone repository
    clone_base_repository();
    // change folder name
    execute_command("mv", &[BASE_REPOSTORY_NAME, &bat_config.project_name])?;
    // Remove .git folder
    execute_command("rm", &["-rf", &format!("{}/.git", bat_config.project_name)])?;
    execute_command("mv", &["Bat.toml", &bat_config.project_name])?;

    println!("Project {} succesfully created", bat_config.project_name);
    Ok(())
}

fn create_bat_config_file() -> error_stack::Result<BatConfig, CommandError> {
    let local_folders = fs_read_dir(".")
        .change_context(CommandError)?
        .map(|f| f.unwrap())
        .filter(|f| {
            f.file_type().unwrap().is_dir()
                && ![".", "target"]
                    .iter()
                    .any(|y| f.file_name().to_str().unwrap().contains(y))
        })
        .filter(|f| {
            let path = f.path();
            let dir = fs::read_dir(path).unwrap();
            let file_names = dir
                .map(|f| f.unwrap().file_name().to_str().unwrap().to_string())
                .collect::<Vec<_>>();
            let is_bat_project = file_names.contains(&"Bat.toml".to_string());
            !is_bat_project
        })
        .map(|f| f.file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    // Folder with the program to audit selection
    let prompt_text = "Select the folder with the program to audit";
    let selection = cli_inputs::select(prompt_text, local_folders.clone(), None)
        .change_context(CommandError)?;
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
    let selection =
        cli_inputs::select(prompt_text, cargo_programs_paths, None).change_context(CommandError)?;
    let selected_program = &cargo_programs_files_info[selection];
    let program_name = selected_program
        .path
        .split("/")
        .last()
        .unwrap()
        .replace("_", "-");
    let program_lib_path = selected_program.path.clone() + "/src/lib.rs";

    let normalized_to_audit_program_lib_path = program_lib_path.replace("./", "../");

    if !Path::new(&program_lib_path).is_file() {
        return Err(
            Report::new(CommandError).attach_printable("lib.rs file not found in selected folder")
        );
    }

    // Project name selection
    let mut project_name: String = program_name.to_owned() + "-audit";
    let prompt_text = format!(
        "Do you want to use the name {} for this project?",
        format!("{project_name}").yellow()
    );
    let options = vec!["yes", "no"];
    let selection =
        cli_inputs::select(prompt_text.as_str(), options, None).change_context(CommandError)?;
    if selection == 1 {
        project_name = cli_inputs::input("Project name:").change_context(CommandError)?;
    }
    let project_path = format!("./{project_name}");

    if Path::new(&project_path).is_dir() {
        return Err(Report::new(CommandError)
            .attach_printable(format!("Folder {} already exists", project_name)));
    }

    let auditor_names_prompt: String =
        cli_inputs::input("Auditor names (comma separated, example: alice,bob):")
            .change_context(CommandError)?;
    let auditor_names: Vec<String> = auditor_names_prompt
        .split(',')
        .map(|l| l.to_string())
        .collect();
    let client_name: String = cli_inputs::input("Client name:").change_context(CommandError)?;
    let commit_hash_url: String =
        cli_inputs::input("Commit hash url:").change_context(CommandError)?;
    let starting_date: String =
        cli_inputs::input("Starting date, example: (01/01/2023):").change_context(CommandError)?;
    let mut miro_board_url: String =
        cli_inputs::input("Miro board url:").change_context(CommandError)?;
    miro_board_url = helpers::normalize_url(&miro_board_url)
        .change_context(CommandError)
        .change_context(CommandError)?;

    // let miro_board_id = "https://miro.com/app/board/".to_string() + &miro_board_id;
    let project_repository_url: String =
        cli_inputs::input("Project repo url, where this audit folder would be pushed:")
            .change_context(CommandError)?;
    let bat_config = BatConfig {
        initialized: true,
        auditor_names,
        project_name,
        client_name,
        miro_board_url,
        starting_date,
        commit_hash_url,
        project_repository_url,
        program_lib_path: normalized_to_audit_program_lib_path,
    };
    bat_config.save().change_context(CommandError)?;
    Ok(bat_config)
}
