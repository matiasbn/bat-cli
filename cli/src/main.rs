#![feature(core_panic)]

mod commands;
mod utils;
use serde::{Deserialize, Serialize};

extern crate core;

use clap::builder::Str;
use clap::{Parser, Subcommand};
use core::panicking::panic;

use crate::utils::get_path;

const DEFAULT_PATH: &str = "./base_repository";

#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Serialize, Deserialize)]
enum Commands {
    /// Generates a code-overhaul template file in the auditor path
    #[serde(rename = "code-overhaul")]
    CodeOverhaul {
        /// The program entrypoint to analyze
        entrypoint: Option<String>,
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
        Commands::CodeOverhaul { entrypoint } => {
            commands::code_overhaul::execute(entrypoint.unwrap())
        }
        // "check" => commands::check::execute(args).unwrap()?,
        // "build" => println!("hey1"),
        // "finding" => println!("hey2"),
        _ => panic!("Bad command"),
    }
}
