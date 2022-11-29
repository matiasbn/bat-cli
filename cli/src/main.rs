#![feature(core_panic)]

mod commands;
mod utils;

extern crate core;

use clap::builder::Str;
use clap::Parser;
use core::panicking::panic;

use crate::utils::utils::get_path;

const DEFAULT_PATH: &str = "./base_repository";

#[derive(Parser, Debug)]
struct Cli {
    /// The command to execute: check, build, finding, code-overhaul
    command: String,
    /// The path to the file to read
    option: Option<String>,
    /// The path to the file to read
    parameter: Option<String>,
}

fn main() {
    let args: Cli = Cli::parse();
    match args.command.as_ref() {
        "code-overhaul" => commands::code_overhaul::execute(args).unwrap()?,
        "check" => commands::check::execute(args).unwrap()?,
        // "build" => println!("hey1"),
        // "finding" => println!("hey2"),
        _ => panic!("Bad command"),
    }
}
