use crate::batbelt;
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::{execute_command, CodeEditor};
use crate::batbelt::git::{check_correct_branch, GitCommit};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::commands::CommandError;
use crate::config::{BatAuditorConfig, BatConfig};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{Report, ResultExt};
use std::fs;

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq, Clone)]
pub enum CodeOverhaulCommand {
    /// Starts a code-overhaul file audit
    Start,
    /// Moves the code-overhaul file from to-review to finished
    Finish,
    /// Update a code-overhaul file by creating a commit
    Update,
    /// Counts the to-review, started, finished and total co files
    Count,
    /// Opens the co file and the instruction of a started entrypoint
    Open,
}

pub fn count_co_files() -> error_stack::Result<(), CommandError> {
    let (to_review_count, started_count, finished_count) = co_counter()?;
    println!("to-review co files: {}", format!("{to_review_count}").red());
    println!("started co files: {}", format!("{started_count}").yellow());
    println!("finished co files: {}", format!("{finished_count}").green());
    println!(
        "total co files: {}",
        format!("{}", to_review_count + started_count + finished_count).purple()
    );
    Ok(())
}

fn co_counter() -> error_stack::Result<(usize, usize, usize), CommandError> {
    let to_review_count = BatFolder::CodeOverhaulToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    let started_count = BatFolder::CodeOverhaulStarted
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    let finished_count = BatFolder::CodeOverhaulFinished
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    Ok((to_review_count, started_count, finished_count))
}

pub fn open_co() -> error_stack::Result<(), CommandError> {
    let _bat_config = BatConfig::get_config().change_context(CommandError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    // list to start
    if bat_auditor_config.use_code_editor {
        let options = vec!["started".green(), "finished".yellow()];
        let prompt_text = format!(
            "Do you want to open a {} or a {} file?",
            options[0], options[1]
        );
        let selection = batbelt::bat_dialoguer::select(&prompt_text, options.clone(), None)
            .change_context(CommandError)?;
        let open_started = selection == 0;
        let co_folder = if open_started {
            BatFolder::CodeOverhaulStarted
        } else {
            BatFolder::CodeOverhaulFinished
        };
        let co_files = co_folder
            .get_all_files_dir_entries(true, None, None)
            .change_context(CommandError)?
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>();
        if !co_files.is_empty() {
            let prompt_text = "Select the code-overhaul file to open:";
            let selection = batbelt::bat_dialoguer::select(prompt_text, co_files.clone(), None)
                .change_context(CommandError)?;
            let file_name = &co_files[selection].clone();
            let bat_file = if open_started {
                BatFile::CodeOverhaulStarted {
                    file_name: file_name.clone(),
                }
            } else {
                BatFile::CodeOverhaulFinished {
                    file_name: file_name.clone(),
                }
            };
            let ep_parser =
                EntrypointParser::new_from_name(file_name.clone().trim_end_matches(".md"))
                    .change_context(CommandError)?;

            bat_file
                .open_in_editor(true, None)
                .change_context(CommandError)?;
            if ep_parser.handler.is_some() {
                let handler_metadata = ep_parser.handler.unwrap();
                let _instruction_file_path = handler_metadata.path;
                let _start_line_index = handler_metadata.start_line_index;
                // BatAuditorConfig::get_config()
                //     .change_context(CommandError)?
                //     .code_editor::;
            }
            BatFile::ProgramLib
                .open_in_editor(true, Some(ep_parser.entrypoint_function.start_line_index))
                .change_context(CommandError)?;
            return Ok(());
        } else {
            println!("Empty {} folder", options[selection].clone());
        }
        BatFile::ProgramLib
            .open_in_editor(true, None)
            .change_context(CommandError)?;
    } else {
        print!("VSCode integration not enabled");
    }
    Ok(())
}

pub async fn finish_co_file() -> error_stack::Result<(), CommandError> {
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
    let selection = BatDialoguer::select(
        prompt_text.to_string(),
        started_entrypoints_names.clone(),
        None,
    )
    .change_context(CommandError)?;

    let finished_endpoint = started_entrypoints_names[selection].clone();
    let finished_co_folder_path = BatFolder::CodeOverhaulFinished
        .get_path(true)
        .change_context(CommandError)?;
    let started_co_file_path = BatFile::CodeOverhaulStarted {
        file_name: finished_endpoint.clone(),
    }
    .get_path(true)
    .change_context(CommandError)?;
    check_code_overhaul_file_completed(started_co_file_path.clone())?;
    execute_command(
        "mv",
        &[&started_co_file_path, &finished_co_folder_path],
        false,
    )
    .change_context(CommandError)?;
    GitCommit::FinishCO {
        entrypoint_name: finished_endpoint.clone(),
    }
    .create_commit()
    .change_context(CommandError)?;

    println!("{} moved to finished", finished_endpoint.green());
    Ok(())
}

fn check_code_overhaul_file_completed(file_path: String) -> error_stack::Result<(), CommandError> {
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

pub fn start_co_file() -> error_stack::Result<(), CommandError> {
    let review_files = BatFolder::CodeOverhaulToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?;

    if review_files.is_empty() {
        return Err(Report::new(CommandError)
            .attach_printable("no to-review files in code-overhaul folder"));
    }
    let prompt_text = "Select the code-overhaul file to start:";
    let selection = BatDialoguer::select(prompt_text.to_string(), review_files.clone(), None)
        .change_context(CommandError)?;

    // user select file
    let to_start_file_name = &review_files[selection].clone();
    let entrypoint_name = to_start_file_name.trim_end_matches(".md");

    BatFile::CodeOverhaulToReview {
        file_name: to_start_file_name.clone(),
    }
    .remove_file()
    .change_context(CommandError)?;

    let started_bat_file = BatFile::CodeOverhaulStarted {
        file_name: to_start_file_name.clone(),
    };

    let started_template =
        CodeOverhaulTemplate::new(entrypoint_name, true).change_context(CommandError)?;
    let mut started_markdown = started_template
        .to_markdown_file(
            &started_bat_file
                .get_path(false)
                .change_context(CommandError)?,
        )
        .change_context(CommandError)?;

    started_markdown.save().change_context(CommandError)?;

    println!("{to_start_file_name} file moved to started");

    GitCommit::StartCO {
        entrypoint_name: to_start_file_name.clone(),
    }
    .create_commit()
    .change_context(CommandError)?;

    started_bat_file
        .open_in_editor(true, None)
        .change_context(CommandError)?;

    // open instruction file in VSCode
    if started_template.entrypoint_parser.is_some() {
        let ep_parser = started_template.entrypoint_parser.unwrap();
        if ep_parser.handler.is_some() {
            let handler = ep_parser.handler.unwrap();
            CodeEditor::open_file_in_editor(&handler.path, Some(handler.start_line_index))?;
        }
        CodeEditor::open_file_in_editor(
            &ep_parser.entrypoint_function.path,
            Some(ep_parser.entrypoint_function.start_line_index),
        )?;
    }
    Ok(())
}

pub fn update_co_file() -> error_stack::Result<(), CommandError> {
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
