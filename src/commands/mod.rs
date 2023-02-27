pub mod code_overhaul;
pub mod create;
pub mod finding;
pub mod init;
pub mod miro_commands;
// pub mod result;
pub mod repository;
pub mod sonar_commands;

use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CommandError;

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command error")
    }
}

impl Error for CommandError {}

pub type CommandResult<T> = error_stack::Result<T, CommandError>;
