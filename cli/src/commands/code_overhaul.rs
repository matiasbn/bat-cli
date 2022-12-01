use crate::config::BatConfig;
use crate::utils::get_branch_name;

use std::path::Path;
use std::process::Command;
use std::str;
use std::string::String;

pub fn create_overhaul_file(entrypoint: String, audit_repo_path: Option<String>) {
    let audit_repo_path = BatConfig::get_audit_folder_path();
    let branch_name = get_branch_name(audit_repo_path.clone());
    // let file_path = get_overhaul_file_path(audit_repo_path, entrypoint);
    // create_code_overhaul_file(file_path);
}

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
