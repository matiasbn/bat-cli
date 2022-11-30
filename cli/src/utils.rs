use serde::{de::Unexpected::Str, Deserialize};
use std::fs;

use crate::DEFAULT_AUDIT_NOTES_PATH;

pub fn clone_base_repository() {}

pub fn get_path(path: Option<String>) -> String {
    match path {
        Some(audit_path) => audit_path,
        None => String::from(DEFAULT_AUDIT_NOTES_PATH),
    }
}

// pub fn get_sam_config() -> Result<SamConfig, ()> {
//     let contents = match fs::read_to_string("../SAM.toml") {
//         // If successful return the files text as `contents`.
//         // `c` is a local variable.
//         Ok(c) => c,
//         // Handle the `error` case.
//         Err(_) => {
//             // Write `msg` to `stderr`.
//             eprintln!("Could not read file `{}`", filename);
//             // Exit the program with exit code `1`.
//             exit(1);
//         }
//     };
//     let toml_str = r#"
//         global_string = "test"
//         global_integer = 5
//         [server]
//         ip = "127.0.0.1"
//         port = 80
//         [[peers]]
//         ip = "127.0.0.1"
//         port = 8080
//         [[peers]]
//         ip = "127.0.0.1"
//     "#;
//     toml::from_str(toml_str).unwrap()
// }

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
