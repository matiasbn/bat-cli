use std::fs::{self};

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::string::String;

use colored::Colorize;

use crate::batbelt;
use crate::batbelt::bash::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::constants::{
    AUDIT_INFORMATION_CLIENT_NAME_PLACEHOLDER, AUDIT_INFORMATION_COMMIT_HASH_PLACEHOLDER,
    AUDIT_INFORMATION_MIRO_BOARD_PLACEHOLER, AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER,
    AUDIT_INFORMATION_STARTING_DATE_PLACEHOLDER, MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT,
    MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::entrypoint::EntrypointParser;
use crate::commands::CommandError;
use crate::config::{BatAuditorConfig, BatConfig};

use crate::batbelt::git::GitCommit;
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::path::{BatFile, BatFolder};

use error_stack::{Report, Result, ResultExt};

pub async fn initialize_bat_project(skip_initial_commit: bool) -> Result<(), CommandError> {
    let _bat_config = BatConfig::get_config().change_context(CommandError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    if !Path::new("BatAuditor.toml").is_file() || !bat_auditor_config.initialized {
        prompt_auditor_options()?;
    }
    let bat_config: BatConfig = BatConfig::get_config().change_context(CommandError)?;
    println!("creating project for the next config: ");
    println!("{:#?}", bat_config);
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .unwrap()
        .stdout;
    let git_initialized: bool = from_utf8(&output).unwrap() == "true\n";
    if !git_initialized {
        println!("Initializing project repository");
        initialize_project_repository()?;
        println!("Project repository successfully initialized");
    }

    let readme_path =
        batbelt::path::get_file_path(BatFile::Readme, true).change_context(CommandError)?;
    let readme_string = fs::read_to_string(readme_path.clone()).unwrap();

    if readme_string.contains(AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER) {
        println!("Updating README.md");
        update_readme_file()?;
    }
    Command::new("git")
        .args(["add", &readme_path])
        .output()
        .unwrap();

    // create auditors branches from develop
    for auditor_name in bat_config.auditor_names {
        let auditor_project_branch_name = format!("{}-{}", auditor_name, bat_config.project_name);
        let auditor_project_branch_exists =
            batbelt::git::check_if_branch_exists(&auditor_project_branch_name)
                .change_context(CommandError)?;
        if !auditor_project_branch_exists {
            println!("Creating branch {:?}", auditor_project_branch_name);
            // checkout develop to create auditor project branch from there
            Command::new("git")
                .args(["checkout", "develop"])
                .output()
                .unwrap();
            Command::new("git")
                .args(["checkout", "-b", &auditor_project_branch_name])
                .output()
                .unwrap();
        }
    }
    let auditor_project_branch_name =
        batbelt::git::get_expected_current_branch().change_context(CommandError)?;
    println!("Checking out {:?} branch", auditor_project_branch_name);
    // checkout auditor branch
    Command::new("git")
        .args(["checkout", auditor_project_branch_name.as_str()])
        .output()
        .unwrap();

    // validate_init_config()?;
    // let auditor_notes_folder = utils::path::get_auditor_notes_path()?;
    let auditor_notes_folder = batbelt::path::get_folder_path(BatFolder::AuditorNotes, false)
        .change_context(CommandError)?;
    let auditor_notes_folder_exists = Path::new(&auditor_notes_folder).is_dir();
    if auditor_notes_folder_exists {
        let auditor_notes_files =
            batbelt::helpers::get::get_only_files_from_folder(auditor_notes_folder.clone())
                .change_context(CommandError)?;
        if auditor_notes_files.is_empty() {
            create_auditor_notes_folder()?;
            // create overhaul files
            initialize_code_overhaul_files()?;
            // commit to-review files
            create_miro_frames_for_entrypoints().await?;
        }
    } else {
        create_auditor_notes_folder()?;
        // create overhaul files
        initialize_code_overhaul_files()?;
        // commit to-review files
        create_miro_frames_for_entrypoints().await?;
    }

    println!("Project successfully initialized");
    if !skip_initial_commit {
        batbelt::git::create_git_commit(GitCommit::InitAuditor, None)
            .change_context(CommandError)?;
    }
    // let lib_file_path = utils::path::get_program_lib_path()?;
    let lib_file_path =
        batbelt::path::get_file_path(BatFile::ProgramLib, true).change_context(CommandError)?;
    // Open lib.rs file in vscode
    vs_code_open_file_in_current_window(PathBuf::from(lib_file_path).to_str().unwrap())
        .change_context(CommandError)?;
    Ok(())
}

fn prompt_auditor_options() -> Result<(), CommandError> {
    let bat_config = BatConfig::get_config().change_context(CommandError)?;
    let auditor_names = bat_config.auditor_names;
    let prompt_text = format!("Select your name:");
    let selection = batbelt::cli_inputs::select(&prompt_text, auditor_names.clone(), None)?;
    let auditor_name = auditor_names.get(selection).unwrap().clone();
    println!(
        "Is great to have you here {}!",
        format!("{}", auditor_name).green()
    );
    let prompt_text = "Do you want to use the Miro integration?";
    let include_miro =
        batbelt::cli_inputs::select_yes_or_no(prompt_text).change_context(CommandError)?;
    let moat = if include_miro {
        let prompt_text = "Miro OAuth access token";
        batbelt::cli_inputs::input(&prompt_text).change_context(CommandError)?
    } else {
        "".to_string()
    };
    let prompt_text = "Do you want to use the VS Code integration?";
    let include_vs_code =
        batbelt::cli_inputs::select_yes_or_no(prompt_text).change_context(CommandError)?;
    let mut bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    bat_auditor_config.initialized = true;
    bat_auditor_config.auditor_name = auditor_name.to_string();
    bat_auditor_config.vs_code_integration = include_vs_code;
    bat_auditor_config.miro_oauth_access_token = moat;
    bat_auditor_config.save().change_context(CommandError)?;
    Ok(())
}

fn update_readme_file() -> Result<(), CommandError> {
    let BatConfig {
        project_name,
        client_name,
        commit_hash_url,
        starting_date,
        miro_board_url,
        ..
    } = BatConfig::get_config().change_context(CommandError)?;
    let readme_path =
        batbelt::path::get_file_path(BatFile::Readme, true).change_context(CommandError)?;
    let data = fs::read_to_string(readme_path.clone()).unwrap();
    let updated_readme = data
        .replace(AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER, &project_name)
        .replace(AUDIT_INFORMATION_CLIENT_NAME_PLACEHOLDER, &client_name)
        .replace(AUDIT_INFORMATION_COMMIT_HASH_PLACEHOLDER, &commit_hash_url)
        .replace(AUDIT_INFORMATION_MIRO_BOARD_PLACEHOLER, &miro_board_url)
        .replace(AUDIT_INFORMATION_STARTING_DATE_PLACEHOLDER, &starting_date);
    fs::write(readme_path, updated_readme).expect("Could not write to file README.md");
    Ok(())
}

fn initialize_project_repository() -> Result<(), CommandError> {
    let bat_config = BatConfig::get_config().change_context(CommandError)?;
    let _bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    // git init
    execute_command("git", &["init"]).change_context(CommandError)?;

    println!("Adding project repository as remote");
    execute_command(
        "git",
        &[
            "remote",
            "add",
            "origin",
            bat_config.project_repository_url.as_str(),
        ],
    )
    .change_context(CommandError)?;

    println!("Commit all to main");
    execute_command("git", &["add", "-A"]).change_context(CommandError)?;
    execute_command("git", &["commit", "-m", "initial commit"]).change_context(CommandError)?;

    println!("Creating develop branch");
    execute_command("git", &["checkout", "-b", "develop"]).change_context(CommandError)?;

    Ok(())
}

fn create_auditor_notes_folder() -> Result<(), CommandError> {
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    println!(
        "creating auditor notes folder for {}",
        bat_auditor_config.auditor_name.red()
    );
    execute_command(
        "cp",
        &[
            "-r",
            batbelt::path::get_folder_path(BatFolder::NotesTemplate, false)
                .change_context(CommandError)?
                .as_str(),
            batbelt::path::get_folder_path(BatFolder::AuditorNotes, false)
                .change_context(CommandError)?
                .as_str(),
        ],
    )
    .change_context(CommandError)?;

    Ok(())
}

pub fn initialize_code_overhaul_files() -> Result<(), CommandError> {
    let entrypoints_names = EntrypointParser::get_entrypoints_names(false).unwrap();

    for entrypoint_name in entrypoints_names {
        create_overhaul_file(entrypoint_name.clone())?;
    }
    Ok(())
}

pub async fn create_miro_frames_for_entrypoints() -> Result<(), CommandError> {
    let miro_config = MiroConfig::new().change_context(CommandError)?;
    if !miro_config.miro_enabled() {
        return Ok(());
    }

    let user_want_to_deploy = batbelt::cli_inputs::select_yes_or_no(
        "Do you want to deploy the Miro frames for code overhaul?",
    )
    .change_context(CommandError)?;

    if !user_want_to_deploy {
        println!("Ok, skipping code-overhaul Miro frames deployment....");
        return Ok(());
    }

    let miro_board_frames = MiroFrame::get_frames_from_miro()
        .await
        .change_context(CommandError)?;

    let entrypoints_names =
        EntrypointParser::get_entrypoints_names(false).change_context(CommandError)?;

    for (entrypoint_index, entrypoint_name) in entrypoints_names.iter().enumerate() {
        let frame_already_deployed = miro_board_frames
            .iter()
            .any(|frame| &frame.title == entrypoint_name);
        if !frame_already_deployed {
            println!("Creating frame in Miro for {}", entrypoint_name.green());
            let mut miro_frame =
                MiroFrame::new(&entrypoint_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
            miro_frame.deploy().await.change_context(CommandError)?;
            let x_modifier = entrypoint_index as i64 % MIRO_BOARD_COLUMNS;
            let y_modifier = entrypoint_index as i64 / MIRO_BOARD_COLUMNS;
            let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 100) * x_modifier;
            let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 100) * y_modifier;
            miro_frame
                .update_position(x_position, y_position)
                .await
                .change_context(CommandError)?;
        }
    }

    Ok(())
}

pub fn create_overhaul_file(entrypoint_name: String) -> Result<(), CommandError> {
    let code_overhaul_auditor_file_path = batbelt::path::get_file_path(
        BatFile::CodeOverhaulToReview {
            file_name: entrypoint_name.clone(),
        },
        false,
    )
    .change_context(CommandError)?;
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        return Err(Report::new(CommandError).attach_printable(format!(
            "code overhaul file already exists for: {entrypoint_name:?}"
        )));
    }
    let mut co_template =
        batbelt::templates::code_overhaul::CodeOverhaulTemplate::template_to_markdown_file(
            &code_overhaul_auditor_file_path,
        );
    co_template.save().change_context(CommandError)?;
    println!(
        "code-overhaul file created: {}{}",
        entrypoint_name.green(),
        ".md".green()
    );
    Ok(())
}
