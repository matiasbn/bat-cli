use super::CommandError;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::{bat_dialoguer, BatEnumerator, ShareableData};
use crate::config::{BatAuditorConfig, BatConfig};
use colored::Colorize;
use error_stack::Result;
use error_stack::{FutureExt, IntoReport, Report, ResultExt};

use crate::batbelt;
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::git::{GitAction, GitCommit};
use crate::batbelt::miro::frame::{
    MiroFrame, MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X,
    MIRO_INITIAL_Y,
};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::commands::{BatCommandEnumerator, CommandResult};
use clap::Subcommand;
use normalize_url::normalizer;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ProjectCommands {
    #[default]
    Create,
    Refresh,
}
impl BatEnumerator for ProjectCommands {}

impl BatCommandEnumerator for ProjectCommands {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ProjectCommands::Create => unimplemented!(),
            ProjectCommands::Refresh => self.refresh_bat_project(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }

    fn check_correct_branch(&self) -> bool {
        false
    }
}

impl ProjectCommands {
    pub fn execute_command(&self) -> Result<(), CommandError> {
        match self {
            ProjectCommands::Create => unimplemented!(),
            ProjectCommands::Refresh => self.refresh_bat_project(),
        }
    }

    fn refresh_bat_project(&self) -> CommandResult<()> {
        let bat_auditor_toml_file = BatFile::BatAuditorToml;
        if !bat_auditor_toml_file
            .file_exists()
            .change_context(CommandError)?
        {
            BatAuditorConfig::new_with_prompt().change_context(CommandError)?;
        }
        GitAction::CheckoutAuditorBranch
            .execute_action()
            .change_context(CommandError)?;

        self.update_co_to_review()?;
        self.update_package_json()?;
        self.update_git_ignore()?;

        GitCommit::UpdateTemplates
            .create_commit()
            .change_context(CommandError)?;

        println!("Templates successfully updated");
        Ok(())
    }

    fn update_co_to_review(&self) -> CommandResult<()> {
        println!("Updating to-review files in code-overhaul folder");
        let to_review_file_names = BatFolder::CodeOverhaulToReview
            .get_all_bat_files(false, None, None)
            .change_context(CommandError)?;
        // if the auditor to-review code overhaul folder exists
        for bat_file in to_review_file_names {
            bat_file.remove_file().change_context(CommandError)?;
            let file_path = bat_file.get_path(false).change_context(CommandError)?;
            let co_template = CodeOverhaulTemplate::new(
                &bat_file.get_file_name().change_context(CommandError)?,
                false,
            )
            .change_context(CommandError)?;
            let mut co_markdown = co_template
                .to_markdown_file(&file_path)
                .change_context(CommandError)?;
            co_markdown.save().change_context(CommandError)?;
        }
        Ok(())
    }

    fn update_package_json(&self) -> CommandResult<()> {
        println!("Updating package.json");
        PackageJsonTemplate::create_package_json(None).change_context(CommandError)
    }

    fn update_git_ignore(&self) -> CommandResult<()> {
        println!("Updating .gitignore");
        GitIgnore {
            to_create_project: false,
        }
        .write_content(true, &TemplateGenerator::get_git_ignore_content())
        .change_context(CommandError)
    }
}

pub fn create_bat_project() -> Result<(), CommandError> {
    // get project config
    let bat_config = create_bat_config_file().change_context(CommandError)?;
    println!("Creating {:#?} project", bat_config);
    TemplateGenerator::create_project().change_context(CommandError)?;
    execute_command("mv", &["Bat.toml", &bat_config.project_name], false)?;

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

            file_names.contains(&"Anchor.toml".to_string())
        })
        .map(|f| f.path().to_str().unwrap().to_string())
        .collect::<Vec<_>>();
    if local_anchor_project_folders.is_empty() {
        let message = "No Anchor projects were found on the current working directory".to_string();
        return Err(Report::new(CommandError).attach_printable(message));
    }
    // Folder with the program to audit selection
    let prompt_text = "Select the folder with the program to audit";
    let selection = bat_dialoguer::select(prompt_text, local_anchor_project_folders.clone(), None)
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
    let selection = bat_dialoguer::select(prompt_text, cargo_programs_paths.clone(), None)
        .change_context(CommandError)?;
    let selected_program_path = &cargo_programs_paths[selection];
    log::debug!("selected_program: {:#?}", selected_program_path);
    let program_name = selected_program_path
        .split('/')
        .last()
        .unwrap()
        .to_string()
        .replace('_', "-");
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
    let mut project_name: String = program_name.replace('_', "-") + "-audit";
    let prompt_text = format!(
        "Do you want to use the name {} for this project?",
        project_name.yellow()
    );

    let use_default = if !cfg!(debug_assertions) {
        bat_dialoguer::select_yes_or_no(prompt_text.as_str()).change_context(CommandError)?
    } else {
        true
    };

    if !use_default {
        project_name = bat_dialoguer::input("Project name:").change_context(CommandError)?;
    }
    let project_path = format!("./{project_name}");

    if Path::new(&project_path).is_dir() {
        return Err(Report::new(CommandError)
            .attach_printable(format!("Folder {} already exists", project_name)));
    }

    let auditor_names_prompt: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Auditor names (comma separated, example: alice,bob):")
            .change_context(CommandError)?
    } else {
        "test_user".to_string()
    };
    let auditor_names: Vec<String> = auditor_names_prompt
        .split(',')
        .map(|l| l.to_string())
        .collect();

    let client_name: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Client name:").change_context(CommandError)?
    } else {
        "test_client".to_string()
    };

    let commit_hash_url: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Commit hash url:").change_context(CommandError)?
    } else {
        "test_hash".to_string()
    };

    let starting_date: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Starting date, example: (01/01/2023):")
            .change_context(CommandError)?
    } else {
        "test_date".to_string()
    };

    let mut miro_board_url: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Miro board url:").change_context(CommandError)?
    } else {
        "https://miro.com/app/board/uXjVPzsgmiY=/".to_string()
    };

    miro_board_url = normalize_miro_board_url(&miro_board_url)?;

    let project_repository_url: String = if !cfg!(debug_assertions) {
        bat_dialoguer::input("Project repo url, where this audit folder would be pushed:")
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

pub async fn initialize_bat_project(skip_initial_commit: bool) -> Result<(), CommandError> {
    let bat_config: BatConfig = BatConfig::get_config().change_context(CommandError)?;
    if !Path::new("BatAuditor.toml").is_file() {
        BatAuditorConfig::new_with_prompt().change_context(CommandError)?;
    }
    println!("creating project for the next config: ");
    println!("{:#?}", bat_config);

    let shared_initialized = ShareableData::new(false);

    GitAction::CheckGitIsInitialized {
        is_initialized: shared_initialized.original,
    }
    .execute_action()
    .change_context(CommandError)?;

    // delete before commit
    BatFile::PackageJson {
        to_create_project: false,
    }
    .remove_file()
    .change_context(CommandError)?;

    if !*shared_initialized.cloned.borrow() {
        println!("Initializing project repository");
        initialize_project_repository()?;
        println!("Project repository successfully initialized");
    }

    // create with proper scripts
    PackageJsonTemplate::create_package_json(None).change_context(CommandError)?;

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
        batbelt::git::get_auditor_branch_name().change_context(CommandError)?;
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

    BatFile::ProgramLib
        .open_in_editor(false, None)
        .change_context(CommandError)?;
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

fn initialize_code_overhaul_files() -> Result<(), CommandError> {
    let entrypoints_names = EntrypointParser::get_entrypoints_names(false).unwrap();

    for entrypoint_name in entrypoints_names {
        create_overhaul_file(entrypoint_name.clone())?;
    }
    Ok(())
}

async fn create_miro_frames_for_entrypoints() -> Result<(), CommandError> {
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
                MiroFrame::new(entrypoint_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
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

fn create_overhaul_file(entrypoint_name: String) -> Result<(), CommandError> {
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
