use std::{fs, path::Path, string::String};

use crate::{
    commands,
    utils::{get_sam_config, SamConfig},
};

pub fn initialize_notes_repo(config_file_path: Option<String>) {
    let sam_config: SamConfig = get_sam_config();
    println!("{:#?}", sam_config);
    // check audit folder exist
    // if Path::new(&sam_config.path.audit_folder_path).is_dir() {
    //     panic!(
    //         "audit folder {:?} already exists, abortings",
    //         &sam_config.path.audit_folder_path
    //     );
    // }
    // check auditor folders exist
    for auditor_name in &sam_config.init.auditors {
        if !Path::new(&sam_config.path.audit_folder_path + &auditor_name).is_dir() {
            panic!(
                "templates folder {:?} does not exist, aborting",
                &sam_config.path.audit_folder_path
            );
        }
    }
    if !Path::new(&sam_config.path.templates_path).is_dir() {
        panic!(
            "templates folder {:?} does not exist, aborting",
            &sam_config.path.audit_folder_path
        );
    }
    // check templates folder exist
    if !Path::new(&sam_config.path.templates_path).is_dir() {
        panic!(
            "templates folder {:?} does not exist, aborting",
            &sam_config.path.audit_folder_path
        );
    }
    // check program_path folder exist
    if sam_config.path.program_path.is_empty() || !Path::new(&sam_config.path.program_path).is_dir()
    {
        panic!(
            "program folder {:?} does not exist, aborting, please update BAT.toml file",
            &sam_config.path.program_path
        );
    }
    // check program_entrypoints_path folder exist
    for entrypoint_path in &sam_config.path.program_entrypoints_path {
        if entrypoint_path.is_empty() || !Path::new(&entrypoint_path).is_dir() {
            panic!(
                "entrypoint folder {:?} does not exist, aborting, please update BAT.toml file",
                &entrypoint_path
            );
        }
    }
    // copy templates/notes-folder-template
    println!("creating repository for the next config: ");
    println!("{:?}", sam_config);
    println!("creating files for entrypoints: ");
    initialize_code_overhaul_files(
        sam_config.path.program_entrypoints_path,
        Some(sam_config.path.audit_folder_path.clone()),
    )
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
