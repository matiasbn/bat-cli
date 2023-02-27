use error_stack::{IntoReport, Report, Result, ResultExt};
use std::io::Read;
use std::process::{ChildStdout, Command, Stdio};

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
    let mut output = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!(
        "Error waiting a child process for parameters: \n command: {} \n args: {:#?}",
        command, args
    );

    output
        .wait()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!(
        "Error reading output of child process for child_output: \n {:#?}",
        output.stdout
    );

    let mut output_string = String::new();

    output
        .stdout
        .ok_or(CommandError)
        .into_report()
        .attach_printable(message.clone())?
        .read_to_string(&mut output_string)
        .ok()
        .ok_or(CommandError)
        .into_report()
        .attach_printable(message.clone())?;

    if print_output {
        println!("{}", output_string);
    }

    Ok(output_string)
}
