use error_stack::{IntoReport, Report, Result, ResultExt};
use std::io::Read;
use std::process::{ChildStdout, Command, Stdio};
use std::str::from_utf8;

use crate::commands::{CommandError, CommandResult};
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
        execute_command("code", &["-a", path_to_file], false).change_context(CommandError)?;
    } else {
        println!("Path to file: {:#?}", path_to_file);
    }
    Ok(())
}

pub fn execute_command(command: &str, args: &[&str], print_output: bool) -> CommandResult<String> {
    let message = format!(
        "Error spawning a child process for parameters: \n command: {} \n args: {:#?}",
        command, args
    );

    let output = Command::new(command)
        .args(args)
        .output()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!(
        "Error reading parsing output to string: \n {:#?}",
        output.stdout
    );

    let output_string = from_utf8(output.stdout.as_slice())
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?
        .to_string();

    log::debug!("output_string: \n{}", output_string);

    if print_output {
        println!("{}", output_string);
    }

    Ok(output_string)
}
