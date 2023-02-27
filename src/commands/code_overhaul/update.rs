use colored::Colorize;

use crate::batbelt::git::GitCommit;

use crate::batbelt::path::BatFolder;
use crate::commands::CommandError;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use error_stack::{Report, Result, ResultExt};

pub fn update_co_file() -> Result<(), CommandError> {
    println!("Select the code-overhaul file to finish:");
    let finished_files_names = BatFolder::CodeOverhaulFinished
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;

    if finished_files_names.is_empty() {
        return Err(Report::new(CommandError).attach_printable(format!(
            "{}",
            "no finished files in code-overhaul folder".red()
        )));
    }

    let selection = BatDialoguer::select(
        "Select the code-overhaul file to update:".to_string(),
        finished_files_names.clone(),
        None,
    )
    .change_context(CommandError)?;

    let finished_file_name = finished_files_names[selection].clone();

    GitCommit::UpdateCO {
        entrypoint_name: finished_file_name,
    }
    .create_commit()
    .change_context(CommandError)?;
    Ok(())
}
