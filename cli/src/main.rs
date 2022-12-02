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
    /// Generates a code-overhaul template file in the auditor path
    // #[serde(rename = "code-overhaul")]
    CodeOverhaul {
        /// The program entrypoint to analyze
        entrypoint_name: Option<String>,
    },
    /// Generates and marks finding files as finished
    #[command(subcommand)]
    Finding(FindingActions),
    /// Checks the health of the files
    Check {
        /// The type of check to execute
        kind: Option<String>,
        path: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum FindingActions {
    /// Creates a finding file
    Create {
        /// Finding name, the file would be named finding_name.md
        finding_name: Option<String>,
    },
    /// Finishes a finding file
    Finish,
}

fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create {} => commands::create::create_project(),
        Commands::Init {} => commands::init::initialize_notes_repo(),
        Commands::CodeOverhaul { entrypoint_name } => {
            let auditor_name = BatConfig::get_config().auditor.auditor_name;
            commands::code_overhaul::create_overhaul_file(entrypoint_name.unwrap(), auditor_name)
        }
        Commands::Finding(FindingActions::Create { finding_name }) => {
            commands::finding::create_finding_file(finding_name.unwrap())
        }
        Commands::Finding(FindingActions::Finish) => {
            unimplemented!("action not implemented yet")
        }
        _ => panic!("Bad command"),
    }
}
