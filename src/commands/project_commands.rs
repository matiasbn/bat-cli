use super::CommandError;
use std::env;

use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::{BatEnumerator, ShareableData};
use crate::config::{BatAuditorConfig, BatConfig};
use colored::Colorize;
use error_stack::{FutureExt, Report, ResultExt};
use error_stack::{IntoReport, Result};

use crate::batbelt;

use crate::batbelt::git::{GitAction, GitCommit};

use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::commands::{BatCommandEnumerator, CommandResult};
use clap::Subcommand;

use crate::commands::sonar_commands::SonarCommand;
use std::path::Path;
use std::process::Command;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ProjectCommands {
    #[default]
    New,
    Reload,
}
impl BatEnumerator for ProjectCommands {}

impl BatCommandEnumerator for ProjectCommands {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ProjectCommands::New => self.new_bat_project(),
            ProjectCommands::Reload => self.reload_bat_project(),
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
    fn reload_bat_project(&self) -> CommandResult<()> {
        let bat_auditor_toml_file = BatFile::BatAuditorToml;
        if !bat_auditor_toml_file
            .file_exists()
            .change_context(CommandError)?
        {
            BatAuditorConfig::new_with_prompt().change_context(CommandError)?;
        }
        let auditor_notes_bat_folder = BatFolder::AuditorNotes;
        if !auditor_notes_bat_folder
            .folder_exists()
            .change_context(CommandError)?
        {
            let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
            project_commands_functions::init_auditor_configuration(
                bat_auditor_config.auditor_name,
            )?;
        } else {
            GitAction::CheckoutAuditorBranch
                .execute_action()
                .change_context(CommandError)?;

            project_commands_functions::update_co_to_review()?;
            project_commands_functions::update_package_json()?;
            project_commands_functions::update_git_ignore()?;

            GitCommit::UpdateTemplates
                .create_commit()
                .change_context(CommandError)?;
        }

        println!("bat project {}", "reloaded!".bright_green());
        Ok(())
    }

    fn new_bat_project(&self) -> Result<(), CommandError> {
        let bat_config = BatConfig::new_with_prompt().change_context(CommandError)?;
        println!("Creating {:#?} project", bat_config);
        TemplateGenerator
            .create_new_project_folders()
            .change_context(CommandError)?;

        let bat_config: BatConfig = BatConfig::get_config().change_context(CommandError)?;

        let shared_initialized = ShareableData::new(false);

        GitAction::CheckGitIsInitialized {
            is_initialized: shared_initialized.original,
        }
        .execute_action()
        .change_context(CommandError)?;

        if !*shared_initialized.cloned.borrow() {
            println!("Initializing project repository");
            project_commands_functions::initialize_project_repository()?;
            println!("Project repository successfully initialized");
        }

        PackageJsonTemplate::create_package_json(None).change_context(CommandError)?;

        println!(
            "\n\nRunning Sonar to update {}\n\n",
            "BatMetadata.json!".bright_green()
        );

        SonarCommand::Run {
            skip_source_code: false,
            only_context_accounts: false,
            only_entry_points: false,
            only_traits: false,
            only_function_dependencies: false,
        }
        .execute_command()?;

        // create auditors branches from develop
        for auditor_name in bat_config.auditor_names {
            BatFile::BatAuditorToml
                .create_empty(false)
                .change_context(CommandError)?;
            let bat_auditor_config = BatAuditorConfig {
                auditor_name: auditor_name.clone(),
                miro_oauth_access_token: "".to_string(),
                use_code_editor: false,
                code_editor: Default::default(),
            };
            bat_auditor_config.save().change_context(CommandError)?;

            project_commands_functions::init_auditor_configuration(auditor_name.clone())?;

            BatFile::BatAuditorToml
                .remove_file()
                .change_context(CommandError)?;
        }

        BatAuditorConfig::new_with_prompt().change_context(CommandError)?;

        BatFile::ProgramLib
            .open_in_editor(false, None)
            .change_context(CommandError)?;

        println!("Project {} successfully created", bat_config.project_name);
        Ok(())
    }
}

mod project_commands_functions {
    use super::*;

    pub fn init_auditor_configuration(auditor_name: String) -> CommandResult<()> {
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
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
        } else {
            println!("Checking out {:?} branch", auditor_project_branch_name);
            // checkout auditor branch
            Command::new("git")
                .args(["checkout", auditor_project_branch_name.as_str()])
                .output()
                .unwrap();
        }
        TemplateGenerator
            .create_folders_for_current_auditor()
            .change_context(CommandError)?;
        initialize_code_overhaul_files()?;
        GitCommit::InitAuditor
            .create_commit()
            .change_context(CommandError)?;
        Ok(())
    }

    pub fn update_co_to_review() -> CommandResult<()> {
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

    pub fn update_package_json() -> CommandResult<()> {
        println!("Updating package.json");
        PackageJsonTemplate::create_package_json(None).change_context(CommandError)
    }

    pub fn update_git_ignore() -> CommandResult<()> {
        println!("Updating .gitignore");
        GitIgnore
            .write_content(true, &TemplateGenerator.get_git_ignore_content())
            .change_context(CommandError)
    }

    pub fn initialize_code_overhaul_files() -> Result<(), CommandError> {
        let entrypoints_names = EntrypointParser::get_entrypoint_names(false).unwrap();

        for entrypoint_name in entrypoints_names {
            create_overhaul_file(entrypoint_name.clone())?;
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

    pub fn initialize_project_repository() -> Result<(), CommandError> {
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
}
