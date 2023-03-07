use super::CommandError;

use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::{BatEnumerator, ShareableData};
use crate::config::{BatAuditorConfig, BatConfig};
use colored::Colorize;
use error_stack::Result;
use error_stack::{FutureExt, Report, ResultExt};

use crate::batbelt;

use crate::batbelt::git::{GitAction, GitCommit};

use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::commands::{BatCommandEnumerator, CommandResult};
use clap::Subcommand;

use std::path::Path;
use std::process::Command;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ProjectCommands {
    #[default]
    Create,
    Reload,
}
impl BatEnumerator for ProjectCommands {}

impl BatCommandEnumerator for ProjectCommands {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ProjectCommands::Create => unimplemented!(),
            ProjectCommands::Reload => self.refresh_bat_project(),
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
            ProjectCommands::Reload => self.refresh_bat_project(),
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
            let co_template = CodeOverhaulTemplate::new(
                &bat_file.get_file_name().change_context(CommandError)?,
                false,
            )
            .change_context(CommandError)?;
            let co_markdown_content = co_template
                .get_markdown_content()
                .change_context(CommandError)?;
            bat_file
                .write_content(false, &co_markdown_content)
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn update_package_json(&self) -> CommandResult<()> {
        println!("Updating package.json");
        PackageJsonTemplate::create_package_json(None).change_context(CommandError)
    }

    fn update_git_ignore(&self) -> CommandResult<()> {
        println!("Updating .gitignore");
        GitIgnore
            .write_content(true, &TemplateGenerator::get_git_ignore_content())
            .change_context(CommandError)
    }
}

pub fn create_bat_project() -> Result<(), CommandError> {
    // get project config
    let bat_config = BatConfig::new_with_prompt().change_context(CommandError)?;
    println!("Creating {:#?} project", bat_config);
    TemplateGenerator::create_project().change_context(CommandError)?;
    println!("Project {} successfully created", bat_config.project_name);
    Ok(())
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
    BatFile::PackageJson
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
        }
    } else {
        TemplateGenerator::create_auditor_folders().change_context(CommandError)?;

        // create overhaul files
        initialize_code_overhaul_files()?;
        // commit to-review files
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
    let entrypoints_names = EntrypointParser::get_entrypoint_names(false).unwrap();

    for entrypoint_name in entrypoints_names {
        create_overhaul_file(entrypoint_name.clone())?;
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
    let co_markdown_content = co_template
        .get_markdown_content()
        .change_context(CommandError)?;

    BatFile::CodeOverhaulToReview {
        file_name: entrypoint_name.clone(),
    }
    .write_content(false, &co_markdown_content)
    .change_context(CommandError)?;

    println!(
        "code-overhaul file created: {}{}",
        entrypoint_name.green(),
        ".md".green()
    );

    Ok(())
}
