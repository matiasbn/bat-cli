use error_stack::{Result, ResultExt};

// VSCode
use crate::batbelt::bash::execute_command;
use crate::commands::CommandError;
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
        execute_command("code", &["-a", path_to_file]).change_context(CommandError)?;
    }
    Ok(())
}
