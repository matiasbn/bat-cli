#![feature(core_panic)]
#![feature(exit_status_error)]

extern crate core;

use clap::{Parser, Subcommand};

mod commands;
mod config;
mod utils;
// use serde::{Deserialize, Serialize};

#[derive(Parser, Debug)]
#[command(author, version, about = "A CLI for Solana Audit Methodology")]
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
        entrypoint: Option<String>,
        /// The program entrypoint to analyze
        audit_repo_path: Option<String>,
    },
    /// Checks the health of the files
    Check {
        /// The type of check to execute
        kind: Option<String>,
        path: Option<String>,
    },
}

fn main() {
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Create {} => commands::create::create_project(),
        Commands::Init {} => commands::init::initialize_notes_repo(),
        Commands::CodeOverhaul {
            entrypoint,
            audit_repo_path,
        } => commands::code_overhaul::create_overhaul_file(entrypoint.unwrap(), audit_repo_path),
        // "check" => commands::check::execute(args).unwrap()?,
        // "build" => println!("hey1"),
        // "finding" => println!("hey2"),
        _ => panic!("Bad command"),
    }
}
