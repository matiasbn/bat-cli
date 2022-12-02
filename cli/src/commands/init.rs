use std::borrow::BorrowMut;
use std::fs::File;
use std::io::BufRead;
use std::process::Command;
use std::{io, string::String};

use crate::config::{BatConfig, InitConfigValidation};

use super::code_overhaul::create_overhaul_file;

pub fn initialize_notes_repo() {
    let bat_config: BatConfig = BatConfig::get_config();
    println!("creating repository for the next config: ");
    println!("{:#?}", bat_config);
    let required = bat_config.required;
    BatConfig::validate_init_config();
    create_notes_repository(required.clone().audit_folder_path);
    // copy templates/notes-folder-template
    create_auditors_notes_folders(required.audit_folder_path.clone(), required.auditor_names);
    // create overhaul files
    initialize_code_overhaul_files()
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

fn initialize_code_overhaul_files() {
    let bat_config = BatConfig::get_config().required;
    let program_lib_path = bat_config.program_lib_path;
    let auditor_names = bat_config.auditor_names;

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

    for auditor_name in auditor_names {
        for entrypoint_name in entrypoints_names.clone() {
            create_overhaul_file(entrypoint_name.clone(), auditor_name.clone());
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
