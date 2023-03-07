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
use crate::commands::{BatCommandEnumerator, CommandResult};

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
use crate::commands::tools_commands::ToolsCommands;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;

use log4rs::Config;
use package::PackageCommand;
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
    CO(CodeOverhaulCommand),
    /// Execute the BatSonar to create metadata files for all Sonar result types
    Sonar,
    // /// Execute specific BatSonar commands
    // #[command(subcommand)]
    // SonarSpecific(SonarSpecificCommand),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingCommand),
    /// utils tools
    #[command(subcommand)]
    Tools(ToolsCommands),
    /// Miro integration
    #[command(subcommand)]
    Miro(MiroCommand),
    /// Git actions to manage repository
    #[command(subcommand)]
    Repo(RepositoryCommand),
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
            BatCommands::CO(command) => command.execute_command().await,
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
            BatCommands::Sonar => SonarCommand::Run.execute_command(),
            // BatCommands::SonarSpecific(command) => command.execute_command(),
            BatCommands::Finding(FindingCommand::Reject) => commands::finding_commands::reject(),
            BatCommands::Miro(command) => command.execute_command().await,
            BatCommands::Tools(command) => command.execute_command(),
            BatCommands::Repo(command) => command.execute_command(),
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
            BatCommands::Sonar => (
                SonarCommand::Run.check_metadata_is_initialized(),
                SonarCommand::Run.check_correct_branch(),
            ),
            // BatCommands::SonarSpecific(command) => (
            //     command.check_metadata_is_initialized(),
            //     command.check_correct_branch(),
            // ),
            BatCommands::Tools(command) => (
                command.check_metadata_is_initialized(),
                command.check_correct_branch(),
            ),
            BatCommands::CO(command) => (
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
            BatCommands::Repo(command) => (
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

    pub fn get_kebab_commands() -> Vec<(Vec<String>, String)> {
        BatCommands::get_type_vec()
            .into_iter()
            .filter_map(|command| match command {
                BatCommands::CO(_) => Some((
                    CodeOverhaulCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Finding(_) => Some((
                    FindingCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Tools(_) => Some((
                    ToolsCommands::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Miro(_) => Some((
                    MiroCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                // BatCommands::SonarSpecific(_) => Some((
                //     SonarSpecificCommand::get_type_vec()
                //         .into_iter()
                //         .map(|command_type| command_type.to_string().to_kebab_case())
                //         .collect::<Vec<_>>(),
                //     command.to_string().to_kebab_case(),
                // )),
                BatCommands::Sonar => Some((vec![], command.to_string().to_kebab_case())),
                BatCommands::Repo(_) => Some((
                    RepositoryCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Reload => {
                    Some((vec![], BatCommands::Reload.to_string().to_kebab_case()))
                }
                _ => None,
            })
            .collect::<Vec<(Vec<_>, String)>>()
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
                cli.command.to_string().to_kebab_case().green()
            );
            Ok(())
        }
        Err(error) => {
            eprintln!(
                "{} {} script finished with error",
                "bat-cli".red(),
                cli.command.to_string().to_kebab_case().red()
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
