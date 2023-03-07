use std::error::Error;
use std::fmt;

use std::cell::RefCell;

use colored::Colorize;
use std::rc::Rc;
use std::str::from_utf8;
use std::{process::Command, str};

use super::path::BatFolder;
use crate::batbelt::command_line::{execute_command, execute_command_with_child_process};

use crate::config::BatAuditorConfig;
use crate::{batbelt::path::BatFile, config::BatConfig, Suggestion};
use error_stack::{IntoReport, Report, Result, ResultExt};

#[derive(Debug)]
pub struct GitError;

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Git operation error")
    }
}

impl Error for GitError {}

type GitResult<T> = Result<T, GitError>;

#[derive(Debug, PartialEq, strum_macros::Display)]
pub enum GitAction {
    CreateBranch { branch_name: String },
    CheckoutAuditorBranch,
    Init,
    RemoteAddProjectRepo,
    AddAll,
    CheckGitIsInitialized { is_initialized: Rc<RefCell<bool>> },
    CheckBranchDontExist { branch_name: String },
    CheckCorrectBranch,
}

impl GitAction {
    pub fn execute_action(&self) -> GitResult<()> {
        let bat_config = BatConfig::get_config().change_context(GitError)?;
        match self {
            GitAction::Init => {
                execute_command("git", &["init"], false).change_context(GitError)?;
            }
            GitAction::RemoteAddProjectRepo => {
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
                let auditor_branch_name = get_auditor_branch_name()?;
                if get_current_branch_name()? != auditor_branch_name {
                    self.checkout_branch(&auditor_branch_name)?
                }
                return Ok(());
            }
            GitAction::CheckCorrectBranch => self.check_correct_branch()?,
            GitAction::CheckBranchDontExist { branch_name: _ } => {}
        }
        Ok(())
    }

    fn check_correct_branch(&self) -> GitResult<()> {
        let expected_auditor_branch = get_auditor_branch_name()?;
        let current_branch = get_current_branch_name()?;
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

    fn checkout_branch(&self, branch_name: &str) -> GitResult<()> {
        execute_command_with_child_process("git", &["checkout", branch_name])
            .change_context(GitError)?;
        Ok(())
    }
}

pub fn deprecated_check_correct_branch() -> GitResult<()> {
    let expected_auditor_branch = get_auditor_branch_name()?;
    let current_branch = get_current_branch_name()?;
    if current_branch != expected_auditor_branch {
        let message = format!(
            "Incorrect branch: \n -current: {}\n -expected: {}",
            current_branch, expected_auditor_branch
        );
        return Err(Report::new(GitError).attach_printable(message)).attach(Suggestion(format!(
            "run \"{} {}\" or \"{}\" to move to the correct branch",
            "git checkout".green(),
            expected_auditor_branch.green(),
            "bat-cli refresh".green()
        )));
    }
    Ok(())
}

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

// returns false if there are files to commit
pub fn check_files_not_committed() -> GitResult<()> {
    let output =
        execute_command("git", &["status", "--porcelain"], false).change_context(GitError)?;
    let modified_files = output
        .lines()
        .map(|line| line.trim().trim_start_matches("M ").to_string())
        .collect::<Vec<_>>();
    if !modified_files.is_empty() {
        let message = format!(
            "There are modified files that needs to be committed:\n{:#?}",
            modified_files
        );
        return Err(Report::new(GitError).attach_printable(message));
    }
    Ok(())
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

pub enum GitCommit {
    Init,
    InitAuditor,
    StartCO { entrypoint_name: String },
    FinishCO { entrypoint_name: String },
    UpdateCO { entrypoint_name: String },
    StartFinding { finding_name: String },
    FinishFinding { finding_name: String },
    RejectFinding { finding_name: String },
    UpdateFinding { finding_name: String },
    AcceptFindings,
    UpdateTemplates,
    Notes,
    UpdateMetadataJson,
}

impl GitCommit {
    pub fn create_commit(&self) -> GitResult<()> {
        let commit_message = self.get_commit_message()?;
        let commit_files = self.get_commit_files()?;
        for commit_file in commit_files {
            execute_command("git", &["add", commit_file.as_str()], false)
                .change_context(GitError)?;
        }
        execute_command("git", &["commit", "-m", commit_message.as_str()], false)
            .change_context(GitError)?;
        Ok(())
    }

    fn get_commit_files(&self) -> GitResult<Vec<String>> {
        let commit_files = match self {
            GitCommit::Init => {
                vec![".".to_string()]
            }
            GitCommit::InitAuditor => {
                vec![BatFolder::AuditorNotes
                    .get_path(true)
                    .change_context(GitError)?]
            }
            GitCommit::StartCO { entrypoint_name } => {
                vec![
                    BatFile::CodeOverhaulToReview {
                        file_name: entrypoint_name.clone(),
                    }
                    .get_path(false)
                    .change_context(GitError)?,
                    BatFile::CodeOverhaulStarted {
                        file_name: entrypoint_name.clone(),
                    }
                    .get_path(true)
                    .change_context(GitError)?,
                    BatFile::BatMetadataFile
                        .get_path(false)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::FinishCO { entrypoint_name } => {
                vec![
                    BatFile::CodeOverhaulStarted {
                        file_name: entrypoint_name.clone(),
                    }
                    .get_path(false)
                    .change_context(GitError)?,
                    BatFile::CodeOverhaulFinished {
                        file_name: entrypoint_name.clone(),
                    }
                    .get_path(true)
                    .change_context(GitError)?,
                ]
            }
            GitCommit::UpdateCO { entrypoint_name } => {
                vec![BatFile::CodeOverhaulFinished {
                    file_name: entrypoint_name.clone(),
                }
                .get_path(true)
                .change_context(GitError)?]
            }
            GitCommit::StartFinding { finding_name } => {
                vec![BatFile::FindingToReview {
                    file_name: finding_name.clone(),
                }
                .get_path(true)
                .change_context(GitError)?]
            }
            GitCommit::FinishFinding { finding_name } => {
                vec![
                    BatFile::FindingToReview {
                        file_name: finding_name.clone(),
                    }
                    .get_path(true)
                    .change_context(GitError)?,
                    BatFolder::AuditorFigures
                        .get_path(true)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::UpdateFinding { finding_name } => {
                vec![
                    BatFile::FindingToReview {
                        file_name: finding_name.clone(),
                    }
                    .get_path(true)
                    .change_context(GitError)?,
                    BatFolder::AuditorFigures
                        .get_path(true)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::RejectFinding { finding_name } => {
                vec![
                    BatFile::FindingToReview {
                        file_name: finding_name.clone(),
                    }
                    .get_path(false)
                    .change_context(GitError)?,
                    BatFile::FindingRejected {
                        file_name: finding_name.clone(),
                    }
                    .get_path(true)
                    .change_context(GitError)?,
                    BatFolder::AuditorFigures
                        .get_path(true)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::AcceptFindings => {
                vec![
                    BatFolder::FindingsAccepted
                        .get_path(true)
                        .change_context(GitError)?,
                    BatFolder::AuditorFigures
                        .get_path(true)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::UpdateTemplates => {
                vec![
                    BatFolder::CodeOverhaulToReview
                        .get_path(true)
                        .change_context(GitError)?,
                    BatFile::GitIgnore.get_path(true).change_context(GitError)?,
                ]
            }
            GitCommit::Notes => {
                vec![
                    BatFile::FindingCandidates
                        .get_path(true)
                        .change_context(GitError)?,
                    BatFile::ThreatModeling
                        .get_path(true)
                        .change_context(GitError)?,
                    BatFile::OpenQuestions
                        .get_path(true)
                        .change_context(GitError)?,
                ]
            }
            GitCommit::UpdateMetadataJson => {
                vec![BatFile::BatMetadataFile
                    .get_path(false)
                    .change_context(GitError)?]
            }
        };
        Ok(commit_files)
    }

    fn get_commit_message(&self) -> GitResult<String> {
        let bat_config = BatConfig::get_config().change_context(GitError)?;
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitError)?;
        let commit_string = match self {
            GitCommit::Init => "initial commit".to_string(),
            GitCommit::InitAuditor => {
                format!(
                    "co: project {} initialized for {}",
                    bat_config.project_name, bat_auditor_config.auditor_name
                )
            }
            GitCommit::StartCO { entrypoint_name } => {
                format!("co: {} started", entrypoint_name)
            }
            GitCommit::FinishCO { entrypoint_name } => {
                format!("co: {} finished", entrypoint_name)
            }
            GitCommit::UpdateCO { entrypoint_name } => {
                format!("co: {} updated", entrypoint_name)
            }
            GitCommit::StartFinding { finding_name } => {
                format!("finding: {} started", finding_name)
            }
            GitCommit::FinishFinding { finding_name } => {
                format!("finding: {} finished", finding_name)
            }
            GitCommit::RejectFinding { finding_name } => {
                format!("finding: {} rejected", finding_name)
            }
            GitCommit::UpdateFinding { finding_name } => {
                format!("finding: {} updated", finding_name)
            }
            GitCommit::AcceptFindings => {
                "finding: to-review findings moved to accepted".to_string()
            }
            GitCommit::UpdateTemplates => "templates: templates update".to_string(),
            GitCommit::Notes => {
                "notes: open_questions, finding_candidates and threat_modeling notes updated"
                    .to_string()
            }
            GitCommit::UpdateMetadataJson => "metadata: metadata.json updated".to_string(),
        };
        Ok(commit_string)
    }
}

#[test]
fn test_get_branches_list() {
    let _branches_list = get_local_branches().unwrap();
}

#[test]
fn test_check_files_not_committed() {
    env_logger::init();
    check_files_not_committed().unwrap();
}
