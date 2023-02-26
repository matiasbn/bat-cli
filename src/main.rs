#[macro_use]
extern crate log;

extern crate confy;

use batbelt::git::{check_correct_branch, GitCommit};
use clap::{Parser, Subcommand};

use crate::commands::miro_commands::MiroCommand;
use crate::commands::sonar_commands::SonarCommand;
use colored::Colorize;
use commands::CommandError;
use error_stack::Result;
use error_stack::ResultExt;
use std::process;

mod batbelt;
mod commands;
mod config;
mod package;

// pub type BatDerive = #[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)];

#[derive(Parser, Debug)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    // #[clap(flatten)]
    // verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: Commands,
}

#[derive(strum_macros::Display, Subcommand, Debug)]
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

#[derive(Subcommand, Debug, strum_macros::Display)]
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

#[derive(Subcommand, Debug, strum_macros::Display)]
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

#[derive(Subcommand, Debug, strum_macros::Display)]
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

#[derive(Subcommand, Debug, strum_macros::Display)]
enum PackageCommand {
    /// run cargo clippy and commit the changes
    Format,
    /// Creates a git flow release, bumps the version, formats the code and publish
    Release,
}

#[tokio::main]
async fn main() {
    let cli: Cli = Cli::parse();
    env_logger::init();
    // env_logger::Builder::new()
    //     .filter_level(cli.verbose.log_level_filter())
    //     .init();

    let branch_checked = match cli.command {
        Commands::Init { .. }
        | Commands::Create
        | Commands::Package(..)
        | Commands::Git(..)
        | Commands::Miro(..) => Ok(()),
        _ => check_correct_branch(),
    };

    if let Err(error) = branch_checked {
        println!(
            "{} script finished with error",
            cli.command.to_string().bright_red()
        );
        log::error!("error output:\n {:#?}", error);
        process::exit(1);
        // return Ok(());
    };

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
        // Commands::Miro(MiroActions::Entrypoint { select_all, sorted }) => {
        //     commands::miro::deploy_entrypoint_screenshots_to_frame(select_all, sorted).await
        // }
        // Commands::Miro(MiroActions::Metadata {
        //     default,
        //     select_all,
        // }) => commands::miro::deploy_metadata_screenshot_to_frame(default, select_all).await,
        Commands::Finding(FindingCommand::Create) => commands::finding::create_finding(),
        Commands::Finding(FindingCommand::Finish) => commands::finding::finish_finding(),
        Commands::Finding(FindingCommand::Update) => commands::finding::update_finding(),
        Commands::Finding(FindingCommand::AcceptAll) => commands::finding::accept_all(),
        Commands::Finding(FindingCommand::Reject) => commands::finding::reject(),
        Commands::Update => commands::update::update_repository().change_context(CommandError),
        Commands::Notes => {
            batbelt::git::create_git_commit(GitCommit::Notes, None).change_context(CommandError)
        }
        // Commands::Result(ResultActions::Findings { html }) => {
        //     commands::result::findings_result(html)
        // }
        // Commands::Result(ResultActions::Commit) => commands::result::results_commit().change_context(CommandError),
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
            log::info!(
                "{} script executed correctly",
                cli.command.to_string().bright_green()
            )
        }
        Err(error) => {
            eprintln!(
                "{} script finished with error",
                cli.command.to_string().bright_red()
            );
            log::error!("error output:\n {:#?}", error)
        }
    }
}
