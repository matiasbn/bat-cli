use crate::batbelt::command_line::execute_command;

use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, BatMetadataType};
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

        SonarSpecificCommand::SourceCode.execute_source_code()?;
        SonarSpecificCommand::ContextAccounts.execute_context_accounts()?;
        SonarSpecificCommand::EntryPoints.execute_entry_points()?;
        SonarSpecificCommand::Traits.execute_traits()?;
        SonarSpecificCommand::FunctionDependencies.execute_function_dependencies()?;

        GitCommit::UpdateMetadataJson
            .create_commit()
            .change_context(CommandError)?;

        Ok(())
    }
}

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum SonarSpecificCommand {
    #[default]
    SourceCode,
    ContextAccounts,
    EntryPoints,
    Traits,
    FunctionDependencies,
}

impl BatEnumerator for SonarSpecificCommand {}

impl BatCommandEnumerator for SonarSpecificCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            SonarSpecificCommand::SourceCode => self.execute_source_code()?,
            SonarSpecificCommand::ContextAccounts => self.execute_context_accounts()?,
            SonarSpecificCommand::EntryPoints => self.execute_entry_points()?,
            SonarSpecificCommand::Traits => self.execute_traits()?,
            SonarSpecificCommand::FunctionDependencies => self.execute_function_dependencies()?,
        }
        GitCommit::UpdateMetadataJson
            .create_commit()
            .change_context(CommandError)?;
        Ok(())
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }

    fn check_correct_branch(&self) -> bool {
        true
    }
}

impl SonarSpecificCommand {
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
