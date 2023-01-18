#![feature(core_panic)]
#![feature(exit_status_error)]
extern crate core;

use clap::{Parser, Subcommand};
use commands::git::GitCommit;

mod command_line;
mod commands;
mod config;
mod constants;
mod publish;
// use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Creates a Bat project
    Create,
    /// Initializes the project from the Bat.toml config file
    Init,
    /// code-overhaul files management
    #[command(subcommand)]
    CO(CodeOverhaulActions),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingActions),
    /// Update the templates folder and the package.json of the audit repository
    Update,
    /// Commits the open_questions, smellies and threat_modeling notes
    Notes,
    /// Cargo publish operations, available only for dev
    #[command(subcommand)]
    Package(PackageActions),
}

#[derive(Subcommand, Debug)]
enum FindingActions {
    /// Creates a finding file
    Create,
    /// Finish a finding file by creating a commit
    Finish,
    /// Update a finding file by creating a commit
    Update,
    /// Prepare the findings for review
    PrepareAll,
    /// Moves all the to-review findings to accepted
    AcceptAll,
    /// Moves a finding from to-review to rejected
    Reject,
}

#[derive(Subcommand, Debug)]
enum CodeOverhaulActions {
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
}

#[derive(Subcommand, Debug)]
enum PackageActions {
    /// Bump the version
    Bump,
    /// Bump version and publish to crates.io
    Publish,
    /// run cargo clippy and commit the changes
    Clippy,
    /// run cargo clippy, bump the version (commit again) and publish to crates.io
    Full,
}
#[tokio::main]
async fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create => commands::create::create_project(),
        Commands::Init => commands::init::initialize_bat_project(),
        Commands::CO(CodeOverhaulActions::Start) => {
            commands::code_overhaul::start_code_overhaul_file()
        }
        Commands::CO(CodeOverhaulActions::Finish) => {
            commands::code_overhaul::finish_code_overhaul_file().await
        }
        Commands::CO(CodeOverhaulActions::Update) => {
            commands::code_overhaul::update_code_overhaul_file()
        }
        Commands::CO(CodeOverhaulActions::Count) => commands::code_overhaul::count_co_files(),
        Commands::CO(CodeOverhaulActions::Miro) => commands::code_overhaul::deploy_miro().await,

        Commands::Finding(FindingActions::Create) => commands::finding::create_finding(),
        Commands::Finding(FindingActions::Finish) => commands::finding::finish_finding(),
        Commands::Finding(FindingActions::Update) => commands::finding::update_finding(),
        Commands::Finding(FindingActions::PrepareAll) => commands::finding::prepare_all(),
        Commands::Finding(FindingActions::AcceptAll) => commands::finding::accept_all(),
        Commands::Finding(FindingActions::Reject) => commands::finding::reject(),
        Commands::Update => commands::update::update_repository(),
        Commands::Notes => commands::git::create_git_commit(GitCommit::Notes, None),
        // only for dev
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Bump) => publish::bump(false),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Clippy) => publish::clippy(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Publish) => publish::publish(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Full) => publish::full(),
        _ => unimplemented!("Command only implemented for dev opetions"),
    }
}
