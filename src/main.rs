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
    /// Cargo publish operations
    #[cfg(debug_assertions)]
    #[command(subcommand)]
    Package(PublishActions),
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
}

#[derive(Subcommand, Debug)]
enum PublishActions {
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
            commands::code_overhaul::start_code_overhaul_file().await
        }
        Commands::CO(CodeOverhaulActions::Finish) => {
            commands::code_overhaul::finish_code_overhaul_file()
        }
        Commands::CO(CodeOverhaulActions::Update) => {
            commands::code_overhaul::update_code_overhaul_file()
        }
        // Commands::CO(CodeOverhaulActions::Test) => commands::code_overhaul::function_to_test(),
        Commands::Finding(FindingActions::Create) => commands::finding::create_finding(),
        Commands::Finding(FindingActions::Finish) => commands::finding::finish_finding(),
        Commands::Finding(FindingActions::Update) => commands::finding::update_finding(),
        Commands::Finding(FindingActions::PrepareAll) => commands::finding::prepare_all(),
        Commands::Finding(FindingActions::AcceptAll) => commands::finding::accept_all(),
        Commands::Finding(FindingActions::Reject) => commands::finding::reject(),
        Commands::Update => commands::update::update_repository(),
        Commands::Notes => commands::git::create_git_commit(GitCommit::Notes, None),
        // only for dev
        Commands::Package(PublishActions::Bump) => publish::bump(false),
        Commands::Package(PublishActions::Clippy) => publish::clippy(),
        Commands::Package(PublishActions::Publish) => publish::publish(),
        Commands::Package(PublishActions::Full) => publish::full(),
        _ => panic!("Bad command"),
    }
}
