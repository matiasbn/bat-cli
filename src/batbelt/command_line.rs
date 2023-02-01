// VSCode

use std::{path::Path, process::Command, str::from_utf8};

use crate::{batbelt::bash::execute_command, config::BatConfig};

pub fn vs_code_open_file_in_current_window(path_to_file: &str) -> Result<(), String> {
    let vs_code_integration = BatConfig::get_validated_config()?
        .auditor
        .vs_code_integration;
    if vs_code_integration {
        println!(
            "Opening {} in VS Code",
            path_to_file.split("/").last().unwrap()
        );
        execute_command("code", &["-a", path_to_file]).unwrap();
    }
    Ok(())
}
