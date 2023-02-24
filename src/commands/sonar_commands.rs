use crate::batbelt;
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::BatMetadataType;
use clap::{Parser, Subcommand};
use colored::Colorize;
use error_stack::{Result, ResultExt};

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use super::CommandError;

#[derive(Subcommand, Debug, strum_macros::Display)]
pub enum SonarCommand {
    /// Updates the functions.md and structs.md files with data
    Run,
    /// Gets the path to a metadata information from metadata files
    PrintPath,
}

impl SonarCommand {
    pub fn execute_command(&self) -> Result<(), CommandError> {
        match self {
            SonarCommand::Run => self.execute_run(),
            SonarCommand::PrintPath => self.execute_print_path(),
        }
    }

    fn execute_print_path(&self) -> Result<(), CommandError> {
        let mut continue_printing = true;
        while continue_printing {
            let selected_bat_metadata_type =
                BatMetadataType::prompt_metadata_type_selection().change_context(CommandError)?;
            match selected_bat_metadata_type {
                BatMetadataType::Structs => {
                    let selections = StructMetadata::prompt_multiselection(false, true)
                        .change_context(CommandError)?;
                    for selection in selections {
                        self.print_formatted_path(
                            selection.name,
                            selection.path,
                            selection.start_line_index,
                        )
                    }
                }
                BatMetadataType::Functions => {
                    let selections = FunctionMetadata::prompt_multiselection(false, true)
                        .change_context(CommandError)?;
                    for selection in selections {
                        self.print_formatted_path(
                            selection.name,
                            selection.path,
                            selection.start_line_index,
                        )
                    }
                }
                BatMetadataType::Entrypoints => unimplemented!(),
            }
            let prompt_text = format!("Do you want to continute {}", "printing paths?".yellow());
            continue_printing = BatDialoguer::select_yes_or_no(prompt_text)?;
        }
        Ok(())
    }

    fn print_formatted_path(&self, name: String, path: String, start_line_index: usize) {
        println!(
            "{}: {}:{}",
            name.blue(),
            path.trim_start_matches("../"),
            start_line_index
        )
    }

    fn execute_run(&self) -> Result<(), CommandError> {
        BatSonar::display_looking_for_loader(SonarResultType::Struct);
        self.structs()?;
        BatSonar::display_looking_for_loader(SonarResultType::Function);
        self.functions()?;
        Ok(())
    }

    fn functions(&self) -> Result<(), CommandError> {
        let mut functions_metadata_markdown = BatMetadataType::Functions
            .get_markdown()
            .change_context(CommandError)?;
        let functions_metadata =
            FunctionMetadata::get_functions_metadata_from_program().change_context(CommandError)?;
        let functions_markdown_content = functions_metadata
            .into_iter()
            .map(|function_metadata| function_metadata.get_markdown_section_content_string())
            .collect::<Vec<_>>()
            .join("\n\n");
        functions_metadata_markdown.content = functions_markdown_content;
        functions_metadata_markdown
            .save()
            .change_context(CommandError)?;
        batbelt::git::create_git_commit(
            GitCommit::UpdateMetadata {
                metadata_type: BatMetadataType::Functions,
            },
            None,
        )
        .unwrap();
        Ok(())
    }

    fn structs(&self) -> Result<(), CommandError> {
        let mut structs_metadata_markdown = BatMetadataType::Structs
            .get_markdown()
            .change_context(CommandError)?;
        let structs_metadata =
            StructMetadata::get_structs_metadata_from_program().change_context(CommandError)?;
        let structs_markdown_content = structs_metadata
            .into_iter()
            .map(|struct_metadata| struct_metadata.get_markdown_section_content_string())
            .collect::<Vec<_>>()
            .join("\n\n");
        structs_metadata_markdown.content = structs_markdown_content;
        structs_metadata_markdown
            .save()
            .change_context(CommandError)?;
        batbelt::git::create_git_commit(
            GitCommit::UpdateMetadata {
                metadata_type: BatMetadataType::Structs,
            },
            None,
        )
        .unwrap();
        Ok(())
    }
}
