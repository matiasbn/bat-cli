use crate::config::BatConfig;

use std::process::Command;
use std::string::String;

pub fn create_overhaul_file(entrypoint_name: String, auditor_name: String) {
    let audit_folder_path = BatConfig::get_audit_folder_path();
    let output = Command::new("cp")
        .args([
            "-r",
            (audit_folder_path.clone() + "/templates/code-overhaul.md").as_str(),
            (audit_folder_path.clone()
                + "/notes/"
                + &auditor_name.clone()
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
