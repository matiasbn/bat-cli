use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::FolderPathType;
use crate::commands::CommandError;

use error_stack::{Result, ResultExt};
use std::fs;
use std::string::String;

pub fn update_co_file() -> Result<(), CommandError> {
    println!("Select the code-overhaul file to finish:");
    // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
    let finished_path = batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true)
        .change_context(CommandError)?;
    // get to-review files
    let finished_files = fs::read_dir(finished_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if finished_files.is_empty() {
        panic!("{}", "no finished files in code-overhaul folder".red());
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&finished_files)
        .default(0)
        .with_prompt("Select the code-overhaul file to update:")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to finished
        Some(index) => {
            let finished_file_name = finished_files[index].clone();
            check_correct_branch().change_context(CommandError)?;
            create_git_commit(GitCommit::UpdateCO, Some(vec![finished_file_name]))
                .change_context(CommandError)?;
            Ok(())
        }
        None => panic!("User did not select anything"),
    }
}
