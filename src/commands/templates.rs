use std::{fs, process::Command};

use crate::{
    config::BatConfig,
    constants::BASE_REPOSTORY_NAME,
    git::{clone_base_repository, create_git_commit, GitCommit},
};

pub fn update_templates() {
    let BatConfig { required: _, .. } = BatConfig::get_validated_config();

    // clone base repository
    println!("Cloning base repository");
    clone_base_repository();

    // delete templates folder
    let templates_path = BatConfig::get_templates_path();
    let output = Command::new("rm")
        .args(["-rf", templates_path.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    // move template to now location
    let output = Command::new("mv")
        .args([
            BASE_REPOSTORY_NAME.to_string() + "/templates",
            templates_path,
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    // delete base_repository cloned
    let output = Command::new("rm")
        .args(["-rf", BASE_REPOSTORY_NAME])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    println!("Updating to-review files in code-overhaul folder");
    // move new templates to to-review in the auditor notes folder
    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None);
    // if the auditor to-review code overhaul folder exists
    if fs::read_dir(to_review_path.clone()).is_ok() {
        let to_review_folder = fs::read_dir(to_review_path).unwrap();
        for file in to_review_folder {
            let file_name = file.unwrap().file_name().into_string().unwrap();
            if file_name != ".gitkeep" {
                let file_path =
                    BatConfig::get_auditor_code_overhaul_to_review_path(Some(file_name.clone()));
                let template_path = BatConfig::get_code_overhaul_template_path();
                let output = Command::new("cp")
                    .args([template_path, file_path])
                    .output()
                    .unwrap();
                if !output.stderr.is_empty() {
                    panic!(
                        "templates update failed with error: {:?}",
                        std::str::from_utf8(output.stderr.as_slice()).unwrap()
                    )
                };
            }
        }
    };
    create_git_commit(GitCommit::Templates, None);
    println!("Templates folder successfully updated");
}
