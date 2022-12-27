use console::Term;
use dialoguer::{console, theme::ColorfulTheme, Input, Select};
use std::{
    fs::{self, File},
    io::{self, BufRead},
    path::{Path, PathBuf},
    process::Command,
    string::String,
};

use crate::{
    command_line::vs_code_open_file_in_current_window,
    config::BatConfig,
    git::{create_git_commit, GitCommit},
};

pub fn reject() {
    prepare_all();
    println!("Select the finding file to reject:");
    let to_review_path = BatConfig::get_auditor_findings_to_review_path(None);
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
                BatConfig::get_auditor_findings_rejected_path(Some(rejected_file_name.clone()));
            let to_review_path =
                BatConfig::get_auditor_findings_to_review_path(Some(rejected_file_name.clone()));
            Command::new("mv")
                .args([to_review_path, rejected_path])
                .output()
                .unwrap();
            println!("{} file moved to rejected", rejected_file_name);
        }
        None => println!("User did not select anything"),
    }
}

pub fn accept_all() {
    prepare_all();
    let to_review_path = BatConfig::get_auditor_findings_to_review_path(None);
    let accepted_path = BatConfig::get_auditor_findings_accepted_path(None);
    for file_result in fs::read_dir(&to_review_path).unwrap() {
        let file_name = file_result.unwrap().file_name();
        if file_name != ".gitkeep" {
            Command::new("mv")
                .args([
                    to_review_path.clone() + file_name.to_str().unwrap(),
                    accepted_path.clone(),
                ])
                .output()
                .unwrap();
        }
    }
    println!("All files has been moved to the accepted folder");
}

pub fn create_finding() {
    let mut finding_name: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Finding name:")
        .interact_text()
        .unwrap();
    finding_name = finding_name.replace("-", "_");
    validate_config_create_finding_file(finding_name.clone());
    copy_template_to_findings_to_review(finding_name.clone());
    create_git_commit(GitCommit::StartFinding, Some(vec![finding_name.clone()]));
    let finding_file_path = BatConfig::get_auditor_findings_to_review_path(Some(finding_name));
    vs_code_open_file_in_current_window(finding_file_path)
}

pub fn finish_finding() {
    let to_review_path = BatConfig::get_auditor_findings_to_review_path(None);
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&review_files)
        .with_prompt("Select finding file:")
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to rejected
        Some(index) => {
            let finding_name = review_files[index].clone();
            validate_config_create_finding_file(finding_name.clone());
            create_git_commit(GitCommit::FinishFinding, Some(vec![finding_name]))
        }
        None => println!("User did not select anything"),
    }
}

pub fn prepare_all() {
    let to_review_path = BatConfig::get_auditor_findings_to_review_path(None);
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
                Command::new("mv")
                    .args([
                        file.path(),
                        PathBuf::from(BatConfig::get_auditor_findings_to_review_path(Some(
                            severity.to_string() + "-" + finding_name.replace(".md", "").as_str(),
                        ))),
                    ])
                    .output()
                    .unwrap();
            }
        }
    }
    create_git_commit(GitCommit::PrepareAllFinding, None);
    println!("All to-review findings severity tags updated")
}

// create_finding_file
fn validate_config_create_finding_file(finding_name: String) {
    let findings_to_review_path = BatConfig::get_auditor_findings_to_review_path(None);
    // check auditor/findings/to_review folder exists
    if !Path::new(&findings_to_review_path).is_dir() {
        panic!("Folder not found: {:#?}", findings_to_review_path);
    }
    // check if file exists in to_review
    let finding_file_path = findings_to_review_path + &finding_name + ".md";
    if Path::new(&finding_file_path).is_file() {
        panic!("Finding file already exists: {:#?}", finding_file_path);
    }
}

fn copy_template_to_findings_to_review(finding_name: String) {
    println!("is the finding an informational?");
    let options = vec!["yes", "no"];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&options)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    let template_path = if selection.unwrap() == 0 {
        BatConfig::get_informational_template_path()
    } else {
        BatConfig::get_finding_template_path()
    };
    let new_file_path = BatConfig::get_auditor_findings_to_review_path(Some(finding_name));
    let output = Command::new("cp")
        .args([template_path, new_file_path.clone()])
        .output()
        .unwrap()
        .status
        .exit_ok();
    if let Err(output) = output {
        panic!("Finding creation failed with reason: {:#?}", output)
    };
    println!("Finding file successfully created at: {:?}", new_file_path);
}
