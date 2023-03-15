use crate::batbelt::command_line::{execute_command, execute_command_with_child_process};
use crate::batbelt::git;
use crate::batbelt::git::{GitError, GitResult};
use crate::config::BatConfig;
use crate::Suggestion;
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use regex::Regex;
use std::cell::RefCell;
use std::rc::Rc;
use std::str;

#[derive(Debug, PartialEq, strum_macros::Display)]
pub enum GitAction {
    CreateBranch {
        branch_name: String,
    },
    CheckoutAuditorBranch,
    Init,
    RemoteAddProjectRepo,
    AddAll,
    CheckGitIsInitialized {
        is_initialized: Rc<RefCell<bool>>,
    },
    CheckBranchDontExist {
        branch_name: String,
    },
    CheckCorrectBranch,
    GetRepositoryPermalink {
        file_path: String,
        start_line_index: usize,
        permalink: Rc<RefCell<String>>,
    },
    GetLastCommitMessage {
        last_commit_message: Rc<RefCell<String>>,
    },
}

impl GitAction {
    pub fn execute_action(&self) -> GitResult<()> {
        match self {
            GitAction::Init => {
                execute_command("git", &["init"], false).change_context(GitError)?;
            }
            GitAction::RemoteAddProjectRepo => {
                let bat_config = BatConfig::get_config().change_context(GitError)?;
                execute_command(
                    "git",
                    &[
                        "remote",
                        "add",
                        "origin",
                        &bat_config.project_repository_url,
                    ],
                    false,
                )
                .change_context(GitError)?;
            }
            GitAction::CreateBranch { branch_name } => {
                execute_command("git", &["checkout", "-b", branch_name], false)
                    .change_context(GitError)?;
            }
            GitAction::AddAll => {
                execute_command("git", &["add", "-A"], false).change_context(GitError)?;
            }
            GitAction::CheckGitIsInitialized { is_initialized } => {
                let output_child =
                    execute_command("git", &["rev-parse", "--is-inside-work-tree"], false)
                        .change_context(GitError)
                        .attach_printable(
                            "Error checking if the project is already on a git project",
                        )?;

                log::debug!("output {} {}", self.to_string(), output_child);

                let is_initialized_result = output_child == "true\n";

                log::debug!(
                    "is_initialized {} {}",
                    self.to_string(),
                    is_initialized_result
                );
                *is_initialized.borrow_mut() = is_initialized_result;
            }
            GitAction::CheckoutAuditorBranch => {
                let auditor_branch_name = git::get_auditor_branch_name()?;
                if git::get_current_branch_name()? != auditor_branch_name {
                    git_action_functions::checkout_branch(&auditor_branch_name)?
                }
                return Ok(());
            }
            GitAction::GetRepositoryPermalink {
                file_path,
                start_line_index,
                permalink,
            } => {
                let github_compatible_commit_hash_url_regex =
                    Regex::new(r#"https://github.com/[\w-]+/[\w-]+/commit/\w{40}"#)
                        .into_report()
                        .change_context(GitError)?;
                let commit_hash_url = BatConfig::get_config()
                    .change_context(GitError)?
                    .commit_hash_url;
                if github_compatible_commit_hash_url_regex.is_match(&commit_hash_url) {
                    let commit_hash_regex = Regex::new(r#"\w{40}"#)
                        .into_report()
                        .change_context(GitError)?;
                    let commit_hash = commit_hash_regex
                        .find(&commit_hash_url)
                        .ok_or(GitError)
                        .into_report()?
                        .as_str()
                        .to_string();
                    let github_url_prefix_regex = Regex::new(r#"https://github.com/[\w-]+/[\w-]+"#)
                        .into_report()
                        .change_context(GitError)?;
                    let github_url_prefix = github_url_prefix_regex
                        .find(&commit_hash_url)
                        .ok_or(GitError)
                        .into_report()?
                        .as_str()
                        .to_string();
                    let mut program_path_formatted = file_path.trim_start_matches("../").split("/");
                    program_path_formatted
                        .next()
                        .ok_or(GitError)
                        .into_report()?;
                    let program_path = program_path_formatted.collect::<Vec<_>>().join("/");
                    let permalink_result = format!(
                        "{}/blob/{}/{}#L{}",
                        github_url_prefix, commit_hash, program_path, start_line_index
                    );
                    *permalink.borrow_mut() = permalink_result.clone();
                } else {
                    println!("Commit hash url format is not compatible, got {}, expected https://github.com/github_handle/repository_name/commit/commit_hash", commit_hash_url.red());
                    *permalink.borrow_mut() = "".to_string();
                }
                return Ok(());
            }
            GitAction::CheckCorrectBranch => git_action_functions::check_correct_branch()?,
            GitAction::CheckBranchDontExist { branch_name: _ } => {}
            GitAction::GetLastCommitMessage {
                last_commit_message,
            } => {
                let last_commit = git_action_functions::get_last_commit_message()?;
                *last_commit_message.borrow_mut() = last_commit;
                return Ok(());
            }
        }
        Ok(())
    }
}

mod git_action_functions {
    use super::*;
    pub fn check_correct_branch() -> GitResult<()> {
        let expected_auditor_branch = git::get_auditor_branch_name()?;
        let current_branch = git::get_current_branch_name()?;
        if current_branch != expected_auditor_branch {
            let message = format!(
                "Incorrect branch: \n -current: {}\n -expected: {}",
                current_branch, expected_auditor_branch
            );
            return Err(Report::new(GitError).attach_printable(message)).attach(Suggestion(
                format!(
                    "run \"{} {}\" or \"{}\" to move to the correct branch",
                    "git checkout".green(),
                    expected_auditor_branch.green(),
                    "bat-cli refresh".green()
                ),
            ));
        }
        Ok(())
    }

    pub fn checkout_branch(branch_name: &str) -> GitResult<()> {
        execute_command_with_child_process("git", &["checkout", branch_name])
            .change_context(GitError)?;
        Ok(())
    }

    pub fn get_last_commit_message() -> GitResult<String> {
        let last_commit_message = execute_command("git", &["log", "-1", "--pretty=%B"], false)
            .change_context(GitError)?
            .trim()
            .to_string();
        Ok(last_commit_message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batbelt::git::get_not_committed_files;
    use crate::batbelt::ShareableData;
    use std::env;
    use std::process::Command;

    #[test]
    fn test_get_commit() {
        // let output = Command::new("git")
        //     .args(["log", " -1", "--pretty=%B"])
        //     .output()
        //     .unwrap();
        // println!("output: {:#?}", output);
        // let msg = String::from_utf8(output.stdout.to_vec()).unwrap();
        // println!("msg: {}", msg);
        env::set_current_dir("../sage-audit").unwrap();
        let changes = get_not_committed_files().unwrap();
        println!("changes: {:#?}", changes);
        let shared_message = ShareableData::new(String::new());
        GitAction::GetLastCommitMessage {
            last_commit_message: shared_message.cloned,
        }
        .execute_action()
        .unwrap();
        println!("message: {}", shared_message.original.borrow_mut());
        assert_eq!("try ammend".to_string(), *shared_message.original.borrow());
    }
}
