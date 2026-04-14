use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use clap::Subcommand;

use error_stack::ResultExt;

use crate::batbelt::metadata::BatMetadataParser;

use crate::batbelt::analytics::BatAnalytics;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum AnalyticsCommand {
    /// Creates analytics
    #[default]
    Create,
    /// Updates analytics
    Update,
}

impl BatEnumerator for AnalyticsCommand {}

impl BatCommandEnumerator for AnalyticsCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            AnalyticsCommand::Create => self.execute_create(),
            AnalyticsCommand::Update => self.execute_update(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        true
    }

    fn check_correct_branch(&self) -> bool {
        true
    }
}

impl AnalyticsCommand {
    fn execute_create(&self) -> CommandResult<()> {
        BatAnalytics::create_analytics().change_context(CommandError)?;
        // format!("Analytics created!");
        Ok(())
    }

    fn execute_update(&self) -> CommandResult<()> {
        BatAnalytics::update_analytics().change_context(CommandError)?;
        // format!("Analytics created!");
        Ok(())
    }
}
