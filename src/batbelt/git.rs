use std::error::Error;
use std::fmt;

use std::cell::RefCell;

use std::rc::Rc;
use std::str::from_utf8;
use std::{process::Command, str};

use super::path::BatFolder;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::metadata::BatMetadataType;
use crate::config::BatAuditorConfig;
use crate::{batbelt::path::BatFile, config::BatConfig};
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;

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
    Init,
    RemoteAddProjectRepo,
    AddAll,
    CheckGitIsInitialized { is_initialized: Rc<RefCell<bool>> },
    CheckBranchDontExist { branch_name: String },
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
            GitAction::CheckBranchDontExist { branch_name: _ } => {}
        }
        Ok(())
    }
}

pub fn check_correct_branch() -> Result<(), GitError> {
    let expected_auditor_branch = get_expected_current_branch()?;
    if get_branch_name()? != expected_auditor_branch {
        let message = format!(
            "You are in an incorrect branch, please run \"git checkout {}\"",
            expected_auditor_branch
        );
        return Err(Report::new(GitError).attach_printable(message));
    }
    Ok(())
}

pub fn get_expected_current_branch() -> Result<String, GitError> {
    let bat_config = BatConfig::get_config().change_context(GitError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitError)?;
    let expected_auditor_branch = format!(
        "{}-{}",
        bat_auditor_config.auditor_name, bat_config.project_name
    );
    Ok(expected_auditor_branch)
}

pub fn check_if_branch_exists(branch_name: &str) -> Result<bool, String> {
    let git_check_branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", branch_name])
        .output()
        .unwrap();
    Ok(git_check_branch_exists.stderr.is_empty())
}

// returns false if there are files to commit
pub fn check_files_not_commited() -> Result<bool, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    let output = from_utf8(output.stdout.as_slice()).unwrap().to_string();
    Ok(output.is_empty())
}

pub fn get_local_branches() -> Result<String, GitError> {
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

pub fn get_remote_branches() -> Result<String, GitError> {
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
pub fn get_branch_name() -> Result<String, GitError> {
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
    UpdateMetadata { metadata_type: BatMetadataType },
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
                    BatFile::PackageJson { for_init: false }
                        .get_path(true)
                        .change_context(GitError)?,
                    BatFolder::CodeOverhaulToReview
                        .get_path(true)
                        .change_context(GitError)?,
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
            GitCommit::UpdateMetadata { metadata_type } => {
                vec![metadata_type.get_path().change_context(GitError)?]
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
            GitCommit::UpdateMetadata { metadata_type } => {
                let metadata_type_string = metadata_type.to_string().to_plural().to_snake_case();
                format!("metadata: {}.md updated", metadata_type_string)
            }
        };
        Ok(commit_string)
    }
}

#[test]
fn test_get_branches_list() {
    let _branches_list = get_local_branches().unwrap();
}
