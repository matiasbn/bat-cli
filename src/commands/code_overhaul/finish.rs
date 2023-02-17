use colored::Colorize;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::{FilePathType, FolderPathType};
use crate::commands::CommandError;

use error_stack::{Result, ResultExt};
use std::process::Command;

pub async fn finish_co_file() -> Result<(), CommandError> {
    check_correct_branch().change_context(CommandError)?;
    // get to-review files
    let started_endpoints =
        batbelt::helpers::get::get_started_entrypoints().change_context(CommandError)?;
    let prompt_text = "Select the code-overhaul to finish:";
    let selection = batbelt::cli_inputs::select(prompt_text, started_endpoints.clone(), None)
        .change_context(CommandError)?;

    let finished_endpoint = &started_endpoints[selection].clone();
    let finished_co_folder_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true)
            .change_context(CommandError)?;
    let started_co_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        },
        true,
    )
    .change_context(CommandError)?;
    batbelt::helpers::check::code_overhaul_file_completed(
        started_co_file_path.clone(),
        finished_endpoint.clone(),
    );

    Command::new("mv")
        .args([started_co_file_path, finished_co_folder_path])
        .output()
        .unwrap();

    create_git_commit(
        GitCommit::FinishCO,
        Some(vec![finished_endpoint.to_string()]),
    )
    .change_context(CommandError)?;

    println!("{} moved to finished", finished_endpoint.green());
    Ok(())
}
