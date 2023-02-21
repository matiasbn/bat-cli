use std::string::String;

use colored::Colorize;
use error_stack::{Report, Result, ResultExt};

use crate::batbelt;
use crate::batbelt::bash::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::git::{create_git_commit, GitCommit};

use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::commands::CommandError;

pub fn start_co_file() -> Result<(), CommandError> {
    let review_files = BatFolder::CodeOverhaulToReview
        .get_all_files_dir_entries(true, None, None)
        .change_context(CommandError)?
        .into_iter()
        .map(|entry| entry.file_name().to_str().unwrap().to_string())
        .collect::<Vec<_>>();

    if review_files.is_empty() {
        return Err(Report::new(CommandError)
            .attach_printable("no to-review files in code-overhaul folder"));
    }
    let prompt_text = "Select the code-overhaul file to start:";
    let selection = batbelt::cli_inputs::select(prompt_text, review_files.clone(), None)
        .change_context(CommandError)?;

    // user select file
    let to_start_file_name = &review_files[selection].clone();
    let entrypoint_name = to_start_file_name.trim_end_matches(".md");
    let to_review_file_path = BatFile::CodeOverhaulToReview {
        file_name: to_start_file_name.clone(),
    }
    .get_path(true)
    .change_context(CommandError)?;

    let started_path = BatFile::CodeOverhaulStarted {
        file_name: to_start_file_name.clone(),
    }
    .get_path(false)
    .change_context(CommandError)?;

    let started_template =
        CodeOverhaulTemplate::new(entrypoint_name, true).change_context(CommandError)?;
    let mut started_markdown = started_template
        .to_markdown_file(&started_path)
        .change_context(CommandError)?;

    started_markdown.save().change_context(CommandError)?;

    execute_command("rm", &[&to_review_file_path]).change_context(CommandError)?;

    println!("{to_start_file_name} file moved to started");

    create_git_commit(
        GitCommit::StartCO,
        Some(vec![to_start_file_name.to_string()]),
    )
    .change_context(CommandError)?;

    // open co file in VSCode
    vs_code_open_file_in_current_window(started_path.as_str())?;

    // open instruction file in VSCode
    if started_template.entrypoint_parser.is_some() {
        let ep_parser = started_template.entrypoint_parser.unwrap();
        if ep_parser.handler.is_some() {
            let handler = ep_parser.handler.unwrap();
            vs_code_open_file_in_current_window(&handler.path)?;
        }
    }
    Ok(())
}
