use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::{execute_command, CodeEditor};
use crate::batbelt::git::GitCommit;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};
use std::fs;

use crate::{batbelt, Suggestion};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{FutureExt, IntoReport, Report, ResultExt};
use regex::Regex;

use crate::batbelt::metadata::miro_metadata::{SignerInfo, SignerType};
use crate::batbelt::metadata::{BatMetadata, BatMetadataCommit, BatMetadataParser, MiroMetadata};
use crate::batbelt::miro::connector::ConnectorOptions;
use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::image::{MiroImage, MiroImageType};

use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::parser::code_overhaul_parser::CodeOverhaulParser;
use crate::batbelt::parser::source_code_parser::SourceCodeScreenshotOptions;
use crate::commands::miro_commands::{miro_command_functions, MiroCommand};

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum CodeOverhaulCommand {
    /// Starts a code-overhaul file audit
    #[default]
    Start,
    /// Moves the code-overhaul file from to-review to finished
    Finish,
    /// creates a code-overhaul summary from the code-overhaul finished notes
    Summary,
}

impl BatEnumerator for CodeOverhaulCommand {}

impl BatCommandEnumerator for CodeOverhaulCommand {
    fn execute_command(&self) -> CommandResult<()> {
        unimplemented!()
    }

    fn check_metadata_is_initialized(&self) -> bool {
        true
    }

    fn check_correct_branch(&self) -> bool {
        true
    }
}

impl CodeOverhaulCommand {
    pub async fn execute_command(&self) -> CommandResult<()> {
        match self {
            CodeOverhaulCommand::Start => self.execute_start().await,
            CodeOverhaulCommand::Finish => self.execute_finish(),
            CodeOverhaulCommand::Summary => self.execute_summary(),
        }
    }

    fn execute_summary(&self) -> CommandResult<()> {
        let mut co_summary_content = String::new();
        let co_finished_bat_files_vec = BatFolder::CodeOverhaulFinished
            .get_all_bat_files(true, None, None)
            .change_context(CommandError)?;
        for finished_co_file in co_finished_bat_files_vec {
            let co_file_content = finished_co_file
                .read_content(false)
                .change_context(CommandError)?;
            let state_changes_section_content = co_commands_functions::extract_section_content(
                &co_file_content,
                &CodeOverhaulSection::StateChanges.to_markdown_header(),
                &CodeOverhaulSection::Notes.to_markdown_header(),
            )?;
            let notes_section_content = co_commands_functions::extract_section_content(
                &co_file_content,
                &CodeOverhaulSection::Notes.to_markdown_header(),
                &CodeOverhaulSection::Signers.to_markdown_header(),
            )?;
            let miro_frame_url_section_content = co_commands_functions::extract_section_content(
                &co_file_content,
                &CodeOverhaulSection::MiroFrameUrl.to_markdown_header(),
                "",
            )?;
            let co_file_name = finished_co_file
                .get_file_name()
                .change_context(CommandError)?;
            let finished_file_summary = format!(
                "# {}\n\n{}\n\n{}\n\n{}\n\n## Code overhaul file path:\n\n[{}](code-overhaul/finished/{})",
                co_file_name,
                state_changes_section_content,
                notes_section_content,
                miro_frame_url_section_content,
                co_file_name,
                co_file_name
            );
            co_summary_content = if co_summary_content.is_empty() {
                finished_file_summary
            } else {
                format!("{}\n\n{}", co_summary_content, finished_file_summary)
            }
        }

        let code_overhaul_summary_bat_file = BatFile::CodeOverhaulSummaryFile;
        code_overhaul_summary_bat_file
            .write_content(false, &co_summary_content)
            .change_context(CommandError)?;

        Ok(())
    }

    fn execute_finish(&self) -> error_stack::Result<(), CommandError> {
        // get to-review files
        let started_entrypoints = BatFolder::CodeOverhaulStarted
            .get_all_files_dir_entries(true, None, None)
            .change_context(CommandError)?;
        let started_entrypoints_names = started_entrypoints
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>();

        let finished_endpoint = if started_entrypoints_names.len() == 1 {
            let selected = started_entrypoints_names[0].clone();
            println!("Moving {} to finished", selected.green());
            selected
        } else {
            let prompt_text = "Select the code-overhaul to finish:";
            let selection = BatDialoguer::select(
                prompt_text.to_string(),
                started_entrypoints_names.clone(),
                None,
            )
            .change_context(CommandError)?;
            started_entrypoints_names[selection].clone()
        };

        let finished_co_folder_path = BatFolder::CodeOverhaulFinished
            .get_path(true)
            .change_context(CommandError)?;
        let started_co_bat_file = BatFile::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        };
        let started_co_bat_file_path = started_co_bat_file
            .get_path(true)
            .change_context(CommandError)?;
        let started_co_bat_file_content = started_co_bat_file
            .read_content(true)
            .change_context(CommandError)?;
        let miro_placeholder =
            CoderOverhaulTemplatePlaceholders::CompleteWithMiroFrameUrl.to_placeholder();
        if started_co_bat_file_content.contains(&miro_placeholder) {
            let entrypoint_name = finished_endpoint.trim_end_matches(".md").to_string();
            if let Ok(miro_co_metadata) =
                MiroMetadata::get_co_metadata_by_entrypoint_name(entrypoint_name)
                    .change_context(CommandError)
            {
                let miro_frame_url =
                    MiroFrame::get_frame_url_by_frame_id(&miro_co_metadata.miro_frame_id)
                        .change_context(CommandError)?;
                let new_content =
                    started_co_bat_file_content.replace(&miro_placeholder, &miro_frame_url);
                started_co_bat_file
                    .write_content(true, &new_content)
                    .change_context(CommandError)?;
            }
        }

        co_commands_functions::check_code_overhaul_file_completed(started_co_bat_file)?;
        execute_command(
            "mv",
            &[&started_co_bat_file_path, &finished_co_folder_path],
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

    async fn execute_start(&self) -> error_stack::Result<(), CommandError> {
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

        let started_markdown_content = started_template
            .get_markdown_content()
            .change_context(CommandError)?;

        started_bat_file
            .write_content(false, &started_markdown_content)
            .change_context(CommandError)?;

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
        }
        let prompt_text = format!(
            "Do you want to deploy the code-overhaul frame for {} now?",
            entrypoint_name.clone().bright_green()
        );
        let deploy_frame = BatDialoguer::select_yes_or_no(prompt_text)?;
        if deploy_frame {
            MiroCommand::CodeOverhaulScreenshots {
                entry_point_name: Some(entrypoint_name.to_string()),
            }
            .execute_command()
            .await?
        }
        Ok(())
    }
}

mod co_commands_functions {
    use super::*;

    pub fn check_code_overhaul_file_completed(
        bat_file: BatFile,
    ) -> error_stack::Result<(), CommandError> {
        let file_data = bat_file.read_content(true).change_context(CommandError)?;
        let file_name = bat_file.get_file_name().change_context(CommandError)?;
        let mut suggestions_vec = vec![];
        let state_changes_checked_placeholders =
            CoderOverhaulTemplatePlaceholders::get_state_changes_checked_placeholders_vec();
        for checked_placeholder in state_changes_checked_placeholders {
            if file_data.contains(&checked_placeholder) {
                suggestions_vec.push(Suggestion(format!(
                    "Delete or update the `{}` place holder from the State changes section",
                    checked_placeholder.clone().bright_red()
                )))
            }
        }
        if !suggestions_vec.is_empty() {
            let mut report = Report::new(CommandError).attach_printable(format!(
                "\"State changes\" section of the {file_name} is not finished"
            ));
            for suggestion in suggestions_vec {
                report = report.attach(suggestion);
            }
            bat_file
                .open_in_editor(false, None)
                .change_context(CommandError)?;
            return Err(report);
        }

        if file_data
            .contains(&CoderOverhaulTemplatePlaceholders::CompleteWithNotes.to_placeholder())
        {
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
            bat_file
                .open_in_editor(false, None)
                .change_context(CommandError)?;
            return Err(Report::new(CommandError)
                .attach_printable(format!(
                    "Please complete the \"Signers\" section of the {file_name} file"
                ))
                .attach(Suggestion(format!(
                    "Delete {} from the Signers section",
                    CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription
                        .to_placeholder()
                ))));
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
            let user_decided_to_continue = batbelt::bat_dialoguer::select_yes_or_no(
                "Miro frame url section is not completed, do you want to proceed anyway?",
            )
            .change_context(CommandError)?;
            if !user_decided_to_continue {
                return Err(Report::new(CommandError).attach_printable("Aborted by the user"));
            }
        }
        Ok(())
    }

    pub fn extract_section_content(
        co_file_content: &str,
        section_header: &str,
        next_section_header: &str,
    ) -> CommandResult<String> {
        let section_content_regex = Regex::new(&format!(
            r#"({})[\s\S]+({})"#,
            section_header, next_section_header
        ))
        .into_report()
        .change_context(CommandError)?;
        log::debug!("{co_file_content}");
        log::debug!("{section_header}");
        log::debug!("{next_section_header}");
        let section_content = section_content_regex
            .find(&co_file_content)
            .ok_or(CommandError)
            .into_report()?
            .as_str()
            .to_string()
            .replace(section_header, &section_header.replace("#", "##"))
            .trim_end_matches(next_section_header)
            .trim()
            .to_string();
        Ok(section_content)
    }
}
