use error_stack::{IntoReport, Report, Result, ResultExt};
use std::{error::Error, fmt, process::Command};

#[derive(Debug)]
pub struct BashError;

impl fmt::Display for BashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command line error")
    }
}

impl Error for BashError {}

pub fn execute_command(command: &str, args: &[&str]) -> Result<(), BashError> {
    let message = format!(
        "Error spawning a child process for paramenters: \n command: {} \n args: {:#?}",
        command, args
    );
    let mut output = Command::new(command)
        .args(args)
        .spawn()
        .into_report()
        .change_context(BashError)
        .attach_printable(message)?;

    let message = format!(
        "Error waiting a child process for paramenters: \n command: {} \n args: {:#?}",
        command, args
    );

    output
        .wait()
        .into_report()
        .change_context(BashError)
        .attach_printable(message)?;
    Ok(())
}
