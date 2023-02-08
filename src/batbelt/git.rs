use std::str::from_utf8;
use std::{process::Command, str};

use colored::Colorize;

use crate::batbelt::constants::BASE_REPOSTORY_URL;
use crate::{
    batbelt::{self, path::FilePathType},
    config::BatConfig,
};

use super::{bash::execute_command, path::FolderPathType};

// Git
pub fn get_branch_name() -> Result<String, String> {
    let BatConfig { required: _, .. } = BatConfig::get_validated_config()?;
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
    UpdateMetadata,
    Figures,
    UpdateCOTemplates,
}

pub fn create_git_commit(
    commit_type: GitCommit,
    commit_files: Option<Vec<String>>,
) -> Result<(), String> {
    check_correct_branch()?;
    let (commit_message, commit_files_path): (String, Vec<String>) = match commit_type {
        GitCommit::Init => {
            let commit_string = "initial commit".to_string();
            (commit_string, vec![".".to_string()])
        }
        GitCommit::InitAuditor => {
            let bat_config = BatConfig::get_validated_config()?;
            let commit_string = format!(
                "co: project {} initialized for {}",
                bat_config.required.project_name, bat_config.auditor.auditor_name
            );
            // (commit_string, vec![utils::path::get_auditor_notes_path()?])
            (
                commit_string,
                vec![batbelt::path::get_folder_path(
                    FolderPathType::AuditorNotes,
                    true,
                )],
            )
        }
        GitCommit::StartCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("code-overhaul file started with commit: {commit_string:?}");
            let file_to_delete_path = batbelt::path::get_file_path(
                FilePathType::CodeOverhaulToReview {
                    file_name: commit_file.clone(),
                },
                false,
            );
            let file_to_add_path = batbelt::path::get_file_path(
                FilePathType::CodeOverhaulStarted {
                    file_name: commit_file.clone(),
                },
                false,
            );
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::StartCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = format!("co: {commit_file_name} started");
            println!("code-overhaul file started with commit: {commit_string}");
            let file_to_delete_path =
                // utils::path::get_auditor_code_overhaul_to_review_path(Some(commit_file.clone()))?;
                batbelt::path::get_file_path(FilePathType::CodeOverhaulToReview { file_name: commit_file.clone() }, false);
            let started_co_folder_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true);
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
            let file_to_delete_path =
                // utils::path::get_auditor_code_overhaul_to_review_path(Some(commit_file.clone()))?;
                batbelt::path::get_file_path(FilePathType::CodeOverhaulFinished { file_name:  commit_file.clone() }, true);
            let file_to_add_path = batbelt::path::get_file_path(
                FilePathType::CodeOverhaulFinished {
                    file_name: commit_file.clone(),
                },
                false,
            );
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::FinishCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = "co: ".to_string() + &commit_file_name + " finished";
            println!("code-overhaul file finished with commit: {commit_string:?}");
            // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
            let started_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true);
            let folder_to_delete_path = format!("{started_path}/{commit_file_name}");
            let finished_folder_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true);
            let file_to_add_path = format!("{finished_folder_path}/{commit_file}.md");
            (commit_string, vec![folder_to_delete_path, file_to_add_path])
        }
        GitCommit::DeployMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " deployed to Miro";
            println!("code-overhaul files deployed to Miro with commit: {commit_string:?}");
            let started_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true);
            let folder_to_add_path = format!("{started_path}/{entrypoint_name}");
            (commit_string, vec![folder_to_add_path])
        }
        GitCommit::UpdateMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " updated in Miro";
            println!("code-overhaul files updated in Miro with commit: {commit_string:?}");
            let started_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true);
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
                batbelt::path::get_file_path(FilePathType::CodeOverhaulFinished { file_name: commit_file.clone() }, true);
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::StartFinding => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "finding: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("finding file created with commit: {commit_string}");
            let file_to_add_path =
                // utils::path::get_file_path(FilePathType::FindingToReview { file_name: commit_file.clone() },true);
                batbelt::path::get_file_path(FilePathType::FindingToReview { file_name: commit_file.clone() }, true);
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
                batbelt::path::get_folder_path(FolderPathType::FindingsToReview, true);
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::AcceptAllFinding => {
            let commit_string = "finding: findings moved to the accepted folder".to_string();
            println!(
                "All findings moved to the accepted folder with commit: {}",
                commit_string.green()
            );
            let accepted_path =
                batbelt::path::get_folder_path(FolderPathType::FindingsAccepted, true);
            let to_review_path =
                batbelt::path::get_folder_path(FolderPathType::FindingsToReview, true);
            (commit_string, vec![accepted_path, to_review_path])
        }
        GitCommit::UpdateRepo => {
            let commit_string = "repo: templates and package.json update".to_string();
            // let file_to_add_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
            let file_to_add_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, true);
            let templates_path = batbelt::path::get_folder_path(FolderPathType::Templates, true);
            (
                commit_string,
                vec![file_to_add_path, templates_path, "package.json".to_string()],
            )
        }
        GitCommit::Notes => {
            println!("Creating a commit for open_questions.md, finding_candidates.md and threat_modeling.md");
            let open_questions_path =
                batbelt::path::get_file_path(FilePathType::OpenQuestions, true);
            let finding_candidates_path =
                batbelt::path::get_file_path(FilePathType::FindingCandidates, true);
            let threat_modeling_path =
                batbelt::path::get_file_path(FilePathType::ThreatModeling, true);
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
                batbelt::path::get_folder_path(FolderPathType::AuditResult, true);
            let audit_result_file_path =
                batbelt::path::get_file_path(FilePathType::AuditResult, true);
            let commit_string = format!("notes: audit_result updated");
            (
                commit_string,
                vec![audit_result_file_path, audit_result_folder_path],
            )
        }
        GitCommit::TMAccounts => {
            println!("Creating a commit for threat_modeling.md");
            let tm_path = batbelt::path::get_file_path(FilePathType::ThreatModeling, true);
            let commit_string = format!("notes: threat_modeling.md updated");
            (commit_string, vec![tm_path])
        }
        GitCommit::UpdateMetadata => {
            println!("Creating a commit for metadata.md");
            let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, true);
            let commit_string = format!("notes: metadata.md updated");
            (commit_string, vec![metadata_path])
        }
        GitCommit::Figures => {
            println!("Creating a commit for auditor figures");
            let figures_path = batbelt::path::get_folder_path(FolderPathType::AuditorFigures, true);
            let commit_string = format!("notes: figures updated");
            (commit_string, vec![figures_path])
        }
        GitCommit::UpdateCOTemplates => {
            println!("Creating a commit for updated CO templates");
            let to_review_co_path =
                batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, true);
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

pub fn check_correct_branch() -> Result<(), String> {
    let expected_auditor_branch = get_expected_current_branch()?;
    if get_branch_name()? != expected_auditor_branch {
        panic!(
            "You are in an incorrect branch, please run \"git checkout {:?}\"",
            expected_auditor_branch
        );
    }
    Ok(())
}

pub fn get_expected_current_branch() -> Result<String, String> {
    let bat_config = BatConfig::get_validated_config()?;
    let expected_auditor_branch = format!(
        "{}-{}",
        bat_config.auditor.auditor_name, bat_config.required.project_name
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

pub fn clone_base_repository() {
    // Clone base repository
    Command::new("git")
        .args(["clone", BASE_REPOSTORY_URL])
        .output()
        .unwrap();
}

pub fn git_push() -> Result<(), String> {
    Command::new("git").arg("push").output().unwrap();
    Ok(())
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

#[test]
fn test_create_git_commit() {
    create_git_commit(GitCommit::FinishCO, Some(vec!["test_co_file".to_string()])).unwrap();
}
