use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::config::BatConfig;
use crate::utils::{check_correct_branch, create_git_commit, GitCommit};

use std::path::Path;
use std::process::Command;
use std::string::String;
use std::{error, fs};

pub fn create_overhaul_file(entrypoint_name: String) {
    let code_overhaul_auditor_file_path =
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(entrypoint_name.clone()));
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        panic!(
            "code overhaul file already exists for: {:?}",
            entrypoint_name
        );
    }
    let output = Command::new("cp")
        .args([
            "-r",
            BatConfig::get_code_overhaul_template_path().as_str(),
            code_overhaul_auditor_file_path.as_str(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "create auditors note folder failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    println!("code-overhaul file created: {:?}.md", entrypoint_name);
}

pub fn start_code_overhaul_file() {
    print!("Select the code-overhaul file to start:");
    // check if program_lib_path is not empty or panic
    let BatConfig { optional, .. } = BatConfig::get_validated_config();
    if optional.program_instructions_path.is_empty() {
        panic!("Optional program_instructions_path parameter not set in Bat.toml")
    }

    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None);
    // get to-review files
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if review_files.is_empty() {
        panic!("no to-review files in code-overhaul folder");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&review_files)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to rejected
        Some(index) => {
            let started_file_name = review_files[index].clone();
            println!("{}", started_file_name);
            let started_path =
                BatConfig::get_auditor_code_overhaul_started_path(Some(started_file_name.clone()));
            let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(Some(
                started_file_name.clone(),
            ));
            check_correct_branch();
            Command::new("mv")
                .args([to_review_path, started_path])
                .output()
                .unwrap();
            println!("{} file moved to started", started_file_name);
            create_git_commit(GitCommit::StartCO, Some(vec![started_file_name]));
        }
        None => println!("User did not select anything"),
    }
}

pub fn finish_code_overhaul_file() {
    println!("Select the code-overhaul file to finish:");
    let started_path = BatConfig::get_auditor_code_overhaul_started_path(None);
    // get to-review files
    let started_files = fs::read_dir(started_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if started_files.is_empty() {
        panic!("no started files in code-overhaul folder");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&started_files)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to rejected
        Some(index) => {
            let finished_file_name = started_files[index].clone();
            let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(Some(
                finished_file_name.clone(),
            ));
            let started_path =
                BatConfig::get_auditor_code_overhaul_started_path(Some(finished_file_name.clone()));
            check_correct_branch();
            Command::new("mv")
                .args([started_path, finished_path])
                .output()
                .unwrap();
            println!("{} file moved to finished", finished_file_name);
            create_git_commit(GitCommit::FinishCO, Some(vec![finished_file_name]));
        }
        None => println!("User did not select anything"),
    }
}

// fn create

// fn get_overhaul_file_path(audit_repo_path: String, entrypoint: String) -> String {
//     let code_overhaul_path =
//         audit_repo_path + &"/notes/".to_string() + &branch_name + &"/code-overhaul/".to_string();
//     code_overhaul_path
// }

// fn create_code_overhaul_file(entrypoint: String, file_path: String) -> Result<(), ()> {
//     if !Path::new(&code_overhaul_path).exists() {
//         panic!(
//             "{:?} auditor folder does not exist, aborting",
//             code_overhaul_path
//         )
//     };

//     let full_overhaul_path =
//         code_overhaul_path + &String::from("/code-overhaul/") + &entrypoint + &String::from(".md");
//     if Path::new(&full_overhaul_path).exists() {
//         panic!("{:?} file already exist, aborting", entrypoint)
//     };
//     Command::new("cp")
//         .args([CODE_OVERHAUL_TEMPLATE_PATH, &full_overhaul_path])
//         .output();
//     println!("Creating {:?} file", entrypoint);
//     Ok(())
// }
