use std::fs::{self};

use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::from_utf8;
use std::string::String;

use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use super::create::AUDITOR_TOML_INITIAL_PATH;
use super::entrypoints::entrypoints::get_entrypoints_names;
use crate::batbelt;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::constants::{
    AUDITOR_TOML_INITIAL_CONFIG_STR, AUDIT_INFORMATION_CLIENT_NAME_PLACEHOLDER,
    AUDIT_INFORMATION_COMMIT_HASH_PLACEHOLDER, AUDIT_INFORMATION_MIRO_BOARD_PLACEHOLER,
    AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER, AUDIT_INFORMATION_STARTING_DATE_PLACEHOLDER,
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::config::{BatConfig, RequiredConfig};

use crate::batbelt::git::GitCommit;
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::path::{FilePathType, FolderPathType};

pub async fn initialize_bat_project() -> Result<(), String> {
    let bat_config: BatConfig = BatConfig::get_init_config()?;
    let BatConfig {
        required, auditor, ..
    } = bat_config.clone();
    if !Path::new("BatAuditor.toml").is_file() || auditor.auditor_name.is_empty() {
        prompt_auditor_options(required.clone())?;
    }
    let bat_config: BatConfig = BatConfig::get_validated_config()?;
    // if !Path::new(&bat_auditor_path).is_dir() {}
    // if auditor.auditor is empty, prompt name
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

    let readme_path = batbelt::path::get_file_path(FilePathType::Readme, true);
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
    for auditor_name in required.auditor_names {
        let auditor_project_branch_name = format!("{}-{}", auditor_name, required.project_name);
        let auditor_project_branch_exists =
            batbelt::git::check_if_branch_exists(&auditor_project_branch_name)?;
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
    let auditor_project_branch_name = batbelt::git::get_expected_current_branch()?;
    println!("Checking out {:?} branch", auditor_project_branch_name);
    // checkout auditor branch
    Command::new("git")
        .args(["checkout", auditor_project_branch_name.as_str()])
        .output()
        .unwrap();

    // validate_init_config()?;
    // let auditor_notes_folder = utils::path::get_auditor_notes_path()?;
    let auditor_notes_folder = batbelt::path::get_folder_path(FolderPathType::AuditorNotes, false);
    let auditor_notes_folder_exists = Path::new(&auditor_notes_folder).is_dir();
    if auditor_notes_folder_exists {
        let auditor_notes_files =
            batbelt::helpers::get::get_only_files_from_folder(auditor_notes_folder.clone())?;
        if auditor_notes_files.is_empty() {
            create_auditor_notes_folder()?;
            // create overhaul files
            initialize_code_overhaul_files().await?;
            // commit to-review files
        }
    } else {
        create_auditor_notes_folder()?;
        // create overhaul files
        initialize_code_overhaul_files().await?;
        // commit to-review files
    }

    println!("Project successfully initialized");
    batbelt::git::create_git_commit(GitCommit::InitAuditor, None)?;
    // let lib_file_path = utils::path::get_program_lib_path()?;
    let lib_file_path = batbelt::path::get_file_path(FilePathType::ProgramLib, true);
    // Open lib.rs file in vscode
    vs_code_open_file_in_current_window(PathBuf::from(lib_file_path).to_str().unwrap())?;
    Ok(())
}

fn prompt_auditor_options(required: RequiredConfig) -> Result<(), String> {
    let auditor_name = get_auditor_name(required.auditor_names);
    println!(
        "Is great to have you here {}!",
        format!("{}", auditor_name).green()
    );
    let prompt_text = "Do you want to use the Miro integration?";
    let include_miro = batbelt::cli_inputs::select_yes_or_no(prompt_text)?;
    let token: String;
    let moat: Option<&str> = if include_miro {
        let prompt_text = "Miro OAuth access token";
        token = batbelt::cli_inputs::input(&prompt_text)?;
        Some(token.as_str())
    } else {
        None
    };
    let prompt_text = "Do you want to use the VS Code integration?";
    let include_vs_code = batbelt::cli_inputs::select_yes_or_no(prompt_text)?;
    update_auditor_toml(auditor_name, moat, include_vs_code);
    Ok(())
}

fn update_readme_file() -> Result<(), String> {
    let RequiredConfig {
        project_name,
        client_name,
        commit_hash_url,
        starting_date,
        miro_board_url,
        ..
    } = BatConfig::get_init_config()?.required;
    let readme_path = batbelt::path::get_file_path(FilePathType::Readme, true);
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

fn get_auditor_name(auditor_names: Vec<String>) -> String {
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your name")
        .default(0)
        .items(&auditor_names[..])
        .interact()
        .unwrap();

    auditor_names[selection].clone()
}

fn update_auditor_toml(
    auditor_name: String,
    miro_oauth_access_token: Option<&str>,
    vs_code_integration: bool,
) {
    let mut new_auditor_file_content = AUDITOR_TOML_INITIAL_CONFIG_STR.replace(
        "auditor_name = \"",
        ("auditor_name = \"".to_string() + &auditor_name).as_str(),
    );
    if let Some(moat) = miro_oauth_access_token {
        new_auditor_file_content = new_auditor_file_content.replace(
            "miro_oauth_access_token = \"",
            ("miro_oauth_access_token = \"".to_string() + moat).as_str(),
        )
    }
    if vs_code_integration {
        new_auditor_file_content = new_auditor_file_content
            .replace("vs_code_integration = false", "vs_code_integration = true")
    }
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);
    fs::write(auditor_toml_path, new_auditor_file_content).expect("Could not write to file!");
}

fn initialize_project_repository() -> Result<(), String> {
    let BatConfig { required, .. } = BatConfig::get_validated_config()?;
    let RequiredConfig {
        project_repository_url,
        ..
    } = required;
    // git init
    let mut output = Command::new("git").args(["init"]).output().unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Adding project repository as remote");
    // git remote add origin project_repository
    output = Command::new("git")
        .args(["remote", "add", "origin", project_repository_url.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Commit all to main");
    output = Command::new("git").args(["add", "-A"]).output().unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    output = Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Creating develop branch");
    // create develop
    Command::new("git")
        .args(["checkout", "-b", "develop"])
        .output()
        .unwrap();

    Ok(())
}

fn create_auditor_notes_folder() -> Result<(), String> {
    let bat_config = BatConfig::get_validated_config()?;
    println!(
        "creating auditor notes folder for {}",
        bat_config.auditor.auditor_name.red()
    );

    let output = Command::new("cp")
        .args([
            "-r",
            batbelt::path::get_folder_path(FolderPathType::NotesTemplate, false).as_str(),
            batbelt::path::get_folder_path(FolderPathType::AuditorNotes, false).as_str(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    Ok(())
}

pub async fn initialize_code_overhaul_files() -> Result<(), String> {
    let entrypoints_names = get_entrypoints_names()?;

    for (entrypoint_index, entrypoint_name) in entrypoints_names.iter().enumerate() {
        create_overhaul_file(entrypoint_name.clone())?;
        // create miro boards for entrypoints

        println!("Creating frame in Miro for {entrypoint_name}");
        let miro_config = MiroConfig::new();
        if miro_config.miro_enabled() {
            let mut miro_frame =
                MiroFrame::new(&entrypoint_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
            miro_frame.deploy().await?;
            let x_modifier = entrypoint_index as i64 % MIRO_BOARD_COLUMNS;
            let y_modifier = entrypoint_index as i64 / MIRO_BOARD_COLUMNS;
            let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 100) * x_modifier;
            let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 100) * y_modifier;
            miro_frame.update_position(x_position, y_position).await?;
        }
    }

    Ok(())
}

pub fn create_overhaul_file(entrypoint_name: String) -> Result<(), String> {
    let code_overhaul_auditor_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulToReview {
            file_name: entrypoint_name.clone(),
        },
        false,
    );
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        panic!("code overhaul file already exists for: {entrypoint_name:?}");
    }
    let mut co_template =
        batbelt::templates::code_overhaul::CodeOverhaulFile::template_to_markdown_file(
            &code_overhaul_auditor_file_path,
        );
    co_template.save()?;
    println!("code-overhaul file created: {entrypoint_name}.md");
    Ok(())
}
