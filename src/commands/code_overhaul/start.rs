use error_stack::{Report, Result, ResultExt};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use crate::batbelt::git::GitCommit;

use crate::batbelt::path::{BatFile, BatFolder};

use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::commands::CommandError;

pub fn start_co_file() -> Result<(), CommandError> {
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
