use std::borrow::BorrowMut;
use std::fs::{self, File};
use std::io::BufRead;
use std::path::Path;
use std::process::Command;
use std::{io, string::String};

use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use super::code_overhaul::create_overhaul_file;
use super::create::AUDITOR_TOML_INITIAL_PATH;
use crate::config::{BatConfig, RequiredConfig, AUDITOR_TOML_INITIAL_CONFIG_STR};

pub fn initialize_bat_project() {
    let bat_config: BatConfig = BatConfig::get_init_config();
    let BatConfig { required, auditor } = bat_config.clone();
    // if auditor.auditor is empty, prompt name
    if auditor.auditor_name.is_empty() {
        let auditor_name = get_auditor_name(required.auditor_names.clone());
        println!("Is great to have you here {}!", auditor_name);
        update_auditor_toml(auditor_name);
    }
    println!("creating project for the next config: ");
    println!("{:#?}", bat_config);
    validate_init_config();
    // copy templates/notes-folder-template
    create_auditor_notes_folder();
    // create overhaul files
    initialize_code_overhaul_files();

    if !Path::new(".git").is_dir() {
        println!("Initializing project repository");
        initialize_project_repository();
        println!("Project repository successfully initialized");
    } else {
        println!("Project repository already initialized");
    }
}

fn get_auditor_name(auditor_names: Vec<String>) -> String {
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select your name")
        .default(0)
        .items(&auditor_names[..])
        .interact()
        .unwrap();
    let auditor_name = auditor_names[selection].clone();
    auditor_name.clone()
}

fn update_auditor_toml(auditor_name: String) {
    let new_auditor_file_content = AUDITOR_TOML_INITIAL_CONFIG_STR.replace(
        "auditor_name = \"",
        ("auditor_name = \"".to_string() + &auditor_name).as_str(),
    );
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);
    fs::write(auditor_toml_path.clone(), new_auditor_file_content)
        .expect("Could not write to file!");
}

fn validate_init_config() {
    // audit notes folder should not exist
    let BatConfig { required, .. } = BatConfig::get_validated_config();
    let auditor_folder_path = BatConfig::get_auditor_notes_path();
    if Path::new(&auditor_folder_path).is_dir() {
        panic!("auditor folder {:?} already exist", &auditor_folder_path);
    }
    if !Path::new(&required.program_lib_path).is_file() {
        panic!(
            "program file at path \"{:?}\" does not exist, please update Bat.toml file",
            &required.program_lib_path
        );
    }
}

fn initialize_project_repository() {
    let BatConfig { required, .. } = BatConfig::get_validated_config();
    let RequiredConfig {
        project_repository_url,
        auditor_names,
        ..
    } = required;
    // git init
    let mut output = Command::new("git").args(["init"]).output().unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Adding project repository as remote");
    // git remote add origin project_repository
    output = Command::new("git")
        .args(["remote", "add", "origin", project_repository_url.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Commit all to main");
    output = Command::new("git").args(["add", "-A"]).output().unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    output = Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Creating develop branch");
    // create develop
    output = Command::new("git")
        .args(["checkout", "-b", "develop"])
        .output()
        .unwrap();

    // create auditors branches from develop
    for auditor_name in auditor_names {
        println!("Creating branch for {:?}", auditor_name);
        output = Command::new("git")
            .args(["checkout", "-b", (auditor_name + "-notes").as_str()])
            .output()
            .unwrap();
        output = Command::new("git")
            .args(["checkout", "develop"])
            .output()
            .unwrap();
    }

    println!("Pushing all branches to origin");
    // push all branches to remote
    output = Command::new("git")
        .args(["push", "origin", "--all"])
        .output()
        .unwrap();

    println!("Checking out {:?}'s branch", BatConfig::get_auditor_name());
    // checkout auditor branch
    output = Command::new("git")
        .args([
            "checkout",
            (BatConfig::get_auditor_name() + "-notes").as_str(),
        ])
        .output()
        .unwrap();
}

fn create_auditor_notes_folder() {
    let dest_path = BatConfig::get_auditor_notes_path();
    println!("creating {}", dest_path);

    let mut output = Command::new("cp")
        .args([
            "-r",
            BatConfig::get_notes_folder_template_path().as_str(),
            BatConfig::get_notes_path().as_str(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "initialize project repository failed with error: {:#?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    output = Command::new("mv")
        .current_dir(BatConfig::get_notes_path())
        .args([
            "notes-folder-template",
            (BatConfig::get_auditor_name() + "-notes").as_str(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "create auditor notes folder failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
}

fn initialize_code_overhaul_files() {
    let BatConfig { required, .. } = BatConfig::get_validated_config();
    let RequiredConfig {
        program_lib_path, ..
    } = required;

    let lib_file = File::open(program_lib_path).unwrap();
    let mut lib_files_lines = io::BufReader::new(lib_file).lines().map(|l| l.unwrap());
    lib_files_lines
        .borrow_mut()
        .enumerate()
        .find(|(_, line)| *line == String::from("#[program]"))
        .unwrap();

    let mut program_lines = vec![String::from(""); 0];
    for (_, line) in lib_files_lines.borrow_mut().enumerate() {
        if line == "}" {
            break;
        }
        program_lines.push(line)
    }

    let entrypoints_names = program_lines
        .iter()
        .filter(|line| line.contains("pub fn"))
        .map(|line| line.replace("pub fn ", "").replace("<'info>", ""))
        .map(|line| String::from(line.split('(').collect::<Vec<&str>>()[0]))
        .map(|line| String::from(line.split_whitespace().collect::<Vec<&str>>()[0]))
        .collect::<Vec<String>>();

    for entrypoint_name in entrypoints_names.clone() {
        create_overhaul_file(entrypoint_name.clone());
    }
}

// fn get_context_names() {
//     let context_names = program_lines
//         .iter()
//         .filter(|line| line.contains("Context<"))
//         .map(|line| {
//             line.replace("pub fn ", "")
//                 .replace("<'info>", "")
//                 .replace("'info, ", "")
//                 .replace("'_, ", "")
//         })
//         .map(|line| {
//             let new_line = if line.contains("(") {
//                 let new_line = String::from(line.split("(").collect::<Vec<&str>>()[1]);
//                 String::from(new_line.split(",").collect::<Vec<&str>>()[0])
//             } else {
//                 line
//             };
//             new_line.split_whitespace().collect::<Vec<&str>>()[1]
//                 .to_string()
//                 .replace(">,", "")
//                 .replace("Context<", "")
//         })
//         .collect::<Vec<String>>();
// }
