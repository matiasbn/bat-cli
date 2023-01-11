#![feature(core_panic)]
#![feature(exit_status_error)]
extern crate core;

use clap::{Parser, Subcommand};

mod command_line;
mod commands;
mod config;
mod constants;
mod git;
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
    /// Update the templates folder
    Templates,
    // /// Checks the health of the files
    // Check {
    //     /// The type of check to execute
    //     check_types: Option<String>,
    // },
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
#[tokio::main]
async fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create {} => commands::create::create_project(),
        Commands::Init {} => commands::init::initialize_bat_project().await,
        Commands::CO(CodeOverhaulActions::Start) => {
            commands::code_overhaul::start_code_overhaul_file()
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
        Commands::Templates => commands::templates::update_templates(),
        _ => panic!("Bad command"),
    }
}
