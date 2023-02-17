pub mod code_overhaul;
pub mod create;
pub mod finding;
pub mod init;
pub mod metadata;
pub mod miro;
// pub mod result;
pub mod sonar;
pub mod update;

use std::{error::Error, fmt, fs};

#[derive(Debug)]
pub struct CommandError;

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command error")
    }
}

impl Error for CommandError {}
