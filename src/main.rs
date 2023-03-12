#[macro_use]
extern crate log;

extern crate confy;

use clap::{Parser, Subcommand};
use colored::Colorize;
use inflector::Inflector;

use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::path::BatFile;
use crate::commands::miro_commands::MiroCommand;
use crate::commands::sonar_commands::SonarCommand;
use crate::commands::{BatCommandEnumerator, BatPackageJsonCommand, CommandResult};

use crate::batbelt::git::GitAction;
use crate::batbelt::BatEnumerator;
use crate::commands::repository_commands::RepositoryCommand;

use commands::co_commands::CodeOverhaulCommand;
use commands::finding_commands::FindingCommand;
use commands::CommandError;
use error_stack::fmt::{Charset, ColorMode};
use error_stack::{FutureExt, IntoReport, Result};
use error_stack::{Report, ResultExt};

use crate::commands::project_commands::ProjectCommands;
use crate::commands::tools_commands::ToolCommand;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;

use log4rs::Config;
use package::PackageCommand;
use regex::Regex;
pub mod batbelt;
pub mod commands;
pub mod config;
pub mod package;

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
    /// Creates a Bat project
    #[default]
    Create,
    /// Initializes the project from the Bat.toml config file
    Init {
        /// Skips the initial commit process
        #[arg(short, long)]
        skip_initial_commit: bool,
    },
    /// Refresh the project
    Reload,
    /// code-overhaul files management
    #[command(subcommand)]
    CodeOverhaul(CodeOverhaulCommand),
    /// Execute the BatSonar to create metadata files for all Sonar result types
    Sonar {
        /// Skips the source code (functions, structs, enums and traits) process
        #[arg(long)]
        skip_source_code: bool,
        /// Runs Sonar only for Context Accounts
        #[arg(long)]
        only_context_accounts: bool,
        /// Runs Sonar only for entry points
        #[arg(long)]
        only_entry_points: bool,
        /// Runs Sonar only for traits implemenetations and definitions data
        #[arg(long)]
        only_traits: bool,
        /// Runs Sonar only for function dependencies
        #[arg(long)]
        only_function_dependencies: bool,
    },
    // /// Execute specific BatSonar commands
    // #[command(subcommand)]
    // SonarSpecific(SonarSpecificCommand),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingCommand),
    /// utils tools
    #[command(subcommand)]
    Tool(ToolCommand),
    /// Miro integration
    #[command(subcommand)]
    Miro(MiroCommand),
    /// Git actions to manage repository
    #[command(subcommand)]
    Repository(RepositoryCommand),
    /// Cargo publish operations, available only for dev
    #[command(subcommand)]
    Package(PackageCommand),
}

impl BatEnumerator for BatCommands {}

impl BatCommands {
    pub async fn execute(&self) -> Result<(), CommandError> {
        self.validate_command()?;
        match self {
            BatCommands::Create => commands::project_commands::create_bat_project(),
            BatCommands::Init {
                skip_initial_commit,
            } => commands::project_commands::initialize_bat_project(*skip_initial_commit).await,
            BatCommands::Reload => ProjectCommands::Reload.execute_command(),
            BatCommands::CodeOverhaul(command) => command.execute_command().await,
            BatCommands::Finding(FindingCommand::Create) => {
                commands::finding_commands::start_finding()
            }
            BatCommands::Finding(FindingCommand::Finish) => {
                commands::finding_commands::finish_finding()
            }
            BatCommands::Finding(FindingCommand::Update) => {
                commands::finding_commands::update_finding()
            }
            BatCommands::Finding(FindingCommand::AcceptAll) => {
                commands::finding_commands::accept_all()
            }
            BatCommands::Sonar {
                skip_source_code,
                only_context_accounts,
                only_entry_points,
                only_traits,
                only_function_dependencies,
            } => SonarCommand::Run {
                skip_source_code: *skip_source_code,
                only_context_accounts: *only_context_accounts,
                only_entry_points: *only_entry_points,
                only_traits: *only_traits,
                only_function_dependencies: *only_function_dependencies,
            }
            .execute_command(),
            // BatCommands::SonarSpecific(command) => command.execute_command(),
            BatCommands::Finding(FindingCommand::Reject) => commands::finding_commands::reject(),
            BatCommands::Miro(command) => command.execute_command().await,
            BatCommands::Tool(command) => command.execute_command(),
            BatCommands::Repository(command) => command.execute_command(),
            // only for dev
            #[cfg(debug_assertions)]
            BatCommands::Package(PackageCommand::Format) => {
                package::format().change_context(CommandError)
            }
            #[cfg(debug_assertions)]
            BatCommands::Package(PackageCommand::Release) => {
                package::release().change_context(CommandError)
            }
            _ => unimplemented!("Command only implemented for dev operations"),
        }
    }

    fn validate_command(&self) -> CommandResult<()> {
        let (check_metadata, check_branch) = match self {
            BatCommands::Create => {
                return Ok(());
            }
            BatCommands::Init { .. } => {
                return Ok(());
            }
            BatCommands::Reload => {
                return Ok(());
            }
            BatCommands::Package(_) => {
                return Ok(());
            }
            BatCommands::Sonar { .. } => (
                SonarCommand::Run {
                    skip_source_code: false,
                    only_context_accounts: false,
                    only_entry_points: false,
                    only_traits: false,
                    only_function_dependencies: false,
                }
                .check_metadata_is_initialized(),
                SonarCommand::Run {
                    skip_source_code: false,
                    only_context_accounts: false,
                    only_entry_points: false,
                    only_traits: false,
                    only_function_dependencies: false,
                }
                .check_correct_branch(),
            ),
            BatCommands::Tool(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
            BatCommands::CodeOverhaul(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
            BatCommands::Finding(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
            BatCommands::Miro(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
            BatCommands::Repository(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
        };
        if check_metadata {
            BatMetadata::read_metadata()
                .change_context(CommandError)?
                .check_metadata_is_initialized()
                .change_context(CommandError)?;
        }

        if check_branch {
            GitAction::CheckCorrectBranch
                .execute_action()
                .change_context(CommandError)?;
        }
        Ok(())
    }

    pub fn get_bat_package_json_commands() -> Vec<BatPackageJsonCommand> {
        BatCommands::get_type_vec()
            .into_iter()
            .filter_map(|command| match command {
                BatCommands::CodeOverhaul(_) => {
                    Some(CodeOverhaulCommand::get_bat_package_json_commands(
                        command.to_string().to_kebab_case(),
                    ))
                }
                BatCommands::Finding(_) => Some(FindingCommand::get_bat_package_json_commands(
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Tool(_) => Some(ToolCommand::get_bat_package_json_commands(
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Miro(_) => Some(MiroCommand::get_bat_package_json_commands(
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Repository(_) => {
                    Some(RepositoryCommand::get_bat_package_json_commands(
                        command.to_string().to_kebab_case(),
                    ))
                }
                BatCommands::Sonar { .. } => Some(SonarCommand::get_bat_package_json_commands(
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Reload => Some(BatPackageJsonCommand {
                    command_name: command.to_string().to_kebab_case(),
                    command_options: vec![],
                }),
                _ => None,
            })
            .collect::<Vec<_>>()
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

fn init_log(cli: Cli) -> CommandResult<()> {
    let bat_log_file = BatFile::Batlog;
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} [{f}:{L}] {h({l})} {M}{n}{m}{n}",
        )))
        .build(bat_log_file.get_path(false).change_context(CommandError)?)
        .into_report()
        .change_context(CommandError)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(cli.verbose.log_level_filter()),
        )
        .into_report()
        .change_context(CommandError)?;

    log4rs::init_config(config)
        .into_report()
        .change_context(CommandError)?;
    Ok(())
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
    // env_logger selectively
    match cli.command {
        BatCommands::Package(..) | BatCommands::Create => {
            env_logger::init();
            Ok(())
        }
        _ => init_log(cli.clone()),
    }?;

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
            log::error!("{:#?} error report:\n {:#?}", cli.command, error);
            Err(error)
        }
    }
}

#[cfg(debug_assertions)]
mod test_bat_main {

    #[test]
    fn test_main() {
        super::main();
    }
}
