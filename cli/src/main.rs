#![feature(core_panic)]

extern crate core;

use clap::{Parser, Subcommand};

use crate::utils::get_notes_path;

mod commands;
mod config;
mod utils;
// use serde::{Deserialize, Serialize};

pub const DEFAULT_AUDIT_NOTES_PATH: &str = "../audit-notes";
pub const TEMPLATES_FOLDER: &str = "../audit-notes/templates";
pub const DEFAULT_CONFIG_FILE_PATH: &str = "./Batman.toml";
pub const CODE_OVERHAUL_TEMPLATE_PATH: &str = "../../templates/code-overhaul.md";

#[derive(Parser, Debug)]
#[command(author, version, about = "A CLI for Solana Audit Methodology")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
// #[derive(Subcommand, Debug, Serialize, Deserialize)]
enum Commands {
    /// Creates a Batman project
    Create {
        /// An optional config file path to create the initial Batman.toml file
        config_file_relative_path: Option<String>,
    },
    /// Initializes the project from the Batman.toml config file
    Init {
        /// An optional config file path for the Batman project
        config_file_relative_path: Option<String>,
    },
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
        Commands::Create {
            config_file_relative_path,
        } => commands::create::create_project(config_file_relative_path),
        Commands::Init {
            config_file_relative_path,
        } => commands::init::initialize_notes_repo(config_file_relative_path),
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
