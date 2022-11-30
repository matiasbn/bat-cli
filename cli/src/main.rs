#![feature(core_panic)]

mod commands;
mod utils;
// use serde::{Deserialize, Serialize};

extern crate core;

use clap::builder::Str;
use clap::{Parser, Subcommand};
use core::panicking::panic;

use crate::utils::get_path;

const DEFAULT_PATH: &str = "../base-repository";
const TEMPLATES_FOLDER: &str = "../templates";
const CODE_OVERHAUL_TEMPLATE_PATH: &str = "../../templates/code-overhaul.md";

#[derive(Parser, Debug)]
#[command(author, version, about = "A CLI for Solana Audit Methodology")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
// #[derive(Subcommand, Debug, Serialize, Deserialize)]
enum Commands {
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
        Commands::CodeOverhaul {
            entrypoint,
            audit_repo_path,
        } => commands::code_overhaul::execute(entrypoint.unwrap(), audit_repo_path),
        // "check" => commands::check::execute(args).unwrap()?,
        // "build" => println!("hey1"),
        // "finding" => println!("hey2"),
        _ => panic!("Bad command"),
    }
}
