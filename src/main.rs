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

use commands::CommandError;
use error_stack::ResultExt;
use error_stack::{FutureExt, IntoReport, Result};
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Config;

mod batbelt;
mod commands;
mod config;
mod package;

// pub type BatDerive = #[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)];

#[derive(Parser, Debug, PartialEq)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    // #[clap(flatten)]
    // verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Commands,
}

#[derive(strum_macros::Display, Subcommand, Debug, PartialEq)]
enum Commands {
    /// Creates a Bat project
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
    Git(GitCommand),
    /// Update the templates folder and the package.json of the audit repository
    Update,
    /// Commits the open_questions, smellies and threat_modeling notes
    Notes,
    /// Cargo publish operations, available only for dev
    #[command(subcommand)]
    Package(PackageCommand),
}

impl Commands {
    pub async fn execute(&self) -> Result<(), CommandError> {
        match self {
            Commands::Create
            | Commands::Init { .. }
            | Commands::CO(_)
            | Commands::Finding(_)
            | Commands::Update
            | Commands::Notes
            | Commands::Git(_)
            | Commands::Package(_) => {
                unimplemented!()
            }
            Commands::Miro(command) => command.execute_command().await?,
            Commands::Sonar(command) => command.execute_command()?,
        }
        Ok(())
    }
}

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq)]
enum GitCommand {
    /// Merges all the branches into develop branch, and then merge develop into the rest of the branches
    UpdateBranches,
    /// Delete local branches
    DeleteLocalBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Fetch remote branches
    FetchRemoteBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
}

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq)]
enum FindingCommand {
    /// Creates a finding file
    Create,
    /// Finish a finding file by creating a commit
    Finish,
    /// Update a finding file by creating a commit
    Update,
    /// Moves all the to-review findings to accepted
    AcceptAll,
    /// Moves a finding from to-review to rejected
    Reject,
}

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq)]
enum CodeOverhaulCommand {
    /// Starts a code-overhaul file audit
    Start,
    /// Moves the code-overhaul file from to-review to finished
    Finish,
    /// Update a code-overhaul file by creating a commit
    Update,
    /// Copies the images to the co Miro frame
    Miro,
    /// Counts the to-review, started, finished and total co files
    Count,
    /// Opens the co file and the instruction of a started entrypoint
    Open,
    /// Updates the templates in to-review folder
    Templates,
}

#[derive(Subcommand, Debug, strum_macros::Display, PartialEq)]
enum PackageCommand {
    /// run cargo clippy and commit the changes
    Format,
    /// Creates a git flow release, bumps the version, formats the code and publish
    Release,
}

fn init_log() -> CommandResult<()> {
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
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))
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
    match cli.command {
        Commands::Package(..) | Commands::Create => {
            env_logger::init();
            Ok(())
        }
        _ => init_log(),
    }?;

    match cli.command {
        Commands::Init { .. }
        | Commands::Create
        | Commands::Package(..)
        | Commands::Git(..)
        | Commands::Sonar(SonarCommand::Run) => Ok(()),
        _ => BatMetadata::check_metadata_is_initialized().change_context(CommandError),
    }?;

    let result = match cli.command {
        Commands::Git(GitCommand::UpdateBranches) => {
            commands::git::GitCommand::UpdateBranches.execute()
        }
        Commands::Git(GitCommand::DeleteLocalBranches { select_all }) => {
            commands::git::GitCommand::DeleteLocalBranches { select_all }.execute()
        }
        Commands::Git(GitCommand::FetchRemoteBranches { select_all }) => {
            commands::git::GitCommand::FetchRemoteBranches { select_all }.execute()
        }
        Commands::Create => commands::create::create_project(),
        Commands::Init {
            skip_initial_commit,
        } => commands::init::initialize_bat_project(skip_initial_commit).await,
        Commands::CO(CodeOverhaulCommand::Start) => commands::code_overhaul::start::start_co_file(),
        Commands::CO(CodeOverhaulCommand::Finish) => {
            commands::code_overhaul::finish::finish_co_file().await
        }
        Commands::CO(CodeOverhaulCommand::Update) => {
            commands::code_overhaul::update::update_co_file()
        }
        Commands::CO(CodeOverhaulCommand::Count) => commands::code_overhaul::count_co_files(),
        Commands::CO(CodeOverhaulCommand::Open) => commands::code_overhaul::open_co(),
        Commands::CO(CodeOverhaulCommand::Templates) => {
            commands::code_overhaul::update_co_templates()
        }
        Commands::Sonar(..) => cli.command.execute().await,
        Commands::Miro(..) => cli.command.execute().await,
        Commands::Finding(FindingCommand::Create) => commands::finding::start_finding(),
        Commands::Finding(FindingCommand::Finish) => commands::finding::finish_finding(),
        Commands::Finding(FindingCommand::Update) => commands::finding::update_finding(),
        Commands::Finding(FindingCommand::AcceptAll) => commands::finding::accept_all(),
        Commands::Finding(FindingCommand::Reject) => commands::finding::reject(),
        Commands::Update => commands::update::update_repository().change_context(CommandError),
        Commands::Notes => GitCommit::Notes
            .create_commit()
            .change_context(CommandError),
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
            eprintln!("{} script finished with error", cli.command);
            log::error!("error output:\n {:#?}", error)
        }
    }
    Ok(())
}
