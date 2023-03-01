#[macro_use]
extern crate log;

extern crate confy;

use clap::{Parser, Subcommand};
use inflector::Inflector;

use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::path::BatFile;
use crate::commands::miro_commands::MiroCommand;
use crate::commands::sonar_commands::SonarCommand;
use crate::commands::CommandResult;

use crate::batbelt::git::check_correct_branch;
use crate::batbelt::BatEnumerator;
use crate::commands::repository_commands::RepositoryCommand;

use commands::co_commands::CodeOverhaulCommand;
use commands::finding_commands::FindingCommand;
use commands::CommandError;
use error_stack::ResultExt;
use error_stack::{FutureExt, IntoReport, Result};

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
    /// code-overhaul files management
    #[command(subcommand)]
    CO(CodeOverhaulCommand),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingCommand),
    /// Miro integration
    #[command(subcommand)]
    Miro(MiroCommand),
    /// Sonar actions
    #[command(subcommand)]
    Sonar(SonarCommand),
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
        match self {
            BatCommands::Create
            | BatCommands::Init { .. }
            | BatCommands::CO(_)
            | BatCommands::Finding(_)
            | BatCommands::Package(_) => {
                unimplemented!()
            }
            BatCommands::Miro(command) => command.execute_command().await?,
            BatCommands::Sonar(command) => command.execute_command()?,
            BatCommands::Repo(command) => command.execute_command()?,
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
                BatCommands::Miro(_) => Some((
                    MiroCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Sonar(_) => Some((
                    SonarCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
                BatCommands::Repo(_) => Some((
                    RepositoryCommand::get_type_vec()
                        .into_iter()
                        .map(|command_type| command_type.to_string().to_kebab_case())
                        .collect::<Vec<_>>(),
                    command.to_string().to_kebab_case(),
                )),
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
        .ok()
        .ok_or(CommandError)?;

    let config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            Root::builder()
                .appender("logfile")
                .build(cli.verbose.log_level_filter()),
        )
        .ok()
        .ok_or(CommandError)?;

    log4rs::init_config(config)
        .into_report()
        .change_context(CommandError)?;
    Ok(())
}

async fn run() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    // env_logger selectively
    match cli.command {
        BatCommands::Package(..) | BatCommands::Create => {
            env_logger::init();
            Ok(())
        }
        _ => init_log(cli.clone()),
    }?;

    // check_correct_branch
    match cli.command {
        BatCommands::Init { .. }
        | BatCommands::Create
        | BatCommands::Package(..)
        | BatCommands::Repo(..)
        | BatCommands::Miro(..) => Ok(()),
        _ => check_correct_branch().change_context(CommandError),
    }?;

    // check metadata
    match cli.command {
        BatCommands::Init { .. }
        | BatCommands::Create
        | BatCommands::Package(..)
        | BatCommands::Repo(..)
        | BatCommands::Sonar(SonarCommand::Run) => Ok(()),
        _ => BatMetadata::check_metadata_is_initialized().change_context(CommandError),
    }?;

    let result = match cli.command {
        BatCommands::Create => commands::project_commands::create_project(),
        BatCommands::Init {
            skip_initial_commit,
        } => commands::project_commands::initialize_bat_project(skip_initial_commit).await,
        BatCommands::CO(CodeOverhaulCommand::Start) => commands::co_commands::start_co_file(),
        BatCommands::CO(CodeOverhaulCommand::Finish) => {
            commands::co_commands::finish_co_file().await
        }
        BatCommands::CO(CodeOverhaulCommand::Update) => commands::co_commands::update_co_file(),
        BatCommands::CO(CodeOverhaulCommand::Count) => commands::co_commands::count_co_files(),
        BatCommands::CO(CodeOverhaulCommand::Open) => commands::co_commands::open_co(),
        BatCommands::Sonar(..) => cli.command.execute().await,
        BatCommands::Miro(..) => cli.command.execute().await,
        BatCommands::Repo(..) => cli.command.execute().await,
        BatCommands::Finding(FindingCommand::Create) => commands::finding_commands::start_finding(),
        BatCommands::Finding(FindingCommand::Finish) => {
            commands::finding_commands::finish_finding()
        }
        BatCommands::Finding(FindingCommand::Update) => {
            commands::finding_commands::update_finding()
        }
        BatCommands::Finding(FindingCommand::AcceptAll) => commands::finding_commands::accept_all(),
        BatCommands::Finding(FindingCommand::Reject) => commands::finding_commands::reject(),
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
    };

    return match result {
        Ok(_) => {
            println!("{:#?} script finished with error", cli.command);
            Ok(())
        }
        Err(error) => {
            eprintln!("{:#?} script finished with error", cli.command);
            log::error!("error output:\n {:#?}", error);
            Err(error)
        }
    };
}

#[tokio::main]
async fn main() {
    match run().await {
        Ok(_) => std::process::exit(0),
        Err(_) => std::process::exit(0),
    };
}
