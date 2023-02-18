use crate::batbelt::bash::execute_command;
use crate::batbelt::constants::BASE_REPOSTORY_NAME;
use crate::batbelt::{
    self,
    git::{clone_base_repository, create_git_commit, GitCommit},
    path::{BatFile, BatFolder},
};
use error_stack::{Result, ResultExt};
use std::error::Error;
use std::fmt;
use std::fs;

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
    let templates_path = batbelt::path::get_folder_path(BatFolder::Templates, true)
        .change_context(UpdateTemplateError)?;
    execute_command("rm", &["-rf", templates_path.as_str()]).change_context(UpdateTemplateError)?;

    // move template to now location
    execute_command(
        "mv",
        &[
            &(BASE_REPOSTORY_NAME.to_string() + "/templates"),
            &templates_path,
        ],
    )
    .change_context(UpdateTemplateError)?;

    println!("Updating to-review files in code-overhaul folder");
    // move new templates to to-review in the auditor notes folder
    // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
    let to_review_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulToReview, true)
        .change_context(UpdateTemplateError)?;
    // if the auditor to-review code overhaul folder exists
    if fs::read_dir(to_review_path.clone()).is_ok() {
        let to_review_folder = fs::read_dir(to_review_path).unwrap();
        for file in to_review_folder {
            let file_name = file.unwrap().file_name().into_string().unwrap();
            if file_name != ".gitkeep" {
                let file_path = batbelt::path::get_file_path(
                    BatFile::CodeOverhaulToReview {
                        file_name: file_name.clone(),
                    },
                    true,
                )
                .change_context(UpdateTemplateError)?;
                let template_path = batbelt::path::get_folder_path(BatFolder::Templates, true)
                    .change_context(UpdateTemplateError)?;
                execute_command("cp", &[&template_path, &file_path])
                    .change_context(UpdateTemplateError)?;
            }
        }
    };

    // replace package.json
    println!("Updating package.json");
    execute_command(
        "mv",
        &[&(BASE_REPOSTORY_NAME.to_string() + "/package.json"), "."],
    )
    .change_context(UpdateTemplateError)?;

    // delete base_repository cloned
    execute_command("rm", &[&"-rf", BASE_REPOSTORY_NAME]).change_context(UpdateTemplateError)?;
    create_git_commit(GitCommit::UpdateRepo, None).change_context(UpdateTemplateError)?;
    println!("Repository successfully updated");
    Ok(())
}
