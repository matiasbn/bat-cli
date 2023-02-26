use std::error::Error;
use std::fmt;

use std::str::from_utf8;
use std::{process::Command, str};

use colored::Colorize;

use super::path::BatFolder;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::metadata::BatMetadataType;
use crate::config::BatAuditorConfig;
use crate::{
    batbelt::{self, path::BatFile},
    config::BatConfig,
};
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;

#[derive(Debug)]
pub struct GitOperationError;

impl fmt::Display for GitOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Git operation error")
    }
}

impl Error for GitOperationError {}

// Git
pub fn get_branch_name() -> Result<String, GitOperationError> {
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
    StartCO,
    StartCOMiro,
    FinishCO,
    FinishCOMiro,
    DeployMiro,
    UpdateMiro,
    UpdateCO,
    StartFinding,
    FinishFinding {
        finding_name: String,
        to_review_finding_file_path: String,
        auditor_figures_folder_path: String,
    },
    UpdateFinding {
        finding_name: String,
        to_review_finding_file_path: String,
        auditor_figures_folder_path: String,
    },
    PrepareAllFinding,
    AcceptAllFinding,
    UpdateRepo,
    Notes,
    AuditResult,
    TMAccounts,
    UpdateMetadata {
        metadata_type: BatMetadataType,
    },
    Figures,
    UpdateCOTemplates,
}

pub fn create_git_commit(
    commit_type: GitCommit,
    commit_files: Option<Vec<String>>,
) -> Result<(), GitOperationError> {
    check_correct_branch()?;
    let bat_config = BatConfig::get_config().change_context(GitOperationError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitOperationError)?;
    let (commit_message, commit_files_path): (String, Vec<String>) = match commit_type {
        GitCommit::Init => {
            let commit_string = "initial commit".to_string();
            (commit_string, vec![".".to_string()])
        }
        GitCommit::InitAuditor => {
            let commit_string = format!(
                "co: project {} initialized for {}",
                bat_config.project_name, bat_auditor_config.auditor_name
            );
            // (commit_string, vec![utils::path::get_auditor_notes_path()?])
            (commit_string, vec![".".to_string()])
        }
        GitCommit::StartCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("code-overhaul file started with commit: {commit_string:?}");
            let file_to_delete_path = batbelt::path::get_file_path(
                BatFile::CodeOverhaulToReview {
                    file_name: commit_file.clone(),
                },
                false,
            )
            .change_context(GitOperationError)?;
            let file_to_add_path = batbelt::path::get_file_path(
                BatFile::CodeOverhaulStarted {
                    file_name: commit_file.clone(),
                },
                false,
            )
            .change_context(GitOperationError)?;

            // let metadata_path = batbelt::path::get_file_path(BatFile::Metadata, false)
            //     .change_context(GitOperationError)?;
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::StartCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = format!("co: {commit_file_name} started");
            println!("code-overhaul file started with commit: {commit_string}");
            let file_to_delete_path = batbelt::path::get_file_path(
                BatFile::CodeOverhaulToReview {
                    file_name: commit_file.clone(),
                },
                false,
            )
            .change_context(GitOperationError)?;
            let started_co_folder_path =
                batbelt::path::get_folder_path(BatFolder::CodeOverhaulStarted, true)
                    .change_context(GitOperationError)?;
            (
                commit_string,
                vec![
                    file_to_delete_path,
                    // started_path/commit_file_name <- folder
                    format!("{started_co_folder_path}/{commit_file_name}"),
                ],
            )
        }
        GitCommit::FinishCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " finished";
            println!("code-overhaul file finished with commit: {commit_string:?}");
            let file_to_delete_path = batbelt::path::get_file_path(
                BatFile::CodeOverhaulFinished {
                    file_name: commit_file.clone(),
                },
                true,
            )
            .change_context(GitOperationError)?;
            let file_to_add_path = batbelt::path::get_file_path(
                BatFile::CodeOverhaulFinished {
                    file_name: commit_file.clone(),
                },
                false,
            )
            .change_context(GitOperationError)?;
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::FinishCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = "co: ".to_string() + &commit_file_name + " finished";
            println!("code-overhaul file finished with commit: {commit_string:?}");
            // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
            let started_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulStarted, true)
                .change_context(GitOperationError)?;
            let folder_to_delete_path = format!("{started_path}/{commit_file_name}");
            let finished_folder_path =
                batbelt::path::get_folder_path(BatFolder::CodeOverhaulFinished, true)
                    .change_context(GitOperationError)?;
            let file_to_add_path = format!("{finished_folder_path}/{commit_file}.md");
            (commit_string, vec![folder_to_delete_path, file_to_add_path])
        }
        GitCommit::DeployMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " deployed to Miro";
            println!("code-overhaul files deployed to Miro with commit: {commit_string:?}");
            let started_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulStarted, true)
                .change_context(GitOperationError)?;
            (commit_string, vec![started_path])
        }
        GitCommit::UpdateMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " updated in Miro";
            println!("code-overhaul files updated in Miro with commit: {commit_string:?}");
            let started_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulStarted, true)
                .change_context(GitOperationError)?;
            let folder_to_add_path = format!("{started_path}/{entrypoint_name}");
            (commit_string, vec![folder_to_add_path])
        }
        GitCommit::UpdateCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " updated";
            println!("code-overhaul file updated with commit: {commit_string:?}");
            let file_to_add_path =
                // utils::path::get_auditor_code_overhaul_finished_path(Some(commit_file.clone()))?;
                batbelt::path::get_file_path(BatFile::CodeOverhaulFinished { file_name: commit_file.clone() }, true).change_context(GitOperationError)?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::StartFinding => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "finding: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("finding file created with commit: {commit_string}");
            let file_to_add_path =
                // utils::path::get_file_path(FilePathType::FindingToReview { file_name: commit_file.clone() },true);
                batbelt::path::get_file_path(BatFile::FindingToReview { file_name: commit_file.clone() }, true).change_context(GitOperationError)?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::FinishFinding {
            finding_name,
            to_review_finding_file_path,
            auditor_figures_folder_path,
        } => {
            let commit_string = format!("finding: {finding_name} finished");
            println!(
                "finding file finished with commit: {}",
                commit_string.green()
            );
            (
                commit_string,
                vec![to_review_finding_file_path, auditor_figures_folder_path],
            )
        }
        GitCommit::UpdateFinding {
            finding_name,
            to_review_finding_file_path,
            auditor_figures_folder_path,
        } => {
            let commit_string = format!("finding: {finding_name} updated");
            println!(
                "finding file udpated with commit: {}",
                commit_string.green()
            );

            (
                commit_string,
                vec![to_review_finding_file_path, auditor_figures_folder_path],
            )
        }
        GitCommit::PrepareAllFinding => {
            let commit_string = "finding: to-review findings severity updated".to_string();
            println!("updating findings severity in repository");
            let file_to_add_path =
                batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
                    .change_context(GitOperationError)?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::AcceptAllFinding => {
            let commit_string = "finding: findings moved to the accepted folder".to_string();
            println!(
                "All findings moved to the accepted folder with commit: {}",
                commit_string.green()
            );
            let accepted_path = batbelt::path::get_folder_path(BatFolder::FindingsAccepted, true)
                .change_context(GitOperationError)?;
            let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
                .change_context(GitOperationError)?;
            (commit_string, vec![accepted_path, to_review_path])
        }
        GitCommit::UpdateRepo => {
            let commit_string = "repo: templates and package.json update".to_string();
            // let file_to_add_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
            let file_to_add_path =
                batbelt::path::get_folder_path(BatFolder::CodeOverhaulToReview, true)
                    .change_context(GitOperationError)?;
            let templates_path = batbelt::path::get_folder_path(BatFolder::Templates, true)
                .change_context(GitOperationError)?;
            (
                commit_string,
                vec![file_to_add_path, templates_path, "package.json".to_string()],
            )
        }
        GitCommit::Notes => {
            println!("Creating a commit for open_questions.md, finding_candidates.md and threat_modeling.md");
            let open_questions_path = batbelt::path::get_file_path(BatFile::OpenQuestions, true)
                .change_context(GitOperationError)?;
            let finding_candidates_path =
                batbelt::path::get_file_path(BatFile::FindingCandidates, true)
                    .change_context(GitOperationError)?;
            let threat_modeling_path = batbelt::path::get_file_path(BatFile::ThreatModeling, true)
                .change_context(GitOperationError)?;
            let commit_string =
                "notes: open_questions, finding_candidates and threat_modeling notes".to_string();
            println!("{commit_string}");
            (
                commit_string,
                vec![
                    open_questions_path,
                    finding_candidates_path,
                    threat_modeling_path,
                ],
            )
        }
        GitCommit::AuditResult => {
            println!("Creating a commit for {}", "audit_result".green());
            let audit_result_folder_path =
                batbelt::path::get_folder_path(BatFolder::AuditResult, true)
                    .change_context(GitOperationError)?;
            let audit_result_file_path = batbelt::path::get_file_path(BatFile::AuditResult, true)
                .change_context(GitOperationError)?;
            let commit_string = format!("notes: audit_result updated");
            (
                commit_string,
                vec![audit_result_file_path, audit_result_folder_path],
            )
        }
        GitCommit::TMAccounts => {
            println!("Creating a commit for threat_modeling.md");
            let tm_path = batbelt::path::get_file_path(BatFile::ThreatModeling, true)
                .change_context(GitOperationError)?;
            let commit_string = format!("notes: threat_modeling.md updated");
            (commit_string, vec![tm_path])
        }
        GitCommit::UpdateMetadata { metadata_type } => {
            let metadata_type_string = metadata_type.to_string().to_plural().to_snake_case();
            println!("Creating a commit for {}.md", metadata_type_string);
            let metadata_path = metadata_type.get_path().change_context(GitOperationError)?;
            let commit_string = format!("metadata: {}.md updated", metadata_type_string);
            (commit_string, vec![metadata_path])
        }
        GitCommit::Figures => {
            println!("Creating a commit for auditor figures");
            let figures_path = batbelt::path::get_folder_path(BatFolder::AuditorFigures, true)
                .change_context(GitOperationError)?;
            let commit_string = format!("notes: figures updated");
            (commit_string, vec![figures_path])
        }
        GitCommit::UpdateCOTemplates => {
            println!("Creating a commit for updated CO templates");
            let to_review_co_path =
                batbelt::path::get_folder_path(BatFolder::CodeOverhaulToReview, true)
                    .change_context(GitOperationError)?;
            let commit_string = format!("templates: co files updated");
            (commit_string, vec![to_review_co_path])
        }
    };

    for commit_file in commit_files_path {
        execute_command("git", &["add", commit_file.as_str()]).unwrap();
    }
    execute_command("git", &["commit", "-m", commit_message.as_str()]).unwrap();
    Ok(())
}

pub fn check_correct_branch() -> Result<(), GitOperationError> {
    let expected_auditor_branch = get_expected_current_branch()?;
    if get_branch_name()? != expected_auditor_branch {
        let message = format!(
            "You are in an incorrect branch, please run \"git checkout {}\"",
            expected_auditor_branch
        );
        return Err(Report::new(GitOperationError).attach_printable(message));
    }
    Ok(())
}

pub fn get_expected_current_branch() -> Result<String, GitOperationError> {
    let bat_config = BatConfig::get_config().change_context(GitOperationError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(GitOperationError)?;
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

pub fn get_local_branches() -> Result<String, GitOperationError> {
    let branches_list = Command::new("git")
        .args(&["branch", "--list"])
        .output()
        .into_report()
        .change_context(GitOperationError)?;
    let list = from_utf8(branches_list.stdout.as_slice())
        .into_report()
        .change_context(GitOperationError)?;
    Ok(list.to_string())
}

pub fn get_remote_branches() -> Result<String, GitOperationError> {
    let branches_list = Command::new("git")
        .args(&["branch", "-r", "--list"])
        .output()
        .into_report()
        .change_context(GitOperationError)?;
    let list = from_utf8(branches_list.stdout.as_slice())
        .into_report()
        .change_context(GitOperationError)?;
    Ok(list.to_string())
}

#[test]
fn test_get_branches_list() {
    let _branches_list = get_local_branches().unwrap();
}
