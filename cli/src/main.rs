#![feature(core_panic)]

extern crate core;

use clap::builder::Str;
use clap::Parser;
use core::panicking::panic;
use std::fmt;
use std::process::Command;

const DEFAULT_PATH: &str = "./base_repository";

#[derive(Parser, Debug)]
struct Cli {
    /// The command to execute: check, build, finding, code-overhaul
    command: String,
    /// The path to the file to read
    option: Option<String>,
    /// The path to the file to read
    path: Option<String>,
}

fn main() {
    let args: Cli = Cli::parse();
    match args.command.as_ref() {
        "check" => {
            check(args);
        }
        // "build" => println!("hey1"),
        // "finding" => println!("hey2"),
        // "code-overhaul" => println!("hey3"),
        _ => panic!("Bad command"),
    }
}

// Functions
fn check(args: Cli) -> Result<&'static str, &'static str> {
    match args.option.clone().unwrap().as_ref() {
        "severity" => check_severity(args),
        "review" => check_review(args),
        "build" => check_build(args),
        _ => panic!("Wrong severity option"),
    }
}

fn get_path(args: Cli) -> String {
    if args.path.is_none() {
        return String::from(DEFAULT_PATH);
    }
    return String::from(args.path.unwrap());
}

fn check_severity(args: Cli) -> Result<&'static str, &'static str> {
    println!("{}", get_path(args));
    println!("check_severity");
    Ok("ok")
}

fn check_review(args: Cli) -> Result<&'static str, &'static str> {
    println!("check_review");
    Ok::<&str, _>("ok")
}

fn check_build(args: Cli) -> Result<&'static str, &'static str> {
    println!("check_build");
    Ok::<&str, _>("ok")
}

// pub enum SamCommands {
//     Check(String),
//     Build(String),
//     Finding(String),
//     CodeOverhaul(String),
// }

// impl From<(String,String)> for SamCommands {
//     fn from((command, word): (String, String)) -> Self {
//         // We use &str from command
//         match command.as_str() {
//             "check" => Self::Check(word),
//             "build" => Self::Build(word),
//             "finding" => Self::Finding(word),
//             "code-overhaul" => Self::CodeOverhaul(word),
//             _=> "error",
//         }
//     }
// }

// impl fmt::Display for SamCommands {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         match self {
//             SamCommands::Check => write!(f, "check"),
//             SamCommands::Build => write!(f, "build"),
//             SamCommands::Finding => write!(f, "finding"),
//             SamCommands::CodeOverhaul => write!(f, "code-overhaul"),
//         }
//     }
// }
