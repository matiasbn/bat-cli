use crate::commands::CommandError;
use error_stack::{IntoReport, Result, ResultExt};
use std::process::Command;

pub fn execute_command(command: &str, args: &[&str]) -> Result<(), CommandError> {
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
    Ok(())
}
