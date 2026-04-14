pub mod git_action;
pub mod git_commit;

use std::error::Error;
use std::fmt;

use std::cell::RefCell;

use colored::Colorize;
use std::rc::Rc;
use std::str::from_utf8;
use std::{process::Command, str};

use super::path::BatFolder;
use crate::batbelt::command_line::{execute_command, execute_command_with_child_process};

use crate::batbelt::metadata::BatMetadataCommit;
use crate::config::BatAuditorConfig;
use crate::{batbelt::path::BatFile, config::BatConfig, Suggestion};
use error_stack::{IntoReport, Report, Result, ResultExt};
use regex::Regex;

#[derive(Debug)]
pub struct GitError;

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Git operation error")
    }
}

impl Error for GitError {}

type GitResult<T> = Result<T, GitError>;

pub fn get_auditor_branch_name() -> GitResult<String> {
    let bat_config = BatConfig::get_config().change_context(GitError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitError)?;
    let expected_auditor_branch = format!(
        "{}-{}",
        bat_auditor_config.auditor_name, bat_config.project_name
    );
    Ok(expected_auditor_branch)
}

pub fn check_if_branch_exists(branch_name: &str) -> GitResult<bool> {
    let git_check_branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", branch_name])
        .output()
        .unwrap();
    Ok(git_check_branch_exists.stderr.is_empty())
}

pub fn check_files_not_committed() -> GitResult<()> {
    let modified_files = get_not_committed_files()?;
    if !modified_files.is_empty() {
        let message = format!(
            "There are modified files that needs to be committed:\n{:#?}",
            modified_files
        );
        return Err(Report::new(GitError).attach_printable(message));
    }
    Ok(())
}

// returns false if there are files to commit
pub fn get_not_committed_files() -> GitResult<Vec<String>> {
    let output =
        execute_command("git", &["status", "--porcelain"], false).change_context(GitError)?;
    let modified_files = output
        .lines()
        .map(|line| line.trim().trim_start_matches("M ").to_string())
        .collect::<Vec<_>>();
    Ok(modified_files)
}

pub fn get_local_branches() -> GitResult<String> {
    let branches_list = Command::new("git")
        .args(["branch", "--list"])
        .output()
        .into_report()
        .change_context(GitError)?;
    let list = from_utf8(branches_list.stdout.as_slice())
        .into_report()
        .change_context(GitError)?;
    Ok(list.to_string())
}

pub fn get_remote_branches() -> GitResult<String> {
    let branches_list = Command::new("git")
        .args(["branch", "-r", "--list"])
        .output()
        .into_report()
        .change_context(GitError)?;
    let list = from_utf8(branches_list.stdout.as_slice())
        .into_report()
        .change_context(GitError)?;
    Ok(list.to_string())
}

// Git
pub fn get_current_branch_name() -> GitResult<String> {
    let git_symbolic = Command::new("git")
        .args(["symbolic-ref", "-q", "head"])
        .output();
    let output = git_symbolic.unwrap();
    let git_branch_slice = str::from_utf8(output.stdout.as_slice()).unwrap();
    let git_branch_tokenized = git_branch_slice.split('/').collect::<Vec<&str>>();
    let git_branch = git_branch_tokenized
        .last()
        .unwrap()
        .split('\n')
        .collect::<Vec<&str>>()[0];
    Ok(git_branch.to_owned())
}

#[test]
fn test_get_branches_list() {
    let _branches_list = get_local_branches().unwrap();
}

