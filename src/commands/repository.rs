use crate::batbelt;
use crate::batbelt::command_line::execute_command;

use crate::batbelt::git::GitCommit;
use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::path::BatFolder;
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::batbelt::templates::TemplateGenerator;
use crate::commands::{CommandError, CommandResult};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use std::process::Command;

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq)]
pub enum RepositoryCommand {
    /// Merges all the branches into develop branch, and then merge develop into the rest of the branches
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
    /// Updates the templates to the last version
    UpdateTemplates,
}

impl RepositoryCommand {
    pub fn execute_command(&self) -> Result<(), CommandError> {
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
                .create_commit()
                .change_context(CommandError),
            RepositoryCommand::UpdateTemplates => self.update_templates(),
        }
    }

    fn merge_all_to_develop(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        self.checkout_branch("develop")?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch '{}' into develop", branch_name);
            execute_command("git", &["merge", &branch_name, "-m", &message], false)
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn merge_develop_to_all(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
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
        self.checkout_branch("develop")?;
        Ok(())
    }

    fn fetch_remote_branches(&self, select_all: bool) -> Result<(), CommandError> {
        let branches_list = self.get_remote_branches_filtered()?;
        let prompt_test = format!("Select the branches {}", "to fetch".green());
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
        self.checkout_branch("develop")?;
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

    fn checkout_branch(&self, branch_name: &str) -> Result<(), CommandError> {
        execute_command("git", &["checkout", branch_name], false).change_context(CommandError)?;
        Ok(())
    }

    fn update_templates(&self) -> CommandResult<()> {
        println!("Updating to-review files in code-overhaul folder");
        // move new templates to to-review in the auditor notes folder
        // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
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

        // replace package.json
        println!("Updating package.json");
        PackageJsonTemplate::update_package_json().change_context(CommandError)?;
        GitIgnore { for_init: false }
            .write_content(true, &TemplateGenerator::get_git_ignore_content())
            .change_context(CommandError)?;
        GitCommit::UpdateTemplates
            .create_commit()
            .change_context(CommandError)?;
        println!("Templates successfully updated");
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
