use colored::Colorize;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::{FilePathType, FolderPathType};

use std::process::Command;
use std::string::String;

pub async fn finish_co_file() -> Result<(), String> {
    check_correct_branch()?;
    // get to-review files
    let started_endpoints = batbelt::helpers::get::get_started_entrypoints()?;
    let prompt_text = "Select the code-overhaul to finish:";
    let selection = batbelt::cli_inputs::select(prompt_text, started_endpoints.clone(), None)?;

    let finished_endpoint = &started_endpoints[selection].clone();
    let finished_co_folder_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true);
    let started_co_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        },
        true,
    );
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
    )?;

    println!("{} moved to finished", finished_endpoint.green());
    Ok(())
}
