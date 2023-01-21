use std::{process::Command, str};

use crate::{
    command_line::execute_command,
    config::{BatConfig, RequiredConfig},
    constants::{self, AUDIT_RESULT_FILE_NAME, BASE_REPOSTORY_URL},
};

// Git
pub fn get_branch_name() -> Result<String, String> {
    let BatConfig { required, .. } = BatConfig::get_validated_config()?;
    let RequiredConfig {
        audit_folder_path, ..
    } = required;
    let git_symbolic = Command::new("git")
        .current_dir(audit_folder_path)
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
    FinishFinding,
    UpdateFinding,
    PrepareAllFinding,
    UpdateRepo,
    Notes,
    Results,
    TMAccounts,
}

pub fn create_git_commit(
    commit_type: GitCommit,
    commit_files: Option<Vec<String>>,
) -> Result<(), String> {
    check_correct_branch();
    let (commit_message, commit_files_path): (String, Vec<String>) = match commit_type {
        GitCommit::Init => {
            let commit_string = "initial commit".to_string();
            (commit_string, vec![BatConfig::get_audit_folder_path(None)?])
        }
        GitCommit::InitAuditor => {
            let commit_string =
                "co: project initialized for ".to_string() + &BatConfig::get_auditor_name()?;
            (commit_string, vec![BatConfig::get_auditor_notes_path()?])
        }
        GitCommit::StartCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("code-overhaul file started with commit: {commit_string:?}");
            let file_to_delete_path =
                BatConfig::get_auditor_code_overhaul_to_review_path(Some(commit_file.clone()))?;
            let file_to_add_path =
                BatConfig::get_auditor_code_overhaul_started_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::StartCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = format!("co: {commit_file_name} started");
            println!("code-overhaul file started with commit: {commit_string}");
            let file_to_delete_path =
                BatConfig::get_auditor_code_overhaul_to_review_path(Some(commit_file.clone()))?;
            let file_to_add_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
            (
                commit_string,
                vec![
                    file_to_delete_path,
                    // started_path/commit_file_name <- folder
                    format!("{file_to_add_path}{commit_file_name}"),
                ],
            )
        }
        GitCommit::FinishCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " finished";
            println!("code-overhaul file finished with commit: {commit_string:?}");
            let file_to_delete_path =
                BatConfig::get_auditor_code_overhaul_started_path(Some(commit_file.clone()))?;
            let file_to_add_path =
                BatConfig::get_auditor_code_overhaul_finished_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_delete_path, file_to_add_path])
        }
        GitCommit::FinishCOMiro => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_file_name = commit_file.clone().replace(".md", "");
            let commit_string = "co: ".to_string() + &commit_file_name + " finished";
            println!("code-overhaul file finished with commit: {commit_string:?}");
            let started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
            let folder_to_delete_path = format!("{started_path}/{commit_file_name}");
            let finished_folder_path = BatConfig::get_auditor_code_overhaul_finished_path(None)?;
            let file_to_add_path = format!("{finished_folder_path}{commit_file}.md");
            (commit_string, vec![folder_to_delete_path, file_to_add_path])
        }
        GitCommit::DeployMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " deployed to Miro";
            println!("code-overhaul files deployed to Miro with commit: {commit_string:?}");
            let started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
            let folder_to_add_path = format!("{started_path}/{entrypoint_name}");
            (commit_string, vec![folder_to_add_path])
        }
        GitCommit::UpdateMiro => {
            let entrypoint_name = &commit_files.unwrap()[0];
            let commit_string = "co: ".to_string() + entrypoint_name + " updated in Miro";
            println!("code-overhaul files updated in Miro with commit: {commit_string:?}");
            let started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
            let folder_to_add_path = format!("{started_path}/{entrypoint_name}");
            (commit_string, vec![folder_to_add_path])
        }
        GitCommit::UpdateCO => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "co: ".to_string() + &commit_file.clone().replace(".md", "") + " updated";
            println!("code-overhaul file updated with commit: {commit_string:?}");
            let file_to_add_path =
                BatConfig::get_auditor_code_overhaul_finished_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::StartFinding => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "finding: ".to_string() + &commit_file.clone().replace(".md", "") + " started";
            println!("finding file created with commit: \"{commit_string}\"");
            let file_to_add_path =
                BatConfig::get_auditor_findings_to_review_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::FinishFinding => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "finding: ".to_string() + &commit_file.clone().replace(".md", "") + " finished";
            println!("finding file finished with commit: \"{commit_string}\"");
            let file_to_add_path =
                BatConfig::get_auditor_findings_to_review_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::UpdateFinding => {
            let commit_file = &commit_files.unwrap()[0];
            let commit_string =
                "finding: ".to_string() + &commit_file.clone().replace(".md", "") + " updated";
            println!("finding file updated with commit: \"{commit_string}\"");
            let file_to_add_path =
                BatConfig::get_auditor_findings_to_review_path(Some(commit_file.clone()))?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::PrepareAllFinding => {
            let commit_string = "finding: to-review findings severity updated".to_string();
            println!("updating findings severity in repository");
            let file_to_add_path = BatConfig::get_auditor_findings_to_review_path(None)?;
            (commit_string, vec![file_to_add_path])
        }
        GitCommit::UpdateRepo => {
            let commit_string = "repo: templates and package.json update".to_string();
            let file_to_add_path = BatConfig::get_auditor_code_overhaul_to_review_path(None)?;
            let packagejson_path =
                BatConfig::get_audit_folder_path(Some("package.json".to_string()))?;
            let templates_path = BatConfig::get_templates_path()?;
            (
                commit_string,
                vec![file_to_add_path, templates_path, packagejson_path],
            )
        }
        GitCommit::Notes => {
            println!("Creating a commit for open_questions.md, smellies.md and threat_modeling.md");
            let auditor_notes_path = BatConfig::get_auditor_notes_path()?;
            let open_questions_path = auditor_notes_path.clone() + "open_questions.md";
            let smellies_path = auditor_notes_path.clone() + "smellies.md";
            let threat_modeling_path = auditor_notes_path + "threat_modeling.md";
            let commit_string =
                "notes: open_questions, smellies and threat_modeling notes".to_string();
            println!("{commit_string}");
            (
                commit_string,
                vec![open_questions_path, smellies_path, threat_modeling_path],
            )
        }
        GitCommit::Results => {
            println!(
                "Creating a commit for {}",
                constants::AUDIT_RESULT_FILE_NAME
            );
            let audit_result_path = BatConfig::get_audit_folder_path(Some(
                constants::AUDIT_RESULT_FILE_NAME.to_string(),
            ))?;
            let commit_string = format!("notes: {} updated", AUDIT_RESULT_FILE_NAME);
            (commit_string, vec![audit_result_path])
        }
        GitCommit::TMAccounts => {
            println!("Creating a commit for threat_modeling.md");
            let tm_path = BatConfig::get_auditor_threat_modeling_path()?;
            let commit_string = format!("notes: threat_modeling.md updated");
            (commit_string, vec![tm_path])
        }
        _ => panic!("Wrong GitCommit type input"),
    };

    for commit_file in commit_files_path {
        let output = Command::new("git")
            .args(["add", commit_file.as_str()])
            .output()
            .unwrap();
        if !output.stderr.is_empty() {
            panic!(
                "git commit creation failed with error: {:?}",
                std::str::from_utf8(output.stderr.as_slice()).unwrap()
            )
        };
    }
    let output = Command::new("git")
        .args(["commit", "-m", commit_message.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "git commit creation failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    Ok(())
}

pub fn check_correct_branch() -> Result<(), String> {
    let expected_auditor_branch = BatConfig::get_auditor_name()? + "-notes";
    if get_branch_name()? != expected_auditor_branch {
        panic!(
            "You are in an incorrect branch, please run \"git checkout {:?}\"",
            expected_auditor_branch.replace('\"', "")
        );
    }
    Ok(())
}

pub fn clone_base_repository() {
    // Clone base repository
    Command::new("git")
        .args(["clone", BASE_REPOSTORY_URL])
        .output()
        .unwrap();
}

pub fn git_push() {
    Command::new("git").arg("push").output().unwrap();
}

// returns false if there are files to commit
pub fn check_files_not_commited() -> Result<bool, String> {
    let output = execute_command(
        "git".to_string(),
        vec!["status", "--porcelain"],
        "error running git status".to_string(),
    )?;
    Ok(output.is_empty())
}

pub fn checkout_main_branch() -> Result<(), String> {
    execute_command(
        "git".to_string(),
        vec!["checkout", "main"],
        "error on running git checkout main".to_string(),
    )?;
    Ok(())
}

#[test]
fn test_create_git_commit() {
    create_git_commit(GitCommit::FinishCO, Some(vec!["test_co_file".to_string()])).unwrap();
}
#[test]
fn test_checkout_branch() {
    checkout_main_branch().unwrap();
}
