use crate::batbelt::command_line::execute_command;

use crate::batbelt::metadata::BatMetadataParser;
use crate::batbelt::path::BatFolder;
use crate::batbelt::BatEnumerator;
use clap::Subcommand;

use error_stack::{Result, ResultExt};

use crate::batbelt::sonar::sonar_interactive::BatSonarInteractive;
use crate::batbelt::sonar::SonarResultType;
use crate::batbelt::templates::TemplateGenerator;
use crate::commands::{BatCommandEnumerator, CommandResult};

use super::CommandError;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum SonarCommand {
    /// Updates the functions.md and structs.md files with data
    #[default]
    Run,
}

impl BatEnumerator for SonarCommand {}

impl BatCommandEnumerator for SonarCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            SonarCommand::Run => self.execute_run(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        match self {
            SonarCommand::Run => false,
        }
    }

    fn check_correct_branch(&self) -> bool {
        match self {
            SonarCommand::Run => true,
        }
    }
}

impl SonarCommand {
    fn execute_run(&self) -> Result<(), CommandError> {
        let metadata_path = BatFolder::MetadataFolder
            .get_path(false)
            .change_context(CommandError)?;
        let metadata_cache_path = BatFolder::MetadataCacheFolder
            .get_path(false)
            .change_context(CommandError)?;
        execute_command("rm", &["-rf", &metadata_path], false)?;
        execute_command("mkdir", &[&metadata_path], false)?;
        execute_command("mkdir", &[&metadata_cache_path], false)?;
        TemplateGenerator::create_auditor_metadata_files().change_context(CommandError)?;
        TemplateGenerator::create_metadata_json().change_context(CommandError)?;
        BatSonarInteractive::SonarStart {
            sonar_result_type: SonarResultType::Struct,
        }
        .print_interactive()
        .change_context(CommandError)?;
        BatSonarInteractive::SonarStart {
            sonar_result_type: SonarResultType::Function,
        }
        .print_interactive()
        .change_context(CommandError)?;
        BatSonarInteractive::SonarStart {
            sonar_result_type: SonarResultType::Trait,
        }
        .print_interactive()
        .change_context(CommandError)?;
        BatSonarInteractive::ParseMetadata
            .print_interactive()
            .change_context(CommandError)?;
        // self.structs()?;
        // self.functions()?;
        // self.traits()?;
        Ok(())
    }

    // fn functions(&self) -> Result<(), CommandError> {
    //     let mut functions_metadata_markdown = BatMetadataType::Function
    //         .get_markdown()
    //         .change_context(CommandError)?;
    //     let functions_metadata =
    //         FunctionMetadata::get_metadata_from_program_files().change_context(CommandError)?;
    //     let functions_markdown_content = functions_metadata
    //         .into_iter()
    //         .map(|function_metadata| function_metadata.get_markdown_section_content_string())
    //         .collect::<Vec<_>>()
    //         .join("\n\n");
    //     functions_metadata_markdown.content = functions_markdown_content;
    //     functions_metadata_markdown
    //         .save()
    //         .change_context(CommandError)?;
    //     batbelt::git::create_git_commit(
    //         GitCommit::UpdateMetadata {
    //             metadata_type: BatMetadataType::Function,
    //         },
    //         None,
    //     )
    //     .unwrap();
    //     Ok(())
    // }
    //
    // fn structs(&self) -> Result<(), CommandError> {
    //     let mut structs_metadata_markdown = BatMetadataType::Struct
    //         .get_markdown()
    //         .change_context(CommandError)?;
    //     let structs_metadata =
    //         StructMetadata::get_metadata_from_program_files().change_context(CommandError)?;
    //     let structs_markdown_content = structs_metadata
    //         .into_iter()
    //         .map(|struct_metadata| struct_metadata.get_markdown_section_content_string())
    //         .collect::<Vec<_>>()
    //         .join("\n\n");
    //     structs_metadata_markdown.content = structs_markdown_content;
    //     structs_metadata_markdown
    //         .save()
    //         .change_context(CommandError)?;
    //     batbelt::git::create_git_commit(
    //         GitCommit::UpdateMetadata {
    //             metadata_type: BatMetadataType::Struct,
    //         },
    //         None,
    //     )
    //     .unwrap();
    //     Ok(())
    // }
    // fn traits(&self) -> Result<(), CommandError> {
    //     let mut traits_metadata_markdown = BatMetadataType::Trait
    //         .get_markdown()
    //         .change_context(CommandError)?;
    //     let traits_metadata =
    //         TraitMetadata::get_metadata_from_program_files().change_context(CommandError)?;
    //     let traits_markdown_content = traits_metadata
    //         .into_iter()
    //         .map(|struct_metadata| struct_metadata.get_markdown_section_content_string())
    //         .collect::<Vec<_>>()
    //         .join("\n\n");
    //     traits_metadata_markdown.content = traits_markdown_content;
    //     traits_metadata_markdown
    //         .save()
    //         .change_context(CommandError)?;
    //     batbelt::git::create_git_commit(
    //         GitCommit::UpdateMetadata {
    //             metadata_type: BatMetadataType::Trait,
    //         },
    //         None,
    //     )
    //     .unwrap();
    //     Ok(())
    // }
}
