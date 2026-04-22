use crate::batbelt;
use crate::batbelt::command_line::execute_command;
use crate::config::BatConfig;

use crate::batbelt::git::{get_current_branch_name, get_not_committed_files};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::git::git_commit::GitCommit;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use std::process::Command;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum RepositoryCommand {
    /// Merges all the branches into develop branch, and then merge develop into the rest of the branches
    #[default]
    UpdateBranches,
    /// Delete local branches
    DeleteLocalBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Fetch remote branches
    FetchRemoteBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Commits the open_questions, finding_candidate and threat_modeling notes
    CommitNotes,
    /// Creates a commit for an updated code-overhaul file
    CommitCodeOverhaulFile,
    /// Creates a commit for the code_overhaul_summary.md file
    CommitCodeOverhaulSummary,
    /// Creates a commit for the code_overhaul_summary.md file
    CommitProgramAccountsMetadata,
}

impl BatEnumerator for RepositoryCommand {}

impl BatCommandEnumerator for RepositoryCommand {
    fn execute_command(&self) -> CommandResult<()> {
        self.check_develop_exists()?;
        match self {
            RepositoryCommand::UpdateBranches => {
                self.merge_all_to_develop()?;
                self.merge_develop_to_all()
            }
            RepositoryCommand::FetchRemoteBranches { select_all } => {
                self.fetch_remote_branches(*select_all)
            }
            RepositoryCommand::DeleteLocalBranches { select_all } => {
                self.delete_local_branches(*select_all)
            }
            RepositoryCommand::CommitNotes => GitCommit::Notes
                .create_commit(true)
                .change_context(CommandError),
            RepositoryCommand::CommitProgramAccountsMetadata => {
                GitCommit::ProgramAccountMetadataUpdated
                    .create_commit(true)
                    .change_context(CommandError)
            }
            RepositoryCommand::CommitCodeOverhaulFile => self.execute_update_co_file(),
            RepositoryCommand::CommitCodeOverhaulSummary => self.update_code_overhaul_summary(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }

    fn check_correct_branch(&self) -> bool {
        match self {
            RepositoryCommand::CommitNotes => true,
            _ => false,
        }
    }
}

impl RepositoryCommand {
    fn update_code_overhaul_summary(&self) -> CommandResult<()> {
        GitCommit::UpdateCOSummary
            .create_commit(true)
            .change_context(CommandError)?;
        println!("Commit created for code_overhaul_summary.md file");
        Ok(())
    }

    fn merge_all_to_develop(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        let current_branch = get_current_branch_name().change_context(CommandError)?;
        self.checkout_branch("develop")?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch '{}' into develop", branch_name);
            execute_command("git", &["merge", &branch_name, "-m", &message], false)
                .change_context(CommandError)?;
        }
        self.checkout_branch(&current_branch)?;
        Ok(())
    }

    fn merge_develop_to_all(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        let current_branch = get_current_branch_name().change_context(CommandError)?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch develop into '{}'", branch_name);
            log::debug!("Merge message: {}", message);
            self.checkout_branch(&branch_name)?;
            println!("Merging develop into {}", branch_name.green());
            Command::new("git")
                .args(["merge", "develop", "-m", &message])
                .output()
                .into_report()
                .change_context(CommandError)?;
            // execute_command("git", &["merge", "develop", "-m", &message])
            //     .change_context(CommandError)?;
        }
        self.checkout_branch(&current_branch)?;
        Ok(())
    }

    fn fetch_remote_branches(&self, select_all: bool) -> Result<(), CommandError> {
        let branches_list = self.get_remote_branches_filtered()?;
        let prompt_test = format!("Select the branches {}", "to fetch".green());
        let current_branch = get_current_branch_name().change_context(CommandError)?;
        let selections = batbelt::bat_dialoguer::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Fetching {}", selected_branch.green());
            log::debug!("selected_branch to fetch: {}", selected_branch);
            execute_command(
                "git",
                &["checkout", selected_branch.trim_start_matches("origin/")],
                false,
            )
            .change_context(CommandError)?;
        }
        self.checkout_branch(&current_branch)?;
        Ok(())
    }

    fn delete_local_branches(&self, select_all: bool) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        self.checkout_branch("develop")?;
        let prompt_test = format!("Select the branches {}", "to delete".red());
        let selections = batbelt::bat_dialoguer::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Deleting {}", selected_branch.green());
            log::debug!("selected_branch to delete: {}", selected_branch);
            execute_command("git", &["branch", "-D", selected_branch], false)
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn check_develop_exists(&self) -> Result<(), CommandError> {
        let branches_list = batbelt::git::get_local_branches().change_context(CommandError)?;
        if !branches_list
            .lines()
            .any(|line| line.trim_start_matches('*').trim_start() == "develop")
        {
            log::debug!("branches_list:\n{}", branches_list);
            return Err(Report::new(CommandError).attach_printable("develop branch not found"));
        }
        Ok(())
    }

    fn get_local_branches_filtered(&self) -> Result<Vec<String>, CommandError> {
        let branches_list = batbelt::git::get_local_branches().change_context(CommandError)?;
        log::debug!("local_branches from batbelt::git: \n{}", branches_list);
        let list = branches_list
            .lines()
            .filter_map(|branch| {
                let branch_name = branch.trim().trim_start_matches('*').trim();
                if branch_name != "main" && branch_name != "develop" {
                    Some(branch_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        log::debug!("filtered branches_list: \n{:#?}", list);
        Ok(list)
    }

    fn get_remote_branches_filtered(&self) -> Result<Vec<String>, CommandError> {
        let branches_list = batbelt::git::get_remote_branches().change_context(CommandError)?;
        log::debug!("remote_branches from batbelt::git: \n{}", branches_list);
        let list = branches_list
            .lines()
            .filter_map(|branch| {
                let branch_name = branch.trim().trim_start_matches('*').trim();
                if branch_name != "origin/main"
                    && branch_name != "origin/develop"
                    && branch_name.split(" ->").next().unwrap() != "origin/HEAD"
                {
                    Some(branch_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        log::debug!("filtered remote_branches: \n{:#?}", list);
        Ok(list)
    }

    fn execute_update_co_file(&self) -> CommandResult<()> {
        println!("Select the code-overhaul file to finish:");
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
        let selected_program_name = if bat_config.is_multi_program() {
            Some(
                bat_config
                    .prompt_select_program()
                    .change_context(CommandError)?,
            )
        } else {
            None
        };
        let co_finished_folder = BatFolder::CodeOverhaulFinished {
            program_name: selected_program_name.clone(),
        };

        let finished_files_names = BatFolder::CodeOverhaulFinished {
            program_name: selected_program_name.clone(),
        }
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;

        if finished_files_names.is_empty() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "{}",
                "no finished files in code-overhaul folder".red()
            )));
        }

        let not_committed_files = get_not_committed_files().change_context(CommandError)?;

        let co_finished_path = co_finished_folder
            .get_path(false)
            .change_context(CommandError)?;

        let not_committed_finished = not_committed_files
            .into_iter()
            .filter_map(|file_path| {
                if file_path.contains(&co_finished_path) {
                    Some(BatFile::Generic { file_path }.get_file_name())
                } else {
                    None
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .change_context(CommandError)?;

        if not_committed_finished.is_empty() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "{}",
                "All finished co files are up to date".green()
            )));
        }

        let selection = BatDialoguer::select(
            "Select the code-overhaul file to update:".to_string(),
            not_committed_finished.clone(),
            None,
        )
        .change_context(CommandError)?;

        let finished_file_name = not_committed_finished[selection].clone();

        GitCommit::UpdateCO {
            entrypoint_name: finished_file_name,
            program_name: selected_program_name,
        }
        .create_commit(true)
        .change_context(CommandError)?;
        Ok(())
    }

    fn checkout_branch(&self, branch_name: &str) -> Result<(), CommandError> {
        execute_command("git", &["checkout", branch_name], false).change_context(CommandError)?;
        Ok(())
    }
}

#[test]
fn test_get_remote_branches_filtered() {
    let remote_branches =
        RepositoryCommand::get_remote_branches_filtered(&RepositoryCommand::UpdateBranches)
            .unwrap();
    println!("remote_branches:\n{:#?}", remote_branches)
}

#[test]
fn test_get_local_branches_filtered() {
    let local_branches =
        RepositoryCommand::get_local_branches_filtered(&RepositoryCommand::UpdateBranches).unwrap();
    println!("local_branches:\n{:#?}", local_branches)
}
