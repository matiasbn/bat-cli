#![feature(core_panic)]
#![feature(exit_status_error)]
extern crate core;

use clap::{Parser, Subcommand};
use utils::git::GitCommit;

mod command_line;
mod commands;
mod config;
mod constants;
mod package;
mod structs;
mod utils;
use std::{error, result};

type Result<T> = result::Result<T, Box<dyn error::Error>>;

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
    /// threat modeling operations
    #[command(subcommand)]
    TM(TMActions),
    /// Miro integration
    #[command(subcommand)]
    Miro(MiroActions),
    /// Update the templates folder and the package.json of the audit repository
    Update,
    /// Commits the open_questions, smellies and threat_modeling notes
    Notes,
    /// Updates the results file in the root of the audit to show co files
    Results,
    /// Updates the metadata.md file
    #[command(subcommand)]
    Metadata(MetadataActions),
    /// Cargo publish operations, available only for dev
    #[command(subcommand)]
    Package(PackageActions),
}

#[derive(Subcommand, Debug)]
enum MetadataActions {
    /// Updates the Structs section of the metadata.md file
    Structs,
    /// Updates the Miro section of the metadata.md file
    Miro,
    // /// Updates the Functions section of the metadata.md file
    // Functions,
}

#[derive(Subcommand, Debug)]
enum MiroActions {
    /// Deploy or updates a code-overhaul frame
    Deploy,
    /// Updates a the images for a code-overhaul folder
    Images,
    /// Creates or updates the Accounts frame
    Accounts,
}
#[derive(Subcommand, Debug)]
enum TMActions {
    /// Updates the threat_modeling.md Assets/Accounts section
    Accounts,
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
    /// Opens the co file and the instruction of a started entrypoint
    Open,
}

#[derive(Subcommand, Debug)]
enum PackageActions {
    /// Bump the version
    Bump,
    /// Bump version and publish to crates.io
    Publish,
    /// run cargo clippy and commit the changes
    Format,
    /// run cargo clippy, bump the version (commit again) and publish to crates.io
    Full,
}
#[tokio::main]
async fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create => commands::create::create_project().unwrap(),
        Commands::Init => commands::init::initialize_bat_project().unwrap(),
        Commands::CO(CodeOverhaulActions::Start) => {
            commands::code_overhaul::start_code_overhaul_file()
                .await
                .unwrap()
        }
        Commands::CO(CodeOverhaulActions::Finish) => {
            commands::code_overhaul::finish_code_overhaul_file()
                .await
                .unwrap()
        }
        Commands::CO(CodeOverhaulActions::Update) => {
            commands::code_overhaul::update_code_overhaul_file().unwrap()
        }
        Commands::CO(CodeOverhaulActions::Count) => {
            commands::code_overhaul::count_co_files().unwrap()
        }
        Commands::CO(CodeOverhaulActions::Open) => {
            commands::code_overhaul::open_co().await.unwrap()
        }
        Commands::Miro(MiroActions::Deploy) => {
            commands::miro::commands::deploy_miro().await.unwrap()
        }
        Commands::Miro(MiroActions::Images) => {
            commands::miro::commands::create_co_snapshots().unwrap()
        }
        Commands::Miro(MiroActions::Accounts) => {
            commands::miro::commands::deploy_accounts().await.unwrap()
        }
        Commands::Metadata(MetadataActions::Structs) => {
            commands::metadata::update_structs().unwrap()
        }
        Commands::Metadata(MetadataActions::Miro) => commands::metadata::update_miro().unwrap(),
        Commands::TM(TMActions::Accounts) => commands::tm::update_accounts().unwrap(),
        Commands::Finding(FindingActions::Create) => commands::finding::create_finding().unwrap(),
        Commands::Finding(FindingActions::Finish) => commands::finding::finish_finding().unwrap(),
        Commands::Finding(FindingActions::Update) => commands::finding::update_finding().unwrap(),
        Commands::Finding(FindingActions::PrepareAll) => commands::finding::prepare_all().unwrap(),
        Commands::Finding(FindingActions::AcceptAll) => commands::finding::accept_all().unwrap(),
        Commands::Finding(FindingActions::Reject) => commands::finding::reject().unwrap(),
        Commands::Update => commands::update::update_repository().unwrap(),
        Commands::Notes => utils::git::create_git_commit(GitCommit::Notes, None).unwrap(),
        Commands::Results => commands::code_overhaul::update_audit_results().unwrap(),
        // only for dev
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Bump) => package::bump(false).unwrap(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Format) => package::format().unwrap(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Publish) => package::publish().unwrap(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Full) => package::full().unwrap(),
        _ => unimplemented!("Command only implemented for dev operations"),
    }
}
