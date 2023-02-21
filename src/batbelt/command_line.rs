use error_stack::{IntoReport, Result, ResultExt};
use std::process::{ChildStdout, Command};

use crate::commands::CommandError;
use crate::config::BatAuditorConfig;

pub fn vs_code_open_file_in_current_window(path_to_file: &str) -> Result<(), CommandError> {
    let vs_code_integration = BatAuditorConfig::get_config()
        .change_context(CommandError)?
        .vs_code_integration;
    if vs_code_integration {
        println!(
            "Opening {} in VS Code",
            path_to_file.split("/").last().unwrap()
        );
        execute_command("code", &["-a", path_to_file]).change_context(CommandError)?;
    } else {
        println!("Path to file: {:#?}", path_to_file);
    }
    Ok(())
}

pub fn execute_command(command: &str, args: &[&str]) -> Result<Option<ChildStdout>, CommandError> {
    let message = format!(
        "Error spawning a child process for paramenters: \n command: {} \n args: {:#?}",
        command, args
    );
    let mut output = Command::new(command)
        .args(args)
        .spawn()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!(
        "Error waiting a child process for paramenters: \n command: {} \n args: {:#?}",
        command, args
    );

    output
        .wait()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    Ok(output.stdout)
}
