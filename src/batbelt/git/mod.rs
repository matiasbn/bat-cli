pub mod git_action;

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

pub enum GitCommit {
    Init,
    InitAuditor,
    StartCO {
        entrypoint_name: String,
    },
    FinishCO {
        entrypoint_name: String,
    },
    UpdateCO {
        entrypoint_name: String,
    },
    UpdateCOSummary,
    StartFinding {
        finding_name: String,
    },
    FinishFinding {
        finding_name: String,
    },
    RejectFinding {
        finding_name: String,
    },
    UpdateFinding {
        finding_name: String,
    },
    AcceptFindings,
    BatReload,
    Notes,
    UpdateMetadataJson {
        bat_metadata_commit: BatMetadataCommit,
    },
    UpdateBatToml,
    ProgramAccountMetadataCreated,
    ProgramAccountMetadataUpdated,
    CodeOverhaulUpdated {
        updated_eps: Vec<String>,
    },
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
            GitCommit::UpdateCOSummary => {
                vec![BatFile::CodeOverhaulSummaryFile
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
            GitCommit::BatReload => {
                vec![BatFile::GitIgnore.get_path(true).change_context(GitError)?]
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
            GitCommit::ProgramAccountMetadataCreated => {
                vec![BatFile::ProgramAccountsMetadataFile
                    .get_path(true)
                    .change_context(GitError)?]
            }
            GitCommit::ProgramAccountMetadataUpdated => {
                vec![BatFile::ProgramAccountsMetadataFile
                    .get_path(true)
                    .change_context(GitError)?]
            }
            GitCommit::UpdateBatToml => {
                vec![BatFile::BatToml.get_path(true).change_context(GitError)?]
            }
            GitCommit::UpdateMetadataJson { .. } => {
                vec![BatFile::BatMetadataFile
                    .get_path(false)
                    .change_context(GitError)?]
            }
            GitCommit::CodeOverhaulUpdated {
                updated_eps: file_path_vec,
            } => file_path_vec.clone(),
        };
        Ok(commit_files)
    }

    fn get_commit_message(&self) -> GitResult<String> {
        let bat_config = BatConfig::get_config().change_context(GitError)?;
        let commit_string = match self {
            GitCommit::Init => "initial commit".to_string(),
            GitCommit::InitAuditor => {
                let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitError)?;
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
            GitCommit::UpdateCOSummary => {
                format!("co: code_overhaul_summary.md updated")
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
            GitCommit::BatReload => "reload: project files updated".to_string(),
            GitCommit::Notes => {
                "notes: open_questions, finding_candidates and threat_modeling notes updated"
                    .to_string()
            }
            GitCommit::UpdateBatToml => "repo: .gitignore updated".to_string(),
            GitCommit::ProgramAccountMetadataCreated => {
                "metadata: program_account_metadata created".to_string()
            }
            GitCommit::ProgramAccountMetadataUpdated => {
                "metadata: program_account_metadata updated".to_string()
            }
            GitCommit::CodeOverhaulUpdated { .. } => "co: entry points updated".to_string(),
            GitCommit::UpdateMetadataJson {
                bat_metadata_commit,
            } => bat_metadata_commit.get_commit_message(),
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
