use std::cell::{RefCell, RefMut};
use std::fs::{self};

use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::str::from_utf8;
use std::string::String;

use colored::Colorize;

use crate::batbelt;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::miro::frame::{
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::commands::CommandError;
use crate::config::{BatAuditorConfig, BatConfig};

use crate::batbelt::git::{GitAction, GitCommit};
use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::path::{BatFile, BatFolder};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::ShareableData;
use error_stack::{Report, Result, ResultExt};

pub async fn initialize_bat_project(skip_initial_commit: bool) -> Result<(), CommandError> {
    let bat_config: BatConfig = BatConfig::get_config().change_context(CommandError)?;
    if !Path::new("BatAuditor.toml").is_file() {
        prompt_auditor_options()?;
    }
    println!("creating project for the next config: ");
    println!("{:#?}", bat_config);

    let shared_initialized = ShareableData::new(false);

    GitAction::CheckGitIsInitialized {
        is_initialized: shared_initialized.original,
    }
    .execute_action()
    .change_context(CommandError)?;

    if !*shared_initialized.cloned.borrow() {
        println!("Initializing project repository");
        initialize_project_repository()?;
        println!("Project repository successfully initialized");
    }

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

    let auditor_notes_bat_folder = BatFolder::AuditorNotes;
    if auditor_notes_bat_folder
        .folder_exists()
        .change_context(CommandError)?
    {
        let auditor_notes_files = auditor_notes_bat_folder
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;
        if auditor_notes_files.is_empty() {
            TemplateGenerator::create_auditor_folders().change_context(CommandError)?;

            // create overhaul files
            initialize_code_overhaul_files()?;
            // commit to-review files
            create_miro_frames_for_entrypoints().await?;
        }
    } else {
        TemplateGenerator::create_auditor_folders().change_context(CommandError)?;

        // create overhaul files
        initialize_code_overhaul_files()?;
        // commit to-review files
        create_miro_frames_for_entrypoints().await?;
    }

    println!("Project successfully initialized");
    if !skip_initial_commit {
        GitCommit::InitAuditor
            .create_commit()
            .change_context(CommandError)?;
    }
    // let lib_file_path = utils::path::get_program_lib_path()?;
    let lib_file_path = BatFile::ProgramLib
        .get_path(true)
        .change_context(CommandError)?;
    // Open lib.rs file in vscode
    vs_code_open_file_in_current_window(PathBuf::from(lib_file_path).to_str().unwrap())
        .change_context(CommandError)?;
    Ok(())
}

fn prompt_auditor_options() -> Result<(), CommandError> {
    let bat_config = BatConfig::get_config().change_context(CommandError)?;
    let auditor_names = bat_config.auditor_names;
    let prompt_text = format!("Select your name:");
    let selection = BatDialoguer::select(prompt_text, auditor_names.clone(), None)?;
    let auditor_name = auditor_names.get(selection).unwrap().clone();
    println!(
        "Is great to have you here {}!",
        format!("{}", auditor_name).green()
    );
    let prompt_text = "Do you want to use the Miro integration?";
    let include_miro =
        BatDialoguer::select_yes_or_no(prompt_text.to_string()).change_context(CommandError)?;
    let moat = if include_miro {
        let prompt_text = "Miro OAuth access token";
        BatDialoguer::input(prompt_text.to_string()).change_context(CommandError)?
    } else {
        "".to_string()
    };
    let prompt_text = "Do you want to use the VS Code integration?";
    let include_vs_code =
        BatDialoguer::select_yes_or_no(prompt_text.to_string()).change_context(CommandError)?;
    let bat_auditor_config = BatAuditorConfig {
        auditor_name: auditor_name.to_string(),
        vs_code_integration: include_vs_code,
        miro_oauth_access_token: moat,
    };
    bat_auditor_config.save().change_context(CommandError)?;
    Ok(())
}

fn initialize_project_repository() -> Result<(), CommandError> {
    // git init
    GitAction::Init
        .execute_action()
        .change_context(CommandError)?;

    println!("Adding project repository as remote");
    GitAction::RemoteAddProjectRepo
        .execute_action()
        .change_context(CommandError)?;

    println!("Commit all to main");
    GitAction::AddAll
        .execute_action()
        .change_context(CommandError)?;
    GitCommit::Init
        .create_commit()
        .change_context(CommandError)?;

    println!("Creating develop branch");
    GitAction::CreateBranch {
        branch_name: "develop".to_string(),
    }
    .execute_action()
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
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    if bat_auditor_config.miro_oauth_access_token.is_empty() {
        return Ok(());
    }

    let user_want_to_deploy = BatDialoguer::select_yes_or_no(
        "Do you want to deploy the Miro frames for code overhaul?".to_string(),
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
    let code_overhaul_file_path = BatFile::CodeOverhaulToReview {
        file_name: entrypoint_name.clone(),
    }
    .get_path(false)
    .change_context(CommandError)?;

    if Path::new(&code_overhaul_file_path).is_file() {
        return Err(Report::new(CommandError).attach_printable(format!(
            "code overhaul file already exists for: {entrypoint_name:?}"
        )));
    }
    let co_template =
        CodeOverhaulTemplate::new(&entrypoint_name, false).change_context(CommandError)?;
    let mut co_markdown = co_template
        .to_markdown_file(&code_overhaul_file_path)
        .change_context(CommandError)?;

    co_markdown.save().change_context(CommandError)?;

    println!(
        "code-overhaul file created: {}{}",
        entrypoint_name.green(),
        ".md".green()
    );

    Ok(())
}
