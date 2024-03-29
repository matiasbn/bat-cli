use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use std::env;

use crate::batbelt::path::{BatFile, BatFolder};

use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use clap::Subcommand;
use colored::Colorize;

use error_stack::{Report, ResultExt};

use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::structs_source_code_metadata::StructSourceCodeMetadata;
use crate::batbelt::metadata::traits_source_code_metadata::TraitSourceCodeMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, BatMetadataType};

use crate::batbelt::templates::package_json_template::PackageJsonTemplate;

use crate::batbelt;
use crate::batbelt::metadata::enums_source_code_metadata::EnumSourceCodeMetadata;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::config::BatAuditorConfig;
use log::Level;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ToolCommand {
    /// Opens a file from source code metadata to code editor. If code editor is None, then prints the path
    #[default]
    OpenSourceCode,
    /// Customize the package.json according to certain log level
    CustomizePackageJson,
    /// Opens the co file and the instruction file of a started entrypoint
    OpenCodeOverhaulFile,
    /// Search source code metadata by id and opens on code editor, if is source_code
    GetMetadataById,
    /// Counts the to-review, started, finished and total co files
    CountCodeOverhaul,
}

impl BatEnumerator for ToolCommand {}

impl BatCommandEnumerator for ToolCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ToolCommand::OpenSourceCode => self.execute_open_source_code(),
            ToolCommand::CustomizePackageJson => self.execute_package_json(),
            ToolCommand::GetMetadataById => self.execute_get_metadata_by_id(),
            ToolCommand::OpenCodeOverhaulFile => self.execute_open_co(),
            ToolCommand::CountCodeOverhaul => self.execute_count_co_files(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        match self {
            ToolCommand::OpenSourceCode => true,
            ToolCommand::CustomizePackageJson => false,
            ToolCommand::GetMetadataById => true,
            ToolCommand::OpenCodeOverhaulFile => true,
            ToolCommand::CountCodeOverhaul => false,
        }
    }

    fn check_correct_branch(&self) -> bool {
        match self {
            ToolCommand::OpenSourceCode => false,
            ToolCommand::CustomizePackageJson => false,
            ToolCommand::GetMetadataById => false,
            ToolCommand::OpenCodeOverhaulFile => false,
            ToolCommand::CountCodeOverhaul => false,
        }
    }
}

impl ToolCommand {
    fn execute_open_source_code(&self) -> CommandResult<()> {
        let selected_bat_metadata_type =
            BatMetadataType::prompt_metadata_type_selection().change_context(CommandError)?;
        let (path, start_line_index) = match selected_bat_metadata_type {
            BatMetadataType::Struct => {
                let StructSourceCodeMetadata {
                    path,
                    start_line_index,
                    ..
                } = StructSourceCodeMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
            BatMetadataType::Function => {
                let FunctionSourceCodeMetadata {
                    path,
                    start_line_index,
                    ..
                } = FunctionSourceCodeMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
            BatMetadataType::Trait => {
                let TraitSourceCodeMetadata {
                    path,
                    start_line_index,
                    ..
                } = TraitSourceCodeMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
            BatMetadataType::Enum => {
                let EnumSourceCodeMetadata {
                    path,
                    start_line_index,
                    ..
                } = EnumSourceCodeMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
        };
        CodeEditor::open_file_in_editor(&path, Some(start_line_index))
            .change_context(CommandError)?;
        Ok(())
    }

    fn execute_package_json(&self) -> CommandResult<()> {
        let prompt_text = "Select the log level:".to_string();
        let log_level_vec = vec![
            Level::Warn,
            Level::Info,
            Level::Debug,
            Level::Trace,
            Level::Error,
        ];
        let selection = BatDialoguer::select(
            prompt_text,
            log_level_vec
                .clone()
                .into_iter()
                .enumerate()
                .map(|(idx, level)| ToolCommand::colored_from_index(&level.to_string(), idx))
                .collect::<Vec<_>>(),
            None,
        )?;
        let level_selected = log_level_vec[selection];
        PackageJsonTemplate::create_package_json(Some(level_selected)).change_context(CommandError)
    }

    fn execute_get_metadata_by_id(&self) -> CommandResult<()> {
        let metadata_id = BatDialoguer::input("Metadata id:".to_string())?;
        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        for function_metadata in bat_metadata.source_code.functions_source_code {
            if function_metadata.metadata_id == metadata_id {
                println!("Metadata found:\n{:#?}", function_metadata);
                CodeEditor::open_file_in_editor(
                    &function_metadata.path,
                    Some(function_metadata.start_line_index),
                )
                .change_context(CommandError)?;
                return Ok(());
            }
        }
        for struct_metadata in bat_metadata.source_code.structs_source_code {
            if struct_metadata.metadata_id == metadata_id {
                println!("Metadata found:\n{:#?}", struct_metadata);
                CodeEditor::open_file_in_editor(
                    &struct_metadata.path,
                    Some(struct_metadata.start_line_index),
                )
                .change_context(CommandError)?;
                return Ok(());
            }
        }
        for trait_metadata in bat_metadata.source_code.traits_source_code {
            if trait_metadata.metadata_id == metadata_id {
                println!("Metadata found:\n{:#?}", trait_metadata);
                CodeEditor::open_file_in_editor(
                    &trait_metadata.path,
                    Some(trait_metadata.start_line_index),
                )
                .change_context(CommandError)?;
                return Ok(());
            }
        }
        for enum_metadata in bat_metadata.source_code.enums_source_code {
            if enum_metadata.metadata_id == metadata_id {
                println!("Metadata found:\n{:#?}", enum_metadata);
                CodeEditor::open_file_in_editor(
                    &enum_metadata.path,
                    Some(enum_metadata.start_line_index),
                )
                .change_context(CommandError)?;
                return Ok(());
            }
        }
        if let Some(trait_metadata) = bat_metadata
            .traits
            .clone()
            .into_iter()
            .find(|trait_meta| trait_meta.metadata_id == metadata_id)
        {
            println!("Metadata found is trait metadata:\n{:#?}", trait_metadata);
            return Ok(());
        }

        if let Some(entry_point_meta) = bat_metadata
            .entry_points
            .clone()
            .into_iter()
            .find(|metadata| metadata.metadata_id == metadata_id)
        {
            println!(
                "Metadata found is entrypoint metadata:\n{:#?}",
                entry_point_meta
            );
            return Ok(());
        }

        if let Some(ca_meta) = bat_metadata
            .context_accounts
            .clone()
            .into_iter()
            .find(|metadata| metadata.metadata_id == metadata_id)
        {
            println!(
                "Metadata found is context accounts metadata:\n{:#?}",
                ca_meta
            );
            return Ok(());
        }

        if let Some(func_dep_meta) = bat_metadata
            .function_dependencies
            .into_iter()
            .find(|metadata| metadata.metadata_id == metadata_id)
        {
            println!(
                "Metadata found is function dependencies metadata:\n{:#?}",
                func_dep_meta
            );
            return Ok(());
        }

        Err(Report::new(CommandError)
            .attach_printable(format!("Metadata for {} couldn't be found", metadata_id)))
    }

    fn execute_open_co(&self) -> error_stack::Result<(), CommandError> {
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
        // list to start
        if bat_auditor_config.use_code_editor {
            let options = vec!["started".green(), "finished".yellow()];
            let prompt_text = format!(
                "Do you want to open a {} or a {} file?",
                options[0], options[1]
            );
            let selection = BatDialoguer::select(prompt_text, options.clone(), None)
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
                    .open_in_editor(false, None)
                    .change_context(CommandError)?;
                if ep_parser.handler.is_some() {
                    let handler_metadata = ep_parser.handler.unwrap();
                    let instruction_file_path = handler_metadata.path;
                    let start_line_index = handler_metadata.start_line_index;
                    CodeEditor::open_file_in_editor(&instruction_file_path, Some(start_line_index))
                        .change_context(CommandError)?;
                }
                return Ok(());
            } else {
                println!("Empty {} folder", options[selection].clone());
            }
        } else {
            print!("VSCode integration not enabled");
        }
        Ok(())
    }

    fn execute_count_co_files(&self) -> error_stack::Result<(), CommandError> {
        let (to_review_count, started_count, finished_count) = self.co_counter()?;
        println!("to-review co files: {}", format!("{to_review_count}").red());
        println!("started co files: {}", format!("{started_count}").yellow());
        println!("finished co files: {}", format!("{finished_count}").green());
        println!(
            "total co files: {}",
            format!("{}", to_review_count + started_count + finished_count).purple()
        );
        Ok(())
    }

    fn co_counter(&self) -> error_stack::Result<(usize, usize, usize), CommandError> {
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
}
