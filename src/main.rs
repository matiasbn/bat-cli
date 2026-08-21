#[macro_use]
extern crate log;

extern crate confy;

use clap::{Parser, Subcommand};
use colored::Colorize;
use inflector::Inflector;

use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::path::BatFile;
use crate::commands::sonar_commands::SonarCommand;
use crate::commands::{BatCommandEnumerator, CommandResult};

use crate::batbelt::BatEnumerator;

use commands::CommandError;
use error_stack::fmt::{Charset, ColorMode};
use error_stack::{IntoReport, Result};
use error_stack::{Report, ResultExt};

use crate::commands::project_commands::ProjectCommands;

use regex::Regex;

pub mod batbelt;
pub mod commands;
pub mod config;
pub mod guide;

// pub type BatDerive = #[derive(Debug, PartialEq, Copy, strum_macros::Display, strum_macros::EnumIter)];

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: BatCommands,
}

#[derive(
    Default, strum_macros::Display, Subcommand, Debug, PartialEq, Clone, strum_macros::EnumIter,
)]
enum BatCommands {
    /// Set up a bat project here: detect the framework, create the Miro board
    /// and scan the source code
    #[default]
    Init,
    /// Authorize bat-cli against Miro once, for every project on this machine
    Login {
        /// Register the Miro app credentials before authorizing
        #[arg(long)]
        setup: bool,
        /// Show who the stored token belongs to, and its scopes
        #[arg(long)]
        status: bool,
        /// Re-authorize even if a valid token is already stored
        #[arg(long)]
        force: bool,
    },
    /// Revoke the stored Miro token and forget it
    Logout,
    /// Update bat-cli to the latest published version
    Update {
        /// Only report whether a newer version exists
        #[arg(long)]
        check: bool,
        /// Reinstall even when already up to date
        #[arg(long)]
        force: bool,
    },
    /// Show or edit the machine-wide preferences (~/.config/bat-cli/config.toml)
    Config {
        /// Re-answer the preferences instead of only printing them
        #[arg(long)]
        edit: bool,
    },
    /// Rescan the source code after it changed, rebuilding the metadata that
    /// deploy reads
    Sonar,
    /// Regenerate the machine-global AI guide and reinstall the assistant skills.
    ///
    /// Hidden because `main::run` already does this before every command; it exists
    /// so a freshly installed binary can be asked to publish ITS OWN guide — see
    /// `update_commands`, where the running process is the outgoing version.
    #[command(name = "refresh-ai-guide", hide = true)]
    RefreshAiGuide,
    /// Deploy an entry point's screenshots to a Miro board
    Deploy {
        /// Entry point to deploy, as `name` or `Contract.name`. Omit to pick
        /// one from a list.
        #[arg(long)]
        entry_point: Option<String>,
        /// Deploy every entry point at once. Not recommended: a real project
        /// puts thousands of objects on the board, and review happens one entry
        /// point at a time.
        #[arg(long)]
        all: bool,
        /// Stop expanding the call graph past this depth. Unset follows it to
        /// the end
        #[arg(long)]
        max_depth: Option<usize>,
        /// Cap the screenshots per frame. Unset draws the whole tree
        #[arg(long)]
        max_nodes: Option<usize>,
        /// Print the computed layout without sending anything to Miro
        #[arg(long)]
        dry_run: bool,
        /// Include contracts coming from lib/
        #[arg(long)]
        include_external: bool,
        /// Write a local PNG preview of the composed frame to this path
        #[arg(long)]
        preview: Option<String>,
        /// Connector thickness in dp (1-24)
        #[arg(long, default_value_t = 8)]
        stroke_width: u32,
    },
}

impl BatEnumerator for BatCommands {}

impl BatCommands {
    pub async fn execute(&self) -> Result<(), CommandError> {
        self.validate_command()?;
        match self {
            BatCommands::Init => ProjectCommands::Init.init_bat_project().await,
            BatCommands::Login {
                setup,
                status,
                force,
            } => {
                use crate::batbelt::miro::auth;
                if *status {
                    auth::status().await.change_context(CommandError)
                } else {
                    auth::login(*setup, *force).await.change_context(CommandError)
                }
            }
            BatCommands::Logout => crate::batbelt::miro::auth::logout()
                .await
                .change_context(CommandError),
            BatCommands::Update { check, force } => {
                crate::commands::update_commands::run(*check, *force).await
            }
            BatCommands::Config { edit } => {
                ProjectCommands::show_global_config(*edit).change_context(CommandError)
            }
            BatCommands::Sonar => SonarCommand::Run.execute_command(),
            // The refresh already ran in `main::run`; nothing left to do.
            BatCommands::RefreshAiGuide => Ok(()),
            BatCommands::Deploy {
                entry_point,
                all,
                max_depth,
                max_nodes,
                dry_run,
                include_external,
                preview,
                stroke_width,
            } => {
                crate::batbelt::evm::miro::auto_deploy::run(
                crate::batbelt::evm::miro::auto_deploy::AutoDeployOptions {
                    entry_point: entry_point.clone(),
                    all: *all,
                    max_depth: *max_depth,
                    max_nodes: *max_nodes,
                    dry_run: *dry_run,
                    include_external: *include_external,
                    preview: preview.clone(),
                    stroke_width: *stroke_width,
                    },
                )
                .await
                .change_context(CommandError)
            }
        }
    }

    /// Every command that reads project data needs the metadata cache to exist.
    ///
    /// There is no branch check any more: bat-cli does not create commits or
    /// manage git, so it has no business dictating which branch you are on.
    fn validate_command(&self) -> CommandResult<()> {
        let check_metadata = match self {
            BatCommands::Init
            | BatCommands::Login { .. }
            | BatCommands::Logout
            | BatCommands::Config { .. }
            | BatCommands::RefreshAiGuide
            | BatCommands::Update { .. } => return Ok(()),
            BatCommands::Sonar => false,
            BatCommands::Deploy { .. } => true,
        };

        if check_metadata {
            let bat_config = crate::config::BatConfig::get_config().change_context(CommandError)?;
            if bat_config.project_type == crate::config::ProjectType::Foundry {
                crate::batbelt::evm::metadata::bat_metadata::EvmBatMetadata::read_metadata()
                    .change_context(CommandError)?;
            } else {
                BatMetadata::read_metadata()
                    .change_context(CommandError)?
                    .check_metadata_is_initialized()
                    .change_context(CommandError)?;
            }
        }
        Ok(())
    }

    pub fn get_pretty_command(&self) -> CommandResult<String> {
        let multi_line_command_regex = Regex::new(r#"[\w]+(\([\w\s,]+\))+"#)
            .into_report()
            .change_context(CommandError)?;
        let command_string = format!("{self:#?}");
        if multi_line_command_regex.is_match(&command_string) {
            let mut command_string_lines = command_string.lines();
            let command_name = command_string_lines.next().unwrap().to_kebab_case();
            let command_option = command_string_lines.next().unwrap().trim().to_kebab_case();
            return Ok(format!("{} {}", command_name, command_option));
        }
        Ok(self.to_string().to_kebab_case())
    }
}


pub struct Suggestion(String);

impl Suggestion {
    pub fn set_report() {
        Report::set_charset(Charset::Utf8);
        Report::set_color_mode(ColorMode::Color);
        Report::install_debug_hook::<Self>(|Self(value), context| {
            context.push_body(format!("{}: {value}", "suggestion".yellow()))
        });
    }
}


async fn run() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    Suggestion::set_report();

    // Logs go to stderr, controlled by -v/-q or RUST_LOG, rather than to a file
    // inside the project that nobody read.
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .parse_default_env()
        .format_timestamp(None)
        .init();

    // The AI guide describes the binary, so every command is a chance to make sure the one
    // on disk is the one just installed — including `config` and `update`, which have no
    // project at all. Best-effort by construction: it never fails the command.
    crate::guide::refresh_ai_surface();

    cli.command.execute().await
}

#[tokio::main]
async fn main() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    match run().await {
        Ok(_) => {
            println!(
                "{} {} script successfully executed!",
                "bat-cli".green(),
                cli.command.get_pretty_command()?.green()
            );
            Ok(())
        }
        Err(error) => {
            eprintln!(
                "{} {} script finished with error",
                "bat-cli".red(),
                cli.command.get_pretty_command()?.red()
            );
            Err(error)
        }
    }
}
