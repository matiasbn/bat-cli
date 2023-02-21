use crate::batbelt;
use crate::batbelt::command_line::execute_command;

use crate::commands::CommandError;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};

use std::process::Command;

pub enum GitCommands {
    UpdateDevelop,
    MergeAllToDevelop,
    MergeDevelopToAll,
    FetchRemoteBranches { select_all: bool },
    DeleteLocalBranches { select_all: bool },
}

impl GitCommands {
    pub fn execute(&self) -> Result<(), CommandError> {
        self.check_develop_exists()?;
        match self {
            GitCommands::UpdateDevelop => {
                self.merge_all_to_develop()?;
                self.merge_develop_to_all()?;
            }
            GitCommands::FetchRemoteBranches { select_all } => {
                self.fetch_remote_branches(select_all.clone())?
            }
            GitCommands::DeleteLocalBranches { select_all } => {
                self.delete_local_branches(select_all.clone())?
            }
            GitCommands::MergeAllToDevelop => self.merge_all_to_develop()?,
            GitCommands::MergeDevelopToAll => self.merge_develop_to_all()?,
        }
        Ok(())
    }

    fn merge_all_to_develop(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        self.checkout_branch("develop")?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch '{}' into develop", branch_name);
            execute_command("git", &["merge", &branch_name, "-m", &message])
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
        let selections = batbelt::cli_inputs::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.clone().len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Fetching {}", selected_branch.green());
            log::debug!("selected_branch to fetch: {}", selected_branch);
            execute_command(
                "git",
                &["checkout", selected_branch.trim_start_matches("origin/")],
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
        let selections = batbelt::cli_inputs::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.clone().len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Deleting {}", selected_branch.green());
            log::debug!("selected_branch to delete: {}", selected_branch);
            execute_command("git", &["branch", "-D", selected_branch])
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn check_develop_exists(&self) -> Result<(), CommandError> {
        let branches_list = batbelt::git::get_local_branches().change_context(CommandError)?;
        if !branches_list
            .lines()
            .any(|line| line.trim_start_matches("*").trim_start() == "develop")
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
                let branch_name = branch.trim().trim_start_matches("*").trim();
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
                let branch_name = branch.trim().trim_start_matches("*").trim();
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
        execute_command("git", &["checkout", branch_name]).change_context(CommandError)?;
        Ok(())
    }
}

#[test]
fn test_get_remote_branches_filtered() {
    let remote_branches =
        GitCommands::get_remote_branches_filtered(&GitCommands::UpdateDevelop).unwrap();
    println!("remote_branches:\n{:#?}", remote_branches)
}

#[test]
fn test_get_local_branches_filtered() {
    let local_branches =
        GitCommands::get_local_branches_filtered(&GitCommands::UpdateDevelop).unwrap();
    println!("local_branches:\n{:#?}", local_branches)
}
