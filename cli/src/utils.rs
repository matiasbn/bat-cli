use serde::Deserialize;

use crate::{DEFAULT_AUDIT_NOTES_PATH, DEFAULT_SAM_CONFIG_PATH};

#[derive(Debug, Deserialize)]
pub struct SamConfig {
    path: Option<SamPathConfig>,
}

#[derive(Debug, Deserialize)]
struct SamPathConfig {
    pub audit_folder_path: String,
    pub templates_path: String,
    pub program_path: String,
    pub program_entrypoints_path: Vec<String>,
}
pub fn clone_base_repository() {}

pub fn get_notes_path(path: Option<String>) -> String {
    match path {
        Some(audit_path) => audit_path,
        None => String::from(DEFAULT_AUDIT_NOTES_PATH),
    }
}

// pub fn get_sam_config(path: Option<String>) -> SamConfig {
//     match path {
//         Some(audit_path) => audit_path,
//         None => String::from(DEFAULT_AUDIT_NOTES_PATH),
//     }
//         // let decoded: SamConfig = toml::from_str(toml_str).unwrap();
//     // println!("{:#?}", decoded);
// }

pub fn get_sam_config_relative_path(relative_path: Option<String>) -> String {
    String::from(DEFAULT_SAM_CONFIG_PATH)
    // match relative_path {
    //     Some(sam_config_path) => sam_config_path + &String::from("/SAM.toml"),
    //     None => String::from(DEFAULT_SAM_CONFIG_PATH),
    // }
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
