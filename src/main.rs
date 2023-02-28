#[macro_use]
extern crate log;

extern crate confy;

use batbelt::git::GitCommit;
use clap::{Parser, Subcommand};

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
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;
use package::PackageCommand;

mod batbelt;
mod commands;
mod config;
mod package;

// pub type BatDerive = #[derive(Debug, PartialEq, Copy, strum_macros::Display, strum_macros::EnumIter)];

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Commands,
}

#[derive(
    Default, strum_macros::Display, Subcommand, Debug, PartialEq, Clone, strum_macros::EnumIter,
)]
enum Commands {
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

impl BatEnumerator for Commands {}

impl Commands {
    pub async fn execute(&self) -> Result<(), CommandError> {
        match self {
            Commands::Create
            | Commands::Init { .. }
            | Commands::CO(_)
            | Commands::Finding(_)
            | Commands::Package(_) => {
                unimplemented!()
            }
            Commands::Miro(command) => command.execute_command().await?,
            Commands::Sonar(command) => command.execute_command()?,
            Commands::Repo(command) => command.execute_command()?,
        }
        Ok(())
    }
}

fn init_log(cli: Cli) -> CommandResult<()> {
    let bat_log_file = BatFile::Batlog;
    bat_log_file.remove_file().change_context(CommandError)?;
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

#[tokio::main]
async fn main() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    // env_logger selectively
    match cli.command {
        Commands::Package(..) | Commands::Create => {
            env_logger::init();
            Ok(())
        }
        _ => init_log(cli.clone()),
    }?;

    // check_correct_branch
    match cli.command {
        Commands::Init { .. }
        | Commands::Create
        | Commands::Package(..)
        | Commands::Repo(..)
        | Commands::Miro(..) => Ok(()),
        _ => check_correct_branch().change_context(CommandError),
    }?;

    // check metadata
    match cli.command {
        Commands::Init { .. }
        | Commands::Create
        | Commands::Package(..)
        | Commands::Repo(..)
        | Commands::Sonar(SonarCommand::Run) => Ok(()),
        _ => BatMetadata::check_metadata_is_initialized().change_context(CommandError),
    }?;

    let result = match cli.command {
        Commands::Create => commands::project_commands::create_project(),
        Commands::Init {
            skip_initial_commit,
        } => commands::project_commands::initialize_bat_project(skip_initial_commit).await,
        Commands::CO(CodeOverhaulCommand::Start) => commands::co_commands::start_co_file(),
        Commands::CO(CodeOverhaulCommand::Finish) => commands::co_commands::finish_co_file().await,
        Commands::CO(CodeOverhaulCommand::Update) => commands::co_commands::update_co_file(),
        Commands::CO(CodeOverhaulCommand::Count) => commands::co_commands::count_co_files(),
        Commands::CO(CodeOverhaulCommand::Open) => commands::co_commands::open_co(),
        Commands::Sonar(..) => cli.command.execute().await,
        Commands::Miro(..) => cli.command.execute().await,
        Commands::Repo(..) => cli.command.execute().await,
        Commands::Finding(FindingCommand::Create) => commands::finding_commands::start_finding(),
        Commands::Finding(FindingCommand::Finish) => commands::finding_commands::finish_finding(),
        Commands::Finding(FindingCommand::Update) => commands::finding_commands::update_finding(),
        Commands::Finding(FindingCommand::AcceptAll) => commands::finding_commands::accept_all(),
        Commands::Finding(FindingCommand::Reject) => commands::finding_commands::reject(),
        // only for dev
        #[cfg(debug_assertions)]
        Commands::Package(PackageCommand::Format) => package::format().change_context(CommandError),
        #[cfg(debug_assertions)]
        Commands::Package(PackageCommand::Release) => {
            package::release().change_context(CommandError)
        }
        _ => unimplemented!("Command only implemented for dev operations"),
    };
    match result {
        Ok(_) => {
            log::info!("{} script executed correctly", cli.command.to_string())
        }
        Err(error) => {
            eprintln!("{:#?} script finished with error", cli.command);
            log::error!("error output:\n {:#?}", error)
        }
    }
    Ok(())
}
