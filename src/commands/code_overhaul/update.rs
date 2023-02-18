use colored::Colorize;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::BatFolder;
use crate::commands::CommandError;

use error_stack::{Report, Result, ResultExt};
use std::fs;
use std::string::String;

pub fn update_co_file() -> Result<(), CommandError> {
    println!("Select the code-overhaul file to finish:");
    // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
    let finished_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulFinished, true)
        .change_context(CommandError)?;
    // get to-review files
    let finished_files = fs::read_dir(finished_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if finished_files.is_empty() {
        return Err(Report::new(CommandError).attach_printable(format!(
            "{}",
            "no finished files in code-overhaul folder".red()
        )));
    }

    let selection = batbelt::cli_inputs::select(
        "Select the code-overhaul file to update:",
        finished_files.clone(),
        None,
    )
    .change_context(CommandError)?;

    let finished_file_name = finished_files[selection].clone();
    check_correct_branch().change_context(CommandError)?;
    create_git_commit(GitCommit::UpdateCO, Some(vec![finished_file_name]))
        .change_context(CommandError)?;
    Ok(())
}
