use super::CommandError;
use crate::batbelt::cli_inputs;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::templates::TemplateGenerator;
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::Result;
use error_stack::{FutureExt, IntoReport, Report, ResultExt};
use normalize_url::normalizer;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub fn create_project() -> Result<(), CommandError> {
    // get project config
    let bat_config = create_bat_config_file().change_context(CommandError)?;
    println!("Creating {:#?} project", bat_config);
    TemplateGenerator::create_project().change_context(CommandError)?;
    execute_command("mv", &["Bat.toml", &bat_config.project_name])?;

    println!("Project {} succesfully created", bat_config.project_name);
    Ok(())
}

fn create_bat_config_file() -> Result<BatConfig, CommandError> {
    let local_anchor_project_folders = WalkDir::new(".")
        .into_iter()
        .map(|f| f.unwrap())
        .filter(|f| {
            f.file_type().is_dir()
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
            let is_anchor_project = file_names.contains(&"Anchor.toml".to_string());
            is_anchor_project
        })
        .map(|f| f.path().to_str().unwrap().to_string())
        .collect::<Vec<_>>();
    if local_anchor_project_folders.is_empty() {
        let message = format!("No Anchor projects were found on the current working directory");
        return Err(Report::new(CommandError).attach_printable(message));
    }
    // Folder with the program to audit selection
    let prompt_text = "Select the folder with the program to audit";
    let selection = cli_inputs::select(prompt_text, local_anchor_project_folders.clone(), None)
        .change_context(CommandError)?;
    let selected_folder_path = &local_anchor_project_folders[selection];
    let cargo_programs_files_info = WalkDir::new(selected_folder_path)
        .into_iter()
        .map(|f| f.unwrap())
        .filter(|dir_entry| {
            dir_entry
                .file_name()
                .to_str()
                .unwrap()
                .contains("Cargo.toml")
                && !dir_entry.path().to_str().unwrap().contains("target")
        })
        .collect::<Vec<_>>();

    // Program to audit selection
    let prompt_text = "Select the program to audit";
    let cargo_programs_paths = cargo_programs_files_info
        .iter()
        .map(|f| {
            f.path()
                .to_str()
                .unwrap()
                .trim_end_matches("/Cargo.toml")
                .to_string()
        })
        .collect::<Vec<_>>();
    let selection = cli_inputs::select(prompt_text, cargo_programs_paths.clone(), None)
        .change_context(CommandError)?;
    let selected_program_path = &cargo_programs_paths[selection];
    log::debug!("selected_program: {:#?}", selected_program_path);
    let program_name = selected_program_path
        .split("/")
        .last()
        .unwrap()
        .to_string()
        .replace("_", "-");
    log::debug!("program_name: {:#?}", program_name);
    let program_lib_path = format!("{}/src/lib.rs", selected_program_path);
    log::debug!("program_lib_path: {:#?}", program_lib_path);
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

    let use_default = if !cfg!(debug_assertions) {
        cli_inputs::select_yes_or_no(prompt_text.as_str()).change_context(CommandError)?
    } else {
        true
    };

    if !use_default {
        project_name = cli_inputs::input("Project name:").change_context(CommandError)?;
    }
    let project_path = format!("./{project_name}");

    if Path::new(&project_path).is_dir() {
        return Err(Report::new(CommandError)
            .attach_printable(format!("Folder {} already exists", project_name)));
    }

    let auditor_names_prompt: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Auditor names (comma separated, example: alice,bob):")
            .change_context(CommandError)?
    } else {
        "test_user".to_string()
    };
    let auditor_names: Vec<String> = auditor_names_prompt
        .split(',')
        .map(|l| l.to_string())
        .collect();

    let client_name: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Client name:").change_context(CommandError)?
    } else {
        "test_client".to_string()
    };

    let commit_hash_url: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Commit hash url:").change_context(CommandError)?
    } else {
        "test_hash".to_string()
    };

    let starting_date: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Starting date, example: (01/01/2023):").change_context(CommandError)?
    } else {
        "test_date".to_string()
    };

    let mut miro_board_url: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Miro board url:").change_context(CommandError)?
    } else {
        "https://miro.com/app/board/uXjVPzsgmiY=/".to_string()
    };

    miro_board_url = normalize_miro_board_url(&miro_board_url)?;

    let project_repository_url: String = if !cfg!(debug_assertions) {
        cli_inputs::input("Project repo url, where this audit folder would be pushed:")
            .change_context(CommandError)?
    } else {
        "https://github.com/matiasbn/test-repo".to_string()
    };

    let bat_config = BatConfig {
        initialized: true,
        program_name,
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

fn normalize_miro_board_url(url_to_normalize: &str) -> Result<String, CommandError> {
    let url = normalizer::UrlNormalizer::new(url_to_normalize)
        .into_report()
        .change_context(CommandError)?
        .normalize(Some(&["moveToWidget", "cot"]))
        .into_report()
        .change_context(CommandError)?;
    Ok(url)
}

#[test]
fn test_normalize_url() {
    let test_url =
        "https://miro.com/app/board/uXjVPqatu4c=/?moveToWidget=3458764546015336005&cot=14";
    let normalized = normalizer::UrlNormalizer::new(test_url)
        .unwrap()
        .normalize(Some(&["moveToWidget", "cot"]))
        .unwrap();
    println!("normalized: \n{}", normalized)
}
