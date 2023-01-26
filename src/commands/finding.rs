use colored::Colorize;
use console::Term;
use dialoguer::{console, theme::ColorfulTheme, Input, Select};
use std::{
    fs::{self, File},
    io::{self, BufRead},
    path::Path,
    process::Command,
    string::String,
};

use crate::{
    command_line::vs_code_open_file_in_current_window,
    utils::{
        self,
        git::{create_git_commit, GitCommit},
        helpers::get::get_only_files_from_folder,
        path::{FilePathType, FolderPathType},
    },
};

pub fn reject() -> Result<(), String> {
    prepare_all(true)?;
    println!("Select the finding file to reject:");
    let to_review_path = utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
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
            let rejected_path =
                utils::path::get_folder_path(FolderPathType::FindingsRejected, true);
            let to_review_path =
                utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
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

pub fn accept_all() -> Result<(), String> {
    prepare_all(false)?;
    // let to_review_path = utils::path::get_auditor_findings_to_review_path(None)?;
    let to_review_path = utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
    // let accepted_path = utils::path::get_auditor_findings_accepted_path(None)?;
    let accepted_path = utils::path::get_folder_path(FolderPathType::FindingsAccepted, true);
    let findings_to_review_files_info = get_only_files_from_folder(to_review_path)?;
    for to_review_file in findings_to_review_files_info {
        let mut output = Command::new("mv")
            .args([to_review_file.path, accepted_path.clone()])
            .spawn()
            .unwrap();
        output.wait().unwrap();
    }
    create_git_commit(GitCommit::AcceptAllFinding, None)?;
    println!(
        "All findings has been moved to the {} folder",
        "accepted".green()
    );
    Ok(())
}

pub fn create_finding() -> Result<(), String> {
    let mut finding_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Finding name:")
        .interact_text()
        .unwrap();
    finding_name = finding_name.replace('-', "_").replace(' ', "_");
    validate_config_create_finding_file(finding_name.clone())?;
    copy_template_to_findings_to_review(finding_name.clone())?;
    create_git_commit(GitCommit::StartFinding, Some(vec![finding_name.clone()]))?;
    // let finding_file_path = utils::path::get_auditor_findings_to_review_path(Some(finding_name))?;
    let finding_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name,
        },
        false,
    );
    vs_code_open_file_in_current_window(finding_file_path.as_str())?;
    Ok(())
}

pub fn finish_finding() -> Result<(), String> {
    let to_review_path = utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    let prompt_text = "Select finding file to finish:";
    let selection = utils::cli_inputs::select(prompt_text, review_files.clone(), None)?;

    let finding_name = &review_files[selection].clone();
    let finding_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name.clone(),
        },
        false,
    );
    validate_finished_finding_file(finding_file_path, finding_name.clone());
    let to_review_finding_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name.clone(),
        },
        true,
    );
    let auditor_figures_folder_path =
        utils::path::get_folder_path(FolderPathType::AuditorFigures, true);
    create_git_commit(
        GitCommit::FinishFinding {
            finding_name: finding_name.to_string(),
            to_review_finding_file_path,
            auditor_figures_folder_path,
        },
        None,
    )?;
    Ok(())
}
pub fn update_finding() -> Result<(), String> {
    let to_review_path = utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    let prompt_text = "Select finding file to update:";
    let selection = utils::cli_inputs::select(prompt_text, review_files.clone(), None)?;

    let finding_name = &review_files[selection].clone();
    let to_review_finding_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name.clone(),
        },
        true,
    );
    let auditor_figures_folder_path =
        utils::path::get_folder_path(FolderPathType::AuditorFigures, true);
    create_git_commit(
        GitCommit::UpdateFinding {
            finding_name: finding_name.to_string(),
            to_review_finding_file_path,
            auditor_figures_folder_path,
        },
        None,
    )?;
    Ok(())
}

fn prepare_all(create_commit: bool) -> Result<(), String> {
    let to_review_path = utils::path::get_folder_path(FolderPathType::FindingsToReview, true);
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
                    &_ => panic!(
                        "severity: {:?} not recongnized in file {:?}",
                        file_severity,
                        file.path()
                    ),
                };
                let finding_file_name = format!(
                    "{}-{}",
                    severity.to_string(),
                    finding_name.replace(".md", "").as_str()
                );
                // let to_path = utils::path::get_auditor_findings_to_review_path(Some())?;
                let to_path = utils::path::get_file_path(
                    FilePathType::FindingToReview {
                        file_name: finding_file_name,
                    },
                    false,
                );
                Command::new("mv")
                    .args([file.path().as_os_str().to_str().unwrap(), to_path.as_str()])
                    .output()
                    .unwrap();
            }
        }
    }
    if create_commit {
        create_git_commit(GitCommit::PrepareAllFinding, None)?;
    }
    println!("All to-review findings severity tags updated");
    Ok(())
}

fn validate_config_create_finding_file(finding_name: String) -> Result<(), String> {
    let finding_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name,
        },
        false,
    );
    if Path::new(&finding_file_path).is_file() {
        panic!("Finding file already exists: {finding_file_path:#?}");
    }
    Ok(())
}

fn copy_template_to_findings_to_review(finding_name: String) -> Result<(), String> {
    let options = vec!["yes", "no"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&options)
        .default(0)
        .with_prompt("is the finding an informational?")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    let template_path = if selection.unwrap() == 0 {
        // utils::path::get_informational_template_path()?
        utils::path::get_file_path(FilePathType::TemplateInformational, true)
    } else {
        utils::path::get_file_path(FilePathType::TemplateFinding, true)
    };
    // let new_file_path = utils::path::get_auditor_findings_to_review_path(Some(finding_name))?;
    let new_file_path = utils::path::get_file_path(
        FilePathType::FindingToReview {
            file_name: finding_name,
        },
        false,
    );
    let output = Command::new("cp")
        .args([template_path, new_file_path.clone()])
        .output()
        .unwrap()
        .status
        .exit_ok();
    if let Err(output) = output {
        panic!("Finding creation failed with reason: {output:#?}")
    };
    println!("Finding file successfully created at: {new_file_path:?}");
    Ok(())
}

fn validate_finished_finding_file(file_path: String, file_name: String) {
    let file_data = fs::read_to_string(file_path).unwrap();
    if file_data.contains("## Finding name") {
        panic!("Please update the Finding name of the {file_name} file");
    }
    if file_data.contains("Fill the description") {
        panic!("Please complete the Description section of the {file_name} file");
    }
    if file_data.contains("Fill the impact") {
        panic!("Please complete the Impact section of the {file_name} file");
    }
    if file_data.contains("Add recommendations") {
        panic!("Please complete the Recommendations section of the {file_name} file");
    }
}
