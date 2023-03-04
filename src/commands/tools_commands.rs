use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;

use crate::batbelt::path::BatFile;

use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use clap::Subcommand;

use error_stack::{Report, ResultExt};

use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::traits_metadata::TraitMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, BatMetadataType};

use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::BatCommands::Repo;
use log::Level;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ToolsCommands {
    /// Opens a file from metadata to code editor. If code editor is None, then prints the path
    #[default]
    Open,
    /// Customize the package.json according to certain log level
    PackageJson,
    /// Search metadata by id and type and opens on code editor
    GetMetadataById,
}

impl BatEnumerator for ToolsCommands {}

impl BatCommandEnumerator for ToolsCommands {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ToolsCommands::Open => self.execute_open(),
            ToolsCommands::PackageJson => self.execute_package_json(),
            ToolsCommands::GetMetadataById => self.execute_get_metadata(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        match self {
            ToolsCommands::Open => true,
            ToolsCommands::PackageJson => false,
            ToolsCommands::GetMetadataById => true,
        }
    }

    fn check_correct_branch(&self) -> bool {
        match self {
            ToolsCommands::Open => false,
            ToolsCommands::PackageJson => false,
            ToolsCommands::GetMetadataById => false,
        }
    }
}

impl ToolsCommands {
    fn execute_open(&self) -> CommandResult<()> {
        let selected_bat_metadata_type =
            BatMetadataType::prompt_metadata_type_selection().change_context(CommandError)?;
        let (path, start_line_index) = match selected_bat_metadata_type {
            BatMetadataType::Struct => {
                let StructMetadata {
                    path,
                    start_line_index,
                    ..
                } = StructMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
            BatMetadataType::Function => {
                let FunctionMetadata {
                    path,
                    start_line_index,
                    ..
                } = FunctionMetadata::prompt_selection().change_context(CommandError)?;
                (path, start_line_index)
            }
            BatMetadataType::Trait => {
                let TraitMetadata {
                    path,
                    start_line_index,
                    ..
                } = TraitMetadata::prompt_selection().change_context(CommandError)?;
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
                .map(|(idx, level)| ToolsCommands::colored_from_index(&level.to_string(), idx))
                .collect::<Vec<_>>(),
            None,
        )?;
        let level_selected = log_level_vec[selection];
        PackageJsonTemplate::create_package_json(Some(level_selected))
            .change_context(CommandError)?;
        BatFile::PackageJson
            .open_in_editor(false, None)
            .change_context(CommandError)
    }

    fn execute_get_metadata(&self) -> CommandResult<()> {
        let metadata_id = BatDialoguer::input("Metadata id:".to_string())?;
        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        for function_metadata in bat_metadata.source_code.functions {
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
        for struct_metadata in bat_metadata.source_code.structs {
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
        for trait_metadata in bat_metadata.source_code.traits {
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
        return Err(Report::new(CommandError)
            .attach_printable(format!("Metadata for {} couldn't be found", metadata_id)));
    }
}
