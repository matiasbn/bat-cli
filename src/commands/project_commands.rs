use super::CommandError;

use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::BatEnumerator;
use crate::config::{BatAuditorConfig, BatConfig};
use colored::Colorize;
use error_stack::Result;
use error_stack::{FutureExt, Report, ResultExt};

use crate::batbelt;
use crate::batbelt::bat_dialoguer;
use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::git::git_commit::GitCommit;

use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::commands::miro_commands::MiroCommand;
use crate::commands::{BatCommandEnumerator, CommandResult};
use clap::Subcommand;

use crate::batbelt::git::git_action::GitAction;
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
            ProjectCommands::New => {
                panic!("Use new_bat_project() directly for async Miro support")
            }
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
        } else {
            let mut bat_auditor_config =
                BatAuditorConfig::get_config().change_context(CommandError)?;
            bat_auditor_config
                .get_external_bat_metadata()
                .change_context(CommandError)?;
            bat_auditor_config.save().change_context(CommandError)?;
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

            GitCommit::BatReload
                .create_commit(true)
                .change_context(CommandError)?;
        }

        println!("bat project {}", "reloaded!".bright_green());
        Ok(())
    }

    pub async fn new_bat_project(&self) -> Result<(), CommandError> {
        let bat_config = BatConfig::new_with_prompt().change_context(CommandError)?;
        println!("Creating {:#?} project", bat_config);
        TemplateGenerator
            .create_new_project_folders()
            .change_context(CommandError)?;

        let bat_config: BatConfig = BatConfig::get_config().change_context(CommandError)?;

        // Create audit branch from current HEAD (repo already initialized)
        println!("Creating audit branch");
        project_commands_functions::initialize_audit_branch()?;

        PackageJsonTemplate::create_package_json(None).change_context(CommandError)?;

        println!(
            "\n\nRunning Sonar to update {}\n\n",
            "BatMetadata.json!".bright_green()
        );

        SonarCommand::Run.execute_command()?;

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
                external_bat_metadata: vec![],
            };
            bat_auditor_config.save().change_context(CommandError)?;

            project_commands_functions::init_auditor_configuration(auditor_name.clone())?;

            BatFile::BatAuditorToml
                .remove_file()
                .change_context(CommandError)?;
        }

        BatAuditorConfig::new_with_prompt().change_context(CommandError)?;

        // Miro integration — ask at the end of the flow
        let use_miro = BatDialoguer::select_yes_or_no("Do you want to use the Miro integration?".to_string())
            .change_context(CommandError)?;

        if use_miro {
            // Get board URL
            let miro_board_url_input: String =
                bat_dialoguer::input("Miro board url:").change_context(CommandError)?;
            let miro_board_url =
                BatConfig::normalize_miro_board_url(&miro_board_url_input).change_context(CommandError)?;

            // Update Bat.toml with the board URL
            let mut bat_config = BatConfig::get_config().change_context(CommandError)?;
            bat_config.miro_board_url = miro_board_url;
            bat_config.save().change_context(CommandError)?;

            // Get OAuth token
            let miro_oauth_token: String =
                bat_dialoguer::input("Miro OAuth access token:").change_context(CommandError)?;

            // Update BatAuditor.toml with the token
            let mut bat_auditor_config =
                BatAuditorConfig::get_config().change_context(CommandError)?;
            bat_auditor_config.miro_oauth_access_token = miro_oauth_token;
            bat_auditor_config.save().change_context(CommandError)?;

            // Deploy CO frames automatically
            MiroCommand::CodeOverhaulFrames
                .execute_command()
                .await?;
        }

        BatFile::ProgramLib
            .open_in_editor(false, None)
            .change_context(CommandError)?;

        println!("Project {} successfully created", bat_config.project_name);
        Ok(())
    }
}

mod project_commands_functions {
    use super::*;
    use lazy_regex::regex;

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
            .create_commit(true)
            .change_context(CommandError)?;
        Ok(())
    }

    pub fn update_co_to_review() -> CommandResult<()> {
        println!("Updating code overhaul files");
        let bat_config = BatConfig::get_config().change_context(CommandError)?;

        let program_names: Vec<Option<String>> = if bat_config.is_multi_program() {
            bat_config
                .get_program_names()
                .into_iter()
                .map(Some)
                .collect()
        } else {
            vec![None]
        };

        let mut updated_eps = vec![];

        for program_name in &program_names {
            let co_bat_folder = BatFolder::CodeOverhaulFolderPath {
                program_name: program_name.clone(),
            };
            let co_dir_file_name = co_bat_folder
                .get_all_bat_files(false, None, None)
                .change_context(CommandError)?;

            // get entry points for this program
            let entry_points_names = if let Some(pn) = program_name {
                let lib_path = bat_config.get_program_lib_path_by_name(pn).unwrap();
                EntrypointParser::get_entrypoint_names_filtered(true, Some(&lib_path))
                    .change_context(CommandError)?
            } else {
                EntrypointParser::get_entrypoint_names_from_program_lib(true)
                    .change_context(CommandError)?
            };

            let (_old_ep, deprecated_ep): (Vec<BatFile>, Vec<BatFile>) =
                co_dir_file_name.clone().into_iter().partition(|bat_file| {
                    entry_points_names.contains(
                        &bat_file
                            .get_file_name()
                            .unwrap()
                            .trim_end_matches(".md")
                            .to_string(),
                    )
                });

            let (_, new_ep): (Vec<String>, Vec<String>) =
                entry_points_names.clone().into_iter().partition(|ep_name| {
                    co_dir_file_name.clone().into_iter().any(|bat_file| {
                        bat_file.get_file_name().unwrap().trim_end_matches(".md") == ep_name
                    })
                });

            // create new ep files
            for ep_name in new_ep {
                println!(
                    "Creating code overhaul file for new entry point: {}{}",
                    ep_name.bright_blue(),
                    ".md".bright_blue()
                );
                let bat_file = BatFile::CodeOverhaulToReview {
                    file_name: ep_name,
                    program_name: program_name.clone(),
                };
                bat_file.create_empty(false).change_context(CommandError)?;
                updated_eps.push(bat_file.get_path(false).change_context(CommandError)?);
            }

            let deprecated_regex = regex!(r#"/code-overhaul/deprecated/"#);

            let filtered_dep = deprecated_ep
                .into_iter()
                .filter(|dep_bat_file| {
                    !deprecated_regex.is_match(&dep_bat_file.get_path(false).unwrap())
                })
                .collect::<Vec<_>>();

            // move deprecated to dep folder
            if !filtered_dep.is_empty() {
                let deprecated_co_bat_folder = BatFolder::CodeOverhaulDeprecated {
                    program_name: program_name.clone(),
                };
                if !deprecated_co_bat_folder
                    .folder_exists()
                    .change_context(CommandError)?
                {
                    deprecated_co_bat_folder
                        .create_folder()
                        .change_context(CommandError)?;
                }

                for ep_file in filtered_dep {
                    println!(
                        "Moving code overhaul file to deprecated folder: {}",
                        ep_file.get_path(false).unwrap().bright_blue()
                    );
                    let file_content = ep_file.read_content(false).change_context(CommandError)?;
                    let file_name = ep_file.get_file_name().change_context(CommandError)?;
                    let deprecated_file = BatFile::CodeOverhaulDeprecated {
                        file_name,
                        program_name: program_name.clone(),
                    };
                    deprecated_file
                        .write_content(false, &file_content)
                        .change_context(CommandError)?;
                    ep_file.remove_file().change_context(CommandError)?;

                    updated_eps.push(
                        deprecated_file
                            .get_path(false)
                            .change_context(CommandError)?,
                    );
                    updated_eps.push(ep_file.get_path(false).change_context(CommandError)?);
                }
            }
        }

        if !updated_eps.is_empty() {
            GitCommit::CodeOverhaulUpdated { updated_eps }
                .create_commit(true)
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
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
        if bat_config.is_multi_program() {
            for program_name in bat_config.get_program_names() {
                let lib_path = bat_config
                    .get_program_lib_path_by_name(&program_name)
                    .unwrap();
                let entrypoints_names =
                    EntrypointParser::get_entrypoint_names_filtered(false, Some(&lib_path))
                        .change_context(CommandError)?;
                for entrypoint_name in entrypoints_names {
                    create_overhaul_file(entrypoint_name, Some(program_name.clone()))?;
                }
            }
        } else {
            let entrypoints_names =
                EntrypointParser::get_entrypoint_names_from_program_lib(false).unwrap();
            for entrypoint_name in entrypoints_names {
                create_overhaul_file(entrypoint_name, None)?;
            }
        }
        Ok(())
    }

    pub fn create_overhaul_file(
        entrypoint_name: String,
        program_name: Option<String>,
    ) -> Result<(), CommandError> {
        let code_overhaul_file_path = BatFile::CodeOverhaulToReview {
            file_name: entrypoint_name.clone(),
            program_name: program_name.clone(),
        }
        .get_path(false)
        .change_context(CommandError)?;

        if Path::new(&code_overhaul_file_path).is_file() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "code overhaul file already exists for: {entrypoint_name:?}"
            )));
        }

        BatFile::CodeOverhaulToReview {
            file_name: entrypoint_name.clone(),
            program_name: program_name.clone(),
        }
        .write_content(false, "")
        .change_context(CommandError)?;

        println!(
            "code-overhaul file created: {}{}",
            entrypoint_name.green(),
            ".md".green()
        );

        Ok(())
    }

    pub fn initialize_audit_branch() -> Result<(), CommandError> {
        // We're inside the target repo, create a develop branch for audit work
        println!("Creating develop branch");
        GitAction::CreateBranch {
            branch_name: "develop".to_string(),
        }
        .execute_action()
        .change_context(CommandError)?;

        println!("Committing audit files");
        GitAction::AddAll
            .execute_action()
            .change_context(CommandError)?;
        GitCommit::Init
            .create_commit(true)
            .change_context(CommandError)?;

        Ok(())
    }
}
