use crate::batbelt::bash::execute_command;
use crate::batbelt::constants::BASE_REPOSTORY_NAME;
use crate::batbelt::{
    self,
    git::{clone_base_repository, create_git_commit, GitCommit},
    path::{FilePathType, FolderPathType},
};
use error_stack::{Result, ResultExt};
use std::error::Error;
use std::fmt;
use std::{fs, process::Command};

#[derive(Debug)]
pub struct UpdateTemplateError;

impl fmt::Display for UpdateTemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("General Bat error")
    }
}

impl Error for UpdateTemplateError {}

pub fn update_repository() -> Result<(), UpdateTemplateError> {
    // clone base repository
    println!("Cloning base repository");
    clone_base_repository();

    // delete templates folder
    println!("Updating templates folder");
    // let templates_path = utils::path::get_templates_path()?;
    let templates_path = batbelt::path::get_folder_path(FolderPathType::Templates, true)
        .change_context(UpdateTemplateError)?;
    let output = Command::new("rm")
        .args(["-rf", templates_path.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update repository failed with error: {:?}",
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
            "update repository failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Updating to-review files in code-overhaul folder");
    // move new templates to to-review in the auditor notes folder
    // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
    let to_review_path = batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, true)
        .change_context(UpdateTemplateError)?;
    // if the auditor to-review code overhaul folder exists
    if fs::read_dir(to_review_path.clone()).is_ok() {
        let to_review_folder = fs::read_dir(to_review_path).unwrap();
        for file in to_review_folder {
            let file_name = file.unwrap().file_name().into_string().unwrap();
            if file_name != ".gitkeep" {
                let file_path = batbelt::path::get_file_path(
                    FilePathType::CodeOverhaulToReview {
                        file_name: file_name.clone(),
                    },
                    true,
                )
                .change_context(UpdateTemplateError)?;
                let template_path = batbelt::path::get_folder_path(FolderPathType::Templates, true)
                    .change_context(UpdateTemplateError)?;
                execute_command("cp", &[&template_path, &file_path])
                    .change_context(UpdateTemplateError)?;
                // let output = Command::new("cp")
                //     .args([template_path, file_path])
                //     .output()
                //     .unwrap();
                // if !output.stderr.is_empty() {
                //     panic!(
                //         "templates update failed with error: {:?}",
                //         std::str::from_utf8(output.stderr.as_slice()).unwrap()
                //     )
                // };
            }
        }
    };

    // replace package.json
    println!("Updating package.json");
    let output = Command::new("mv")
        .args([
            BASE_REPOSTORY_NAME.to_string() + "/package.json",
            ".".to_string(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update repository failed with error: {:?}",
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
            "update repository failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    if !output.stderr.is_empty() {
        panic!(
            "update repository failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    create_git_commit(GitCommit::UpdateRepo, None).change_context(UpdateTemplateError)?;
    println!("Repository successfully updated");
    Ok(())
}
