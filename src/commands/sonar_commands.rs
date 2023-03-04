use crate::batbelt::command_line::execute_command;

use crate::batbelt::metadata::{BatMetadataParser, BatMetadataType};
use crate::batbelt::path::BatFolder;
use crate::batbelt::BatEnumerator;
use clap::Subcommand;

use crate::batbelt::git::GitCommit;
use error_stack::{Result, ResultExt};

use crate::batbelt::sonar::sonar_interactive::BatSonarInteractive;
use crate::batbelt::sonar::{BatSonarError, SonarResultType};
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

        BatSonarInteractive::GetSourceCodeMetadata
            .print_interactive()
            .change_context(CommandError)?;

        BatSonarInteractive::GetEntryPointsMetadata
            .print_interactive()
            .change_context(CommandError)?;

        BatSonarInteractive::GetTraitsMetadata
            .print_interactive()
            .change_context(CommandError)?;

        BatSonarInteractive::GetTraitsMetadata
            .print_interactive()
            .change_context(CommandError)?;

        GitCommit::UpdateMetadataJson
            .create_commit()
            .change_context(CommandError)?;
        Ok(())
    }
}
