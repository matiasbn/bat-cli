use crate::batbelt::bash::execute_command;
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::batbelt::{
    self,
    git::{create_git_commit, GitCommit},
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
                let file_path = BatFile::CodeOverhaulToReview {
                    file_name: file_name.clone(),
                }
                .get_path(true)
                .change_context(UpdateTemplateError)?;
                execute_command("rm", &[&file_path]).change_context(UpdateTemplateError)?;
                let co_template = CodeOverhaulTemplate::new(&file_name, false)
                    .change_context(UpdateTemplateError)?;
                let mut co_markdown = co_template
                    .to_markdown_file(&file_path)
                    .change_context(UpdateTemplateError)?;
                co_markdown.save().change_context(UpdateTemplateError)?;
            }
        }
    };

    // replace package.json
    println!("Updating package.json");
    PackageJsonTemplate::update_package_json().change_context(UpdateTemplateError)?;

    create_git_commit(GitCommit::UpdateRepo, None).change_context(UpdateTemplateError)?;
    println!("Repository successfully updated");
    Ok(())
}
