pub mod code_overhaul;
pub mod create;
pub mod finding;
pub mod init;
pub mod miro;
// pub mod result;
pub mod git;
pub mod sonar;
pub mod update;

use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CommandError;

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command error")
    }
}

impl Error for CommandError {}
