use error_stack::{Report, Result};
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
    let mut output = Command::new(command).args(args).spawn().or_else(|err| {
        let message = format!(
            "Error spawning a child process for paramenters: \n command: {} \n args: {:#?}",
            command, args
        );
        Err(Report::new(BashError).attach_printable(message))
    })?;
    output.wait().or_else(|err| {
        let message = format!(
            "Error waiting a child process for paramenters: \n command: {} \n args: {:#?}",
            command, args
        );
        Err(Report::new(BashError).attach_printable(message))
    })?;
    Ok(())
}
