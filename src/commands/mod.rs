pub mod co_commands;
pub mod finding_commands;
pub mod miro_commands;
pub mod project_commands;
pub mod repository_commands;
pub mod sonar_commands;
pub mod utils_commands;

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

pub trait BatCommandEnumerator {
    fn execute_command(&self) -> CommandResult<()>;
    fn check_metadata_is_initialized(&self) -> bool;
    fn check_correct_branch(&self) -> bool;
}
