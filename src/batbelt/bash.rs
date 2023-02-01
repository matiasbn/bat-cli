use std::{io, process::Command};

pub fn execute_command(command: &str, args: &[&str]) -> io::Result<()> {
    let mut output = Command::new(command).args(args).spawn()?;
    output.wait()?;
    Ok(())
}
