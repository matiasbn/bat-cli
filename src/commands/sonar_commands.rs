use crate::batbelt::metadata::{BatMetadata, BatMetadataCommit, BatMetadataParser};

use crate::batbelt::BatEnumerator;
use clap::Subcommand;

use crate::batbelt::git::GitCommit;
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
    /// Gets metadata from the source code
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
    fn execute_run(&self) -> CommandResult<()> {
        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        // backup miro metadata
        let miro_metadata = bat_metadata.miro;
        // backup co metadata

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

        self.execute_source_code()?;
        self.execute_context_accounts()?;
        self.execute_entry_points()?;
        self.execute_traits()?;
        self.execute_function_dependencies()?;

        let mut bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        bat_metadata.miro = miro_metadata;
        bat_metadata.initialized = true;
        bat_metadata.save_metadata().change_context(CommandError)?;

        GitCommit::UpdateMetadataJson {
            bat_metadata_commit: BatMetadataCommit::RunSonarMetadataCommit,
        }
        .create_commit()
        .change_context(CommandError)?;

        Ok(())
    }

    fn execute_source_code(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetSourceCodeMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }

    fn execute_context_accounts(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetContextAccountsMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }

    fn execute_entry_points(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetEntryPointsMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }

    fn execute_traits(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetTraitsMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }

    fn execute_function_dependencies(&self) -> Result<(), CommandError> {
        BatSonarInteractive::GetFunctionDependenciesMetadata
            .print_interactive()
            .change_context(CommandError)?;
        Ok(())
    }
}
