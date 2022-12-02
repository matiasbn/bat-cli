#![feature(core_panic)]
#![feature(exit_status_error)]
extern crate core;

use clap::{Parser, Subcommand};
use config::BatConfig;

mod commands;
mod config;
mod utils;
// use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about = "Blockchain Audit Toolkit (BAT) CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
// #[derive(Subcommand, Debug, Serialize, Deserialize)]
enum Commands {
    /// Creates a Bat project
    Create,
    /// Initializes the project from the Bat.toml config file
    Init,
    /// code-overhaul files management
    // #[serde(rename = "code-overhaul")]
    #[command(subcommand)]
    CodeOverhaul(CodeOverhaulActions),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingActions),
    /// Checks the health of the files
    Check {
        /// The type of check to execute
        check_types: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum FindingActions {
    /// Creates a finding file
    Create {
        /// Finding name, the file would be named finding_name.md
        finding_name: String,
    },
    /// Prepare the findings for review
    PrepareAll,
    /// Moves all the to-review findings to accepted
    AcceptAll,
    /// Moves a finding from to-review to rejected
    Reject,
}

#[derive(Subcommand, Debug)]
enum CodeOverhaulActions {
    /// Creates a code-overhaul file
    Create {
        /// The program entrypoint to analyze
        entrypoint_name: Option<String>,
    },
    /// Moves the code-overhaul file from to-review to finished
    Finish,
}

fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create {} => commands::create::create_project(),
        Commands::Init {} => commands::init::initialize_notes_repo(),
        Commands::CodeOverhaul(CodeOverhaulActions::Create { entrypoint_name }) => {
            commands::code_overhaul::create_overhaul_file(entrypoint_name.unwrap())
        }
        Commands::Finding(FindingActions::Create { finding_name }) => {
            commands::finding::create_finding_file(finding_name)
        }
        Commands::Finding(FindingActions::PrepareAll) => commands::finding::prepare_all(),
        Commands::Finding(FindingActions::AcceptAll) => commands::finding::accept_all(),
        Commands::Finding(FindingActions::Reject) => commands::finding::reject(),
        _ => panic!("Bad command"),
    }
}
