use crate::batbelt::command_line::execute_command;
use crate::batbelt::git::git_action::GitAction;
use crate::batbelt::git::{GitError, GitResult};
use crate::batbelt::metadata::BatMetadataCommit;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::ShareableData;
use crate::config::{BatAuditorConfig, BatConfig};
use error_stack::ResultExt;

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
    BatFile {
        bat_file: BatFile,
        commit_message: String,
    },
}

impl GitCommit {
    pub fn create_commit(&self, try_amend: bool) -> GitResult<()> {
        let commit_message = self.get_commit_message()?;
        let commit_files = self.get_commit_files()?;
        for commit_file in commit_files {
            execute_command("git", &["add", commit_file.as_str()], false)
                .change_context(GitError)?;
        }
        let shared_last_message = ShareableData::new(String::new());
        GitAction::GetLastCommitMessage {
            last_commit_message: shared_last_message.cloned,
        }
        .execute_action()?;
        if try_amend && commit_message == *shared_last_message.original.borrow_mut() {
            execute_command("git", &["commit", "--amend", "--no-edit"], false)
                .change_context(GitError)?;
        } else {
            execute_command("git", &["commit", "-m", commit_message.as_str()], false)
                .change_context(GitError)?;
        }
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
            GitCommit::BatFile { bat_file, .. } => {
                vec![bat_file.get_path(false).change_context(GitError)?]
            }
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
            GitCommit::BatFile { commit_message, .. } => commit_message.clone(),
            GitCommit::UpdateMetadataJson {
                bat_metadata_commit,
            } => bat_metadata_commit.get_commit_message(),
        };
        Ok(commit_string)
    }
}
