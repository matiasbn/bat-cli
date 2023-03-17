use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use std::env;

use crate::batbelt::path::{prettify_source_code_path, BatFile, BatFolder};

use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use clap::Subcommand;
use colored::{ColoredString, Colorize};

use error_stack::{Report, ResultExt};
use lazy_regex::regex;

use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::structs_source_code_metadata::StructSourceCodeMetadata;
use crate::batbelt::metadata::traits_source_code_metadata::TraitSourceCodeMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, BatMetadataType};

use crate::batbelt::templates::package_json_template::PackageJsonTemplate;

use crate::batbelt;
use crate::batbelt::analytics::BatAnalytics;
use crate::batbelt::metadata::enums_source_code_metadata::EnumSourceCodeMetadata;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::config::BatAuditorConfig;
use log::Level;
use tabled::object::Rows;
use tabled::{Modify, Panel, Style, Table, Tabled, Width};

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum AnalyticsCommand {
    /// Opens a file from source code metadata to code editor. If code editor is None, then prints the path
    #[default]
    Create,
}

impl BatEnumerator for AnalyticsCommand {}

impl BatCommandEnumerator for AnalyticsCommand {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            AnalyticsCommand::Create => self.execute_create(),
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
}
