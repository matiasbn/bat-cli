use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::templates::finding_template::FindingTemplate;
use crate::batbelt::{
    git::GitCommit,
    path::{BatFile, BatFolder},
};
use colored::Colorize;
use console::Term;
use dialoguer::{console, theme::ColorfulTheme, Select};
use error_stack::{Report, Result, ResultExt};
use inflector::Inflector;
use std::{
    fs::{self, File},
    io::{self, BufRead},
    path::Path,
    process::Command,
    string::String,
};

use super::CommandError;

pub fn reject() -> Result<(), CommandError> {
    prepare_all()?;
    let to_review_files_names = BatFolder::FindingsToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;
    // get to-review files
    let selection = BatDialoguer::select(
        "Select the finding file to reject:".to_string(),
        to_review_files_names.clone(),
        None,
    )?;

    let rejected_file_name = to_review_files_names[selection].clone();
    BatFile::FindingToReview {
        file_name: rejected_file_name.clone(),
    }
    .move_file(
        &BatFile::FindingRejected {
            file_name: rejected_file_name.clone(),
        }
        .get_path(false)
        .change_context(CommandError)?,
    )
    .change_context(CommandError)?;

    GitCommit::RejectFinding {
        finding_name: rejected_file_name.clone(),
    }
    .create_commit()
    .change_context(CommandError)?;

    println!("{rejected_file_name} file moved to rejected");

    Ok(())
}

pub fn accept_all() -> Result<(), CommandError> {
    prepare_all()?;
    let accepted_path = BatFolder::FindingsAccepted
        .get_path(true)
        .change_context(CommandError)?;
    let findings_to_review_files = BatFolder::FindingsToReview
        .get_all_files_dir_entries(true, None, None)
        .change_context(CommandError)?;
    for to_review_file in findings_to_review_files {
        execute_command(
            "mv",
            &[
                to_review_file.path().to_str().unwrap(),
                &accepted_path.clone(),
            ],
        )?;
    }
    GitCommit::AcceptFindings
        .create_commit()
        .change_context(CommandError)?;
    println!(
        "All findings has been moved to the {} folder",
        "accepted".green()
    );
    Ok(())
}

pub fn start_finding() -> Result<(), CommandError> {
    let input_name =
        BatDialoguer::input("Finding name:".to_string()).change_context(CommandError)?;
    let finding_name = input_name.to_snake_case();
    validate_config_create_finding_file(finding_name.clone())?;
    copy_template_to_findings_to_review(finding_name.clone())?;

    GitCommit::StartFinding {
        finding_name: finding_name.clone(),
    }
    .create_commit()
    .change_context(CommandError)?;

    BatFile::FindingToReview {
        file_name: finding_name,
    }
    .open_in_vs_code()
    .change_context(CommandError)?;

    Ok(())
}

pub fn finish_finding() -> Result<(), CommandError> {
    let to_review_files = BatFolder::FindingsToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;
    // get to-review files
    let prompt_text = "Select finding file to finish:";
    let selection = BatDialoguer::select(prompt_text.to_string(), to_review_files.clone(), None)
        .change_context(CommandError)?;

    let finding_name = &to_review_files[selection].clone();
    validate_finished_finding_file(finding_name.clone())?;
    GitCommit::FinishFinding {
        finding_name: finding_name.to_string(),
    }
    .create_commit()
    .change_context(CommandError)?;
    Ok(())
}

pub fn update_finding() -> Result<(), CommandError> {
    let to_review_files = BatFolder::FindingsToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;

    let prompt_text = "Select finding file to update:";
    let selection = BatDialoguer::select(prompt_text.to_string(), to_review_files.clone(), None)
        .change_context(CommandError)?;

    let finding_name = to_review_files[selection].clone();
    GitCommit::UpdateFinding { finding_name }
        .create_commit()
        .change_context(CommandError)?;
    Ok(())
}

fn prepare_all() -> Result<(), CommandError> {
    let to_review_dir_entries = BatFolder::FindingsToReview
        .get_all_files_dir_entries(true, None, None)
        .change_context(CommandError)?;
    for to_review_file in to_review_dir_entries {
        let file = to_review_file;
        let file_name = file.file_name();
        let mut file_name_tokenized = file_name
            .to_str()
            .unwrap()
            .to_string()
            .split('-')
            .map(|token| token.to_string())
            .collect::<Vec<String>>();
        let severity_flags = ["1", "2", "3", "4"];
        let finding_name = if severity_flags.contains(&file_name_tokenized[0].as_str()) {
            file_name_tokenized.remove(0);
            file_name_tokenized.join("-")
        } else {
            file_name_tokenized.join("-")
        };
        let open_file = File::open(file.path()).unwrap();
        let file_lines = io::BufReader::new(open_file).lines().map(|l| l.unwrap());
        for line in file_lines {
            if line.contains("Severity:") {
                let file_severity = line
                    .replace("**Severity:**", "")
                    .replace(' ', "")
                    .to_lowercase();
                let severity = match file_severity.as_str() {
                    "high" => "1",
                    "medium" => "2",
                    "low" => "3",
                    "informational" => "4",
                    &_ => {
                        return Err(Report::new(CommandError).attach_printable(format!(
                            "severity: {:?} not recongnized in file {:?}",
                            file_severity,
                            file.path()
                        )));
                    }
                };
                let finding_file_name = format!(
                    "{}-{}",
                    severity.to_string(),
                    finding_name.replace(".md", "").as_str()
                );
                // let to_path = utils::path::get_auditor_findings_to_review_path(Some())?;
                let to_path = BatFile::FindingToReview {
                    file_name: finding_file_name,
                }
                .get_path(false)
                .change_context(CommandError)?;
                execute_command(
                    "mv",
                    &[file.path().as_os_str().to_str().unwrap(), to_path.as_str()],
                )?;
            }
        }
    }
    println!("All to-review findings severity tags updated");
    Ok(())
}

fn validate_config_create_finding_file(finding_name: String) -> Result<(), CommandError> {
    let bat_file = BatFile::FindingToReview {
        file_name: finding_name,
    };

    if bat_file.file_exists().change_context(CommandError)? {
        return Err(Report::new(CommandError).attach_printable(format!(
            "Finding file already exists: {:#?}",
            bat_file.get_path(false).change_context(CommandError)?
        )));
    }
    Ok(())
}

fn copy_template_to_findings_to_review(finding_name: String) -> Result<(), CommandError> {
    let prompt_text = "is the finding an informational?";
    let is_informational =
        BatDialoguer::select_yes_or_no(prompt_text.to_string()).change_context(CommandError)?;
    FindingTemplate::new_finding_file(&finding_name, is_informational)
        .change_context(CommandError)?;
    let finding_path = BatFile::FindingToReview {
        file_name: finding_name,
    }
    .get_path(false)
    .change_context(CommandError)?;
    println!("Finding file successfully created at: {}", finding_path);
    Ok(())
}

fn validate_finished_finding_file(file_name: String) -> Result<(), CommandError> {
    let bat_file = BatFile::FindingToReview {
        file_name: file_name.clone(),
    };
    let file_data = bat_file.read_content(true).change_context(CommandError)?;
    if file_data.contains("Fill the description") {
        bat_file.open_in_vs_code().change_context(CommandError)?;
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Description section of the {} file",
            file_name.clone()
        )));
    }
    if file_data.contains("Fill the impact") {
        bat_file.open_in_vs_code().change_context(CommandError)?;
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Impact section of the {} file",
            file_name.clone()
        )));
    }
    if file_data.contains("Add recommendations") {
        bat_file.open_in_vs_code().change_context(CommandError)?;
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Recommendations section of the {} file",
            file_name.clone()
        )));
    }
    Ok(())
}
