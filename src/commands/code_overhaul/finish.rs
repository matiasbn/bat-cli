use colored::Colorize;
use std::fs;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::{BatFile, BatFolder};
use crate::commands::CommandError;

use crate::batbelt::command_line::execute_command;
use crate::batbelt::templates::code_overhaul_template::CoderOverhaulTemplatePlaceholders;
use error_stack::{Report, Result, ResultExt};

pub async fn finish_co_file() -> Result<(), CommandError> {
    check_correct_branch().change_context(CommandError)?;
    // get to-review files
    let started_entrypoints = BatFolder::CodeOverhaulStarted
        .get_all_files_dir_entries(true, None, None)
        .change_context(CommandError)?;
    let started_entrypoints_names = started_entrypoints
        .into_iter()
        .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let prompt_text = "Select the code-overhaul to finish:";
    let selection =
        batbelt::bat_dialoguer::select(prompt_text, started_entrypoints_names.clone(), None)
            .change_context(CommandError)?;

    let finished_endpoint = &started_entrypoints_names[selection].clone();
    let finished_co_folder_path =
        batbelt::path::get_folder_path(BatFolder::CodeOverhaulFinished, true)
            .change_context(CommandError)?;
    let started_co_file_path = batbelt::path::get_file_path(
        BatFile::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        },
        true,
    )
    .change_context(CommandError)?;
    check_code_overhaul_file_completed(started_co_file_path.clone())?;
    execute_command("mv", &[&started_co_file_path, &finished_co_folder_path])
        .change_context(CommandError)?;

    create_git_commit(
        GitCommit::FinishCO,
        Some(vec![finished_endpoint.to_string()]),
    )
    .change_context(CommandError)?;

    println!("{} moved to finished", finished_endpoint.green());
    Ok(())
}
fn check_code_overhaul_file_completed(file_path: String) -> Result<(), CommandError> {
    let file_data = fs::read_to_string(file_path).unwrap();
    if file_data
        .contains(&CoderOverhaulTemplatePlaceholders::CompleteWithStateChanges.to_placeholder())
    {
        return Err(Report::new(CommandError).attach_printable(
            "Please complete the \"What it does?\" section of the {file_name} file",
        ));
    }

    if file_data.contains(&CoderOverhaulTemplatePlaceholders::CompleteWithNotes.to_placeholder()) {
        let user_decided_to_continue = batbelt::bat_dialoguer::select_yes_or_no(
            "Notes section not completed, do you want to proceed anyway?",
        )
        .change_context(CommandError)?;
        if !user_decided_to_continue {
            return Err(Report::new(CommandError).attach_printable("Aborted by the user"));
        }
    }

    if file_data.contains(
        &CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription.to_placeholder(),
    ) {
        return Err(Report::new(CommandError)
            .attach_printable("Please complete the \"Signers\" section of the {file_name} file"));
    }

    if file_data
        .contains(&CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder())
    {
        let user_decided_to_continue = batbelt::bat_dialoguer::select_yes_or_no(
            "Validations section not completed, do you want to proceed anyway?",
        )
        .change_context(CommandError)?;
        if !user_decided_to_continue {
            return Err(Report::new(CommandError).attach_printable("Aborted by the user"));
        }
    }

    if file_data
        .contains(&CoderOverhaulTemplatePlaceholders::CompleteWithMiroFrameUrl.to_placeholder())
    {
        return Err(Report::new(CommandError).attach_printable(
            "Please complete the \"Miro board frame\" section of the {file_name} file",
        ));
    }
    Ok(())
}
