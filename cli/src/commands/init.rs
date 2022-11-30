use std::{fs, path::Path, string::String};

use crate::config::BatmanConfig;
use crate::{commands, utils::get_config};

pub fn initialize_notes_repo(config_file_path: Option<String>) {
    let batman_config: BatmanConfig = get_config();
    println!("{:#?}", batman_config);
    // check audit folder exist
    if Path::new(&batman_config.path.audit_folder_path).is_dir() {
        panic!(
            "audit folder {:?} already exists, abortings",
            &batman_config.path.audit_folder_path
        );
    }

    // check auditors folders exist
    for auditor_name in &batman_config.init.auditors_names {
        let auditor_folder_path =
            batman_config.path.audit_folder_path.clone() + "/".as_ref() + &auditor_name + "-notes";
        if !Path::new(&auditor_folder_path).is_dir() {
            panic!(
                "templates folder {:?} does not exist, aborting",
                &batman_config.path.audit_folder_path
            );
        }
    }
    // check program_path folder exist
    if batman_config.path.program_path.is_empty()
        || !Path::new(&batman_config.path.program_path).is_dir()
    {
        panic!(
            "program folder {:?} does not exist, aborting, please update Batman.toml file",
            &batman_config.path.program_path
        );
    }
    // check program_entrypoints_path folder exist
    for entrypoint_path in &batman_config.path.program_entrypoints_path {
        if entrypoint_path.is_empty() || !Path::new(&entrypoint_path).is_dir() {
            panic!(
                "entrypoint folder {:?} does not exist, aborting, please update Batman.toml file",
                &entrypoint_path
            );
        }
    }
    // copy templates/notes-folder-template
    println!("creating repository for the next config: ");
    println!("{:?}", batman_config);
    println!("creating files for entrypoints: ");
    // initialize_code_overhaul_files(
    //     batman_config.path.program_entrypoints_path,
    //     Some(batman_config.path.audit_folder_path.clone()),
    // )
}

fn initialize_code_overhaul_files(
    program_entrypoints_path: Vec<String>,
    audit_folder_path: Option<String>,
) {
    for entrypoint_path in program_entrypoints_path.clone() {
        for entrypoint_file in fs::read_dir(entrypoint_path).unwrap() {
            let file_name_str = entrypoint_file.unwrap().file_name();
            let file_name = file_name_str
                .to_str()
                .unwrap()
                .split(".rs")
                .collect::<Vec<&str>>()[0];
            if (file_name != "mod") {
                commands::code_overhaul::create_overhaul_file(
                    String::from(file_name),
                    audit_folder_path.clone(),
                )
            }
        }
    }
}
