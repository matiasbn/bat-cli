use crate::batbelt::command_line::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::templates::finding_template::FindingTemplate;
use crate::batbelt::{
    self,
    git::{create_git_commit, GitCommit},
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
    prepare_all(true).change_context(CommandError)?;
    println!("Select the finding file to reject:");
    let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
        .change_context(CommandError)?;
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&review_files)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to rejected
        Some(index) => {
            let rejected_file_name = review_files[index].clone();
            let rejected_path = batbelt::path::get_folder_path(BatFolder::FindingsRejected, true)
                .change_context(CommandError)?;
            let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
                .change_context(CommandError)?;
            Command::new("mv")
                .args([to_review_path, rejected_path])
                .output()
                .unwrap();
            println!("{rejected_file_name} file moved to rejected");
        }
        None => println!("User did not select anything"),
    }
    Ok(())
}

pub fn accept_all() -> Result<(), CommandError> {
    prepare_all(false)?;
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
    create_git_commit(GitCommit::AcceptAllFinding, None).change_context(CommandError)?;
    println!(
        "All findings has been moved to the {} folder",
        "accepted".green()
    );
    Ok(())
}

pub fn create_finding() -> Result<(), CommandError> {
    let input_name = batbelt::bat_dialoguer::input("Finding name:").change_context(CommandError)?;
    let finding_name = input_name.to_snake_case();
    validate_config_create_finding_file(finding_name.clone())?;
    copy_template_to_findings_to_review(finding_name.clone())?;
    create_git_commit(GitCommit::StartFinding, Some(vec![finding_name.clone()]))
        .change_context(CommandError)?;
    let finding_file_path = BatFile::FindingToReview {
        file_name: finding_name,
    }
    .get_path(false)
    .change_context(CommandError)?;
    vs_code_open_file_in_current_window(finding_file_path.as_str()).change_context(CommandError)?;
    Ok(())
}

pub fn finish_finding() -> Result<(), CommandError> {
    let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
        .change_context(CommandError)?;
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    let prompt_text = "Select finding file to finish:";
    let selection = batbelt::bat_dialoguer::select(prompt_text, review_files.clone(), None)
        .change_context(CommandError)?;

    let finding_name = &review_files[selection].clone();
    let finding_file_path = batbelt::path::get_file_path(
        BatFile::FindingToReview {
            file_name: finding_name.clone(),
        },
        false,
    )
    .change_context(CommandError)?;
    validate_finished_finding_file(finding_file_path, finding_name.clone())?;
    let to_review_finding_file_path = batbelt::path::get_file_path(
        BatFile::FindingToReview {
            file_name: finding_name.clone(),
        },
        true,
    )
    .change_context(CommandError)?;
    let auditor_figures_folder_path =
        batbelt::path::get_folder_path(BatFolder::AuditorFigures, true)
            .change_context(CommandError)?;
    create_git_commit(
        GitCommit::FinishFinding {
            finding_name: finding_name.to_string(),
            to_review_finding_file_path,
            auditor_figures_folder_path,
        },
        None,
    )
    .change_context(CommandError)?;
    Ok(())
}
pub fn update_finding() -> Result<(), CommandError> {
    let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
        .change_context(CommandError)?;
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    let prompt_text = "Select finding file to update:";
    let selection = batbelt::bat_dialoguer::select(prompt_text, review_files.clone(), None)
        .change_context(CommandError)?;

    let finding_name = &review_files[selection].clone();
    let to_review_finding_file_path = batbelt::path::get_file_path(
        BatFile::FindingToReview {
            file_name: finding_name.clone(),
        },
        true,
    )
    .change_context(CommandError)?;
    let auditor_figures_folder_path =
        batbelt::path::get_folder_path(BatFolder::AuditorFigures, true)
            .change_context(CommandError)?;
    create_git_commit(
        GitCommit::UpdateFinding {
            finding_name: finding_name.to_string(),
            to_review_finding_file_path,
            auditor_figures_folder_path,
        },
        None,
    )
    .change_context(CommandError)?;
    Ok(())
}

fn prepare_all(create_commit: bool) -> Result<(), CommandError> {
    let to_review_path = batbelt::path::get_folder_path(BatFolder::FindingsToReview, true)
        .change_context(CommandError)?;
    for to_review_file in fs::read_dir(to_review_path).unwrap() {
        let file = to_review_file.unwrap();
        let file_name = file.file_name();
        if file_name.to_str().unwrap() == ".gitkeep" {
            continue;
        }
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
                let to_path = batbelt::path::get_file_path(
                    BatFile::FindingToReview {
                        file_name: finding_file_name,
                    },
                    false,
                )
                .change_context(CommandError)?;
                Command::new("mv")
                    .args([file.path().as_os_str().to_str().unwrap(), to_path.as_str()])
                    .output()
                    .unwrap();
            }
        }
    }
    if create_commit {
        create_git_commit(GitCommit::PrepareAllFinding, None).change_context(CommandError)?;
    }
    println!("All to-review findings severity tags updated");
    Ok(())
}

fn validate_config_create_finding_file(finding_name: String) -> Result<(), CommandError> {
    let finding_file_path = BatFile::FindingToReview {
        file_name: finding_name,
    }
    .get_path(false)
    .change_context(CommandError)?;
    if Path::new(&finding_file_path).is_file() {
        return Err(Report::new(CommandError).attach_printable(format!(
            "Finding file already exists: {finding_file_path:#?}"
        )));
    }
    Ok(())
}

fn copy_template_to_findings_to_review(finding_name: String) -> Result<(), CommandError> {
    let prompt_text = "is the finding an informational?";
    let is_informational =
        batbelt::bat_dialoguer::select_yes_or_no(prompt_text).change_context(CommandError)?;
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

fn validate_finished_finding_file(
    file_path: String,
    file_name: String,
) -> Result<(), CommandError> {
    let file_data = fs::read_to_string(file_path).unwrap();
    if file_data.contains("Fill the description") {
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Description section of the {file_name} file"
        )));
    }
    if file_data.contains("Fill the impact") {
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Impact section of the {file_name} file"
        )));
    }
    if file_data.contains("Add recommendations") {
        return Err(Report::new(CommandError).attach_printable(format!(
            "Please complete the Recommendations section of the {file_name} file"
        )));
    }
    Ok(())
}
