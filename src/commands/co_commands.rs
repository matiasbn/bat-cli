use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::git::git_commit::GitCommit;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::config::BatConfig;
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use crate::{batbelt, Suggestion};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{FutureExt, IntoReport, Report, ResultExt};
use lazy_regex::regex;
use regex::Regex;

use crate::batbelt::metadata::program_accounts_metadata::ProgramAccountMetadata;
use crate::batbelt::metadata::{BatMetadataParser, MiroMetadata};
use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::parser::code_overhaul_parser::CodeOverhaulParser;
use crate::commands::miro_commands::MiroCommand;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum CodeOverhaulCommand {
    /// Starts a code-overhaul file audit
    Start {
        /// Skips miro deployment
        #[arg(long)]
        skip_miro: bool,
        /// Starts a guided process to start a co file
        #[arg(long)]
        interactive: bool,
    },
    /// Moves the code-overhaul file from to-review to finished
    #[default]
    Finish,
    /// creates a code-overhaul summary from the code-overhaul finished notes
    Summary,
    /// creates program accounts metadata
    CreateProgramAccountsMetadata,
    /// calculates state changes from the program accounts metadata
    UpdateProgramAccountsMetadata,
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
            CodeOverhaulCommand::Start {
                skip_miro,
                interactive,
            } => self.execute_start(*skip_miro, *interactive).await,
            CodeOverhaulCommand::Finish => self.execute_finish(),
            CodeOverhaulCommand::Summary => self.execute_summary(),
            CodeOverhaulCommand::CreateProgramAccountsMetadata => {
                self.execute_program_accounts_metadata()
            }
            CodeOverhaulCommand::UpdateProgramAccountsMetadata => self.execute_calculate_sc(),
        }
    }

    fn execute_calculate_sc(&self) -> CommandResult<()> {
        ProgramAccountMetadata::update_program_accounts_metadata_file()
            .change_context(CommandError)?;
        println!("Programs account metadata created");
        Ok(())
    }

    fn execute_program_accounts_metadata(&self) -> CommandResult<()> {
        ProgramAccountMetadata::create_program_accounts_metadata_file()
            .change_context(CommandError)?;
        println!("Programs account metadata created");
        Ok(())
    }

    fn execute_summary(&self) -> CommandResult<()> {
        let mut co_summary_content = String::new();
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
        let program_names: Vec<Option<String>> = if bat_config.is_multi_program() {
            bat_config
                .get_program_names()
                .into_iter()
                .map(Some)
                .collect()
        } else {
            vec![None]
        };
        let mut all_finished_files = vec![];
        for pn in &program_names {
            let files = BatFolder::CodeOverhaulFinished {
                program_name: pn.clone(),
            }
            .get_all_bat_files(true, None, None)
            .change_context(CommandError)?;
            all_finished_files.extend(files);
        }
        let co_finished_bat_files_vec = all_finished_files;
        for finished_co_file in co_finished_bat_files_vec {
            let entry_point_name = finished_co_file
                .get_file_name()
                .change_context(CommandError)?
                .trim_end_matches(".md")
                .to_string();
            let co_parser = CodeOverhaulParser::new_from_entry_point_name(entry_point_name)
                .change_context(CommandError)?;
            log::debug!("co_parser:\n{:#?}", co_parser);
            let _state_changes_section_content = co_parser.section_content.state_changes.replace(
                &CodeOverhaulSection::StateChanges.to_markdown_header(),
                &CodeOverhaulSection::StateChanges
                    .to_markdown_header()
                    .replace("#", "##"),
            );
            let mut notes_section_content = co_parser.section_content.notes.replace(
                &CodeOverhaulSection::Notes.to_markdown_header(),
                &CodeOverhaulSection::Notes
                    .to_markdown_header()
                    .replace("#", "##"),
            );
            let miro_frame_url_section_content = co_parser.section_content.miro_frame_url.replace(
                &CodeOverhaulSection::MiroFrameUrl.to_markdown_header(),
                &CodeOverhaulSection::MiroFrameUrl
                    .to_markdown_header()
                    .replace("#", "##"),
            );
            let co_file_name = finished_co_file
                .get_file_name()
                .change_context(CommandError)?;

            let checkbox_regex = regex!(r#"- \[x\]"#);
            let notes_section_filtered = notes_section_content
                .lines()
                .filter(|line| !checkbox_regex.is_match(line))
                .collect::<Vec<_>>();

            if notes_section_filtered.len() == 2 && notes_section_filtered[1].is_empty() {
                continue;
            }

            notes_section_content = notes_section_filtered.join("\n");

            let finished_file_summary = format!(
                "# {}\n\n{}\n\n{}\n\n## Code overhaul file path:\n\n[{}](code-overhaul/finished/{})",
                co_file_name,
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
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
        let program_name = if bat_config.is_multi_program() {
            Some(
                bat_config
                    .prompt_select_program()
                    .change_context(CommandError)?,
            )
        } else {
            None
        };

        // get to-review files
        let started_entrypoint_direntry_vec = BatFolder::CodeOverhaulStarted {
            program_name: program_name.clone(),
        }
        .get_all_files_dir_entries(true, None, None)
            .change_context(CommandError)?;
        let started_entrypoint_names = started_entrypoint_direntry_vec
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>();

        if started_entrypoint_names.is_empty() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "{} folder is empty",
                "code-overhaul/started".green()
            )));
        }

        let finished_endpoint = if started_entrypoint_names.len() == 1 {
            let selected = started_entrypoint_names[0].clone();
            println!("Moving {} to finished", selected.green());
            selected
        } else {
            let prompt_text = "Select the code-overhaul to finish:";
            let selection = BatDialoguer::select(
                prompt_text.to_string(),
                started_entrypoint_names.clone(),
                None,
            )
            .change_context(CommandError)?;
            started_entrypoint_names[selection].clone()
        };

        let finished_co_folder_path = BatFolder::CodeOverhaulFinished {
            program_name: program_name.clone(),
        }
        .get_path(true)
            .change_context(CommandError)?;
        let started_co_bat_file = BatFile::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
            program_name: program_name.clone(),
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
            program_name: program_name.clone(),
        }
        .create_commit(true)
        .change_context(CommandError)?;

        println!("{} moved to finished", finished_endpoint.green());
        Ok(())
    }

    async fn execute_start(
        &self,
        skip_miro: bool,
        _interactive: bool,
    ) -> error_stack::Result<(), CommandError> {
        let bat_config = BatConfig::get_config().change_context(CommandError)?;
        let program_name = if bat_config.is_multi_program() {
            Some(
                bat_config
                    .prompt_select_program()
                    .change_context(CommandError)?,
            )
        } else {
            None
        };

        let review_files = BatFolder::CodeOverhaulToReview {
            program_name: program_name.clone(),
        }
        .get_all_files_names(true, None, None)
            .change_context(CommandError)?;

        if review_files.is_empty() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "{} folder is empty",
                "code-overhaul/to-review".green()
            )));
        }
        let prompt_text = "Select the code-overhaul file to start:";
        let selection = BatDialoguer::select(prompt_text.to_string(), review_files.clone(), None)
            .change_context(CommandError)?;

        // user select file
        let to_start_file_name = &review_files[selection].clone();
        let entrypoint_name = to_start_file_name.trim_end_matches(".md");

        BatFile::CodeOverhaulToReview {
            file_name: to_start_file_name.clone(),
            program_name: program_name.clone(),
        }
        .remove_file()
        .change_context(CommandError)?;

        let started_bat_file = BatFile::CodeOverhaulStarted {
            file_name: to_start_file_name.clone(),
            program_name: program_name.clone(),
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
            program_name: program_name.clone(),
        }
        .create_commit(true)
        .change_context(CommandError)?;

        started_bat_file
            .open_in_editor(true, None)
            .change_context(CommandError)?;

        // open entrypoint file at the entrypoint function line
        if let Some(ep_parser) = started_template.entrypoint_parser {
            let ep_bat_file = BatFile::Generic {
                file_path: ep_parser.entry_point_function.path.clone(),
            };
            ep_bat_file
                .open_in_editor(true, Some(ep_parser.entry_point_function.start_line_index))
                .change_context(CommandError)?;
        }
        if !skip_miro {
            let deployed = co_commands_functions::prompt_deploy_miro(
                entrypoint_name.to_string(),
                program_name.clone(),
            )
            .await?;
            if deployed {
                GitCommit::UpdateCO {
                    entrypoint_name: to_start_file_name.clone(),
                    program_name: program_name.clone(),
                }
                .create_commit(true)
                .change_context(CommandError)?;
            }
        }
        Ok(())
    }

    // fn execute_start_interactive(&self) -> CommandResult<String> {
    //     let suggested_entry_point_cache =
    //         EntryPointFlow::get_suggested_next_entry_point()
    //             .change_context(CommandError)?;
    //     let entry_point_name = suggested_entry_point_cache.entry_point_name;
    //     let ep_parser =
    //         EntrypointParser::new_from_name(&entry_point_name).change_context(CommandError)?;
    //     let handler_sc_metadata = ep_parser.handler.unwrap();
    //     let handler_sc_parser = handler_sc_metadata.to_source_code_parser(None);
    //
    //     CodeEditor::open_file_in_editor(
    //         &handler_sc_parser.path,
    //         Some(handler_sc_parser.start_line_index),
    //     )?;
    //
    //     let handler_content = handler_sc_parser.get_source_code_content();
    //
    //     let handler_content_lines = handler_content.lines().collect::<Vec<_>>();
    //     if !suggested_entry_point_cache.init_program_accounts.is_empty() {
    //         for init_program_account in suggested_entry_point_cache.init_program_accounts {
    //             println!("Initializing: {}", init_program_account.bright_green());
    //             let solana_account_parser =
    //                 SolanaAccountParser::new_from_struct_name_and_solana_account_type(
    //                     init_program_account,
    //                     SolanaAccountType::ProgramStateAccount,
    //                 )
    //                 .change_context(CommandError)?;
    //             for account in solana_account_parser.accounts {
    //                 let prompt_text = format!(
    //                     "Is the {}[{}] value assigned on this handler?:",
    //                     account.account_name, account.account_type
    //                 );
    //                 let is_assigned = BatDialoguer::select_yes_or_no(prompt_text)?;
    //                 if !is_assigned {
    //                     continue;
    //                 }
    //                 let prompt_text = format!(
    //                     "Select the lines with the value for {}[{}] with the space bar",
    //                     account.account_name.bright_green(),
    //                     account.account_type.bright_yellow()
    //                 );
    //                 let selection = BatDialoguer::multiselect(
    //                     prompt_text,
    //                     handler_content_lines.clone(),
    //                     None,
    //                     true,
    //                 )?;
    //                 let parsed_value = if selection.len() == 1 {
    //                     co_commands_functions::get_value_single_line(
    //                         handler_content_lines[selection[0]],
    //                     )
    //                 } else {
    //                     co_commands_functions::get_value_single_line(
    //                         handler_content_lines[selection[0]],
    //                     )
    //                 };
    //                 // let parse_value = println!(
    //                 //     "handler_content_lines[selection]: {}",
    //                 //     handler_content_lines[selection]
    //                 // );
    //             }
    //         }
    //     }
    //     // println!(
    //     //     "init_program_ca_metadata: {:#?}",
    //     //     suggested_entry_point_cache
    //     // );
    //     Ok("".to_string())
    // }
}

mod co_commands_functions {
    use super::*;
    use lazy_regex::regex;

    pub fn get_value_single_line(line: &str) -> CommandResult<String> {
        let inline_assignment_regex = regex!(r#"[\w_.()? ]+= "#);
        match inline_assignment_regex.find(line.trim()) {
            None => {}
            Some(_line_match) => {}
        }
        let _struct_assignment_regex = regex!(r#"[\w_ ]+: ]"#);
        Ok("".to_string())
    }

    pub fn get_value_multi_line(
        _lines_vec: Vec<&str>,
        _selection_vec: Vec<usize>,
    ) -> CommandResult<String> {
        // let inline_assignment_regex = regex!(r#"[\w_.()? ]+= "#);
        // match inline_assignment_regex.find(line.trim()) {
        //     None => {}
        //     Some(line_match) => {}
        // }
        // let struct_assignment_regex = regex!(r#"[\w_ ]+: ]"#);
        Ok("".to_string())
    }

    pub async fn prompt_deploy_miro(
        entry_point_name: String,
        program_name: Option<String>,
    ) -> CommandResult<bool> {
        let prompt_text = format!(
            "Do you want to deploy the code-overhaul screenshots to Miro for {} now?",
            entry_point_name.clone().bright_green()
        );
        let deploy_frame = BatDialoguer::select_yes_or_no(prompt_text)?;
        if deploy_frame {
            MiroCommand::deploy_co_screenshots_with_program(
                Some(entry_point_name.to_string()),
                program_name,
            )
            .await?
        }
        Ok(deploy_frame)
    }

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
            let user_decided_to_continue = BatDialoguer::select_yes_or_no(format!(
                "{} section not completed, do you want to proceed anyway?",
                "Validations".green()
            ))
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
            .find(co_file_content)
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
