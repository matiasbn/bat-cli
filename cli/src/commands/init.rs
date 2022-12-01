use std::borrow::{Borrow, BorrowMut};
use std::fs::File;
use std::io::{BufRead, Split};
use std::process::{Command, Output};
use std::{io, path::Path, string::String};

use crate::config::{BatConfig, RequiredConfig};

pub fn initialize_notes_repo() {
    let bat_config: BatConfig = BatConfig::get_config();
    println!("creating repository for the next config: ");
    println!("{:#?}", bat_config.clone());
    let required = bat_config.required;
    validate_initial_config(required.clone()).unwrap();
    create_notes_repository(required.clone().audit_folder_path);
    // copy templates/notes-folder-template
    create_auditors_notes_folders(
        required.audit_folder_path.clone(),
        required.auditor_names.clone(),
    );
    // create overhaul files
    initialize_code_overhaul_files(
        required.program_lib_path.clone(),
        required.audit_folder_path.clone(),
        required.auditor_names.clone(),
    )
}

fn create_notes_repository(audit_folder_path: String) {
    let output = Command::new("cp")
        .args(["-r", "../base-repository", audit_folder_path.as_str()])
        .output()
        .unwrap()
        .status
        .exit_ok();
    if let Err(output) = output {
        panic!("create notes repository failed with error: {:?}", output)
    };
}

fn create_auditors_notes_folders(audit_folder_path: String, auditor_names: Vec<String>) {
    for auditor in auditor_names {
        let output = Command::new("cp")
            .args([
                "-r",
                (audit_folder_path.clone() + "/templates/notes-folder-template").as_str(),
                (audit_folder_path.clone() + "/notes/" + &auditor + "-notes").as_str(),
            ])
            .output()
            .unwrap()
            .status
            .exit_ok();
        if let Err(output) = output {
            panic!(
                "create auditors notes folders failed with error: {:?}",
                output
            )
        };
    }
}

fn initialize_code_overhaul_files(
    program_lib_path: String,
    audit_folder_path: String,
    auditor_names: Vec<String>,
) {
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
        .map(|line| String::from(line.split("(").collect::<Vec<&str>>()[0]))
        .map(|line| String::from(line.split_whitespace().collect::<Vec<&str>>()[0]))
        .collect::<Vec<String>>();

    let context_names = program_lines
        .iter()
        .filter(|line| line.contains("Context<"))
        .map(|line| {
            line.replace("pub fn ", "")
                .replace("<'info>", "")
                .replace("'info, ", "")
                .replace("'_, ", "")
        })
        .map(|line| {
            let new_line = if line.contains("(") {
                let new_line = String::from(line.split("(").collect::<Vec<&str>>()[1]);
                String::from(new_line.split(",").collect::<Vec<&str>>()[0])
            } else {
                line
            };
            new_line.split_whitespace().collect::<Vec<&str>>()[1]
                .to_string()
                .replace(">,", "")
                .replace("Context<", "")
        })
        .collect::<Vec<String>>();

    for auditor in auditor_names {
        for entrypoint_name in entrypoints_names.clone() {
            let output = Command::new("cp")
                .args([
                    "-r",
                    (audit_folder_path.clone() + "/templates/code-overhaul.md").as_str(),
                    (audit_folder_path.clone()
                        + "/notes/"
                        + &auditor.clone()
                        + "-notes/code-overhaul/to-review/"
                        + &entrypoint_name.clone()
                        + ".md")
                        .as_str(),
                ])
                .output()
                .unwrap()
                .status
                .exit_ok();
            if let Err(output) = output {
                panic!("create code overhaul files failed with error: {:?}", output)
            };
        }
    }
    // for entrypoint_path in program_entrypoints_path.clone() {
    //     for entrypoint_file in fs::read_dir(entrypoint_path).unwrap() {
    //         let file_name_str = entrypoint_file.unwrap().file_name();
    //         let file_name = file_name_str
    //             .to_str()
    //             .unwrap()
    //             .split(".rs")
    //             .collect::<Vec<&str>>()[0];
    //         if file_name != "mod" {
    //             commands::code_overhaul::create_overhaul_file(
    //                 String::from(file_name),
    //                 audit_folder_path.clone(),
    //             )
    //         }
    //     }
    // }
}

fn validate_initial_config(bat_config: RequiredConfig) -> Result<String, String> {
    // audit notes folder should not exist
    if Path::new(&bat_config.audit_folder_path).is_dir() {
        panic!(
            "audit folder {:?} already exists, abortings",
            &bat_config.audit_folder_path
        );
    }

    // auditors notes folders should not exist and not empty
    if bat_config.auditor_names.is_empty() {
        panic!("required parameter auditors_names is empty in Bat.toml file, aborting",);
    }
    for auditor_name in &bat_config.auditor_names {
        let auditor_folder_path =
            bat_config.audit_folder_path.clone() + "/".as_ref() + &auditor_name + "-notes";
        if Path::new(&auditor_folder_path).is_dir() {
            panic!(
                "auditor folder {:?} already exist, aborting",
                &auditor_folder_path
            );
        }
    }
    // program_path not empty and program_path exists
    if bat_config.program_lib_path.is_empty() {
        panic!("required parameter program_path is empty in Bat.toml file, aborting",);
    } else if !Path::new(&bat_config.program_lib_path).is_file() {
        panic!(
            "program file at path \"{:?}\" does not exist, aborting, please update Bat.toml file",
            &bat_config.program_lib_path
        );
    }
    Ok(String::from("Ok"))
}

#[test]
fn test_create_notes_repository() {
    let bat_config = BatConfig::get_test_config().required;
    create_notes_repository(bat_config.audit_folder_path)
}

#[test]
fn test_create_auditors_notes_folders() {
    let bat_config = BatConfig::get_test_config().required;
    create_auditors_notes_folders(bat_config.audit_folder_path, bat_config.auditor_names)
}
#[test]
fn test_initialize_code_overhaul_files() {
    let bat_config = BatConfig::get_test_config().required;
    initialize_code_overhaul_files(
        bat_config.program_lib_path,
        bat_config.audit_folder_path,
        bat_config.auditor_names,
    )
}
