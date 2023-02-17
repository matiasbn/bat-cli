use error_stack::{Result, ResultExt};

// VSCode
use crate::{batbelt::bash::execute_command, config::BatConfig};
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CommandLineError;

impl fmt::Display for CommandLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command line error")
    }
}

impl Error for CommandLineError {}

pub fn vs_code_open_file_in_current_window(path_to_file: &str) -> Result<(), CommandLineError> {
    let vs_code_integration = BatConfig::get_validated_config()
        .change_context(CommandLineError)?
        .auditor
        .vs_code_integration;
    if vs_code_integration {
        println!(
            "Opening {} in VS Code",
            path_to_file.split("/").last().unwrap()
        );
        execute_command("code", &["-a", path_to_file]).change_context(CommandLineError)?;
    }
    Ok(())
}
