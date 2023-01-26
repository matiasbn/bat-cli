// VSCode

use std::{path::Path, process::Command, str::from_utf8};

use crate::{config::BatConfig, utils::bash::execute_command_to_stdio};

pub fn vs_code_open_file_in_current_window(path_to_file: &str) -> Result<(), String> {
    let vs_code_integration = BatConfig::get_validated_config()?
        .auditor
        .vs_code_integration;
    if vs_code_integration {
        println!(
            "Opening {} in VS Code",
            path_to_file.split("/").last().unwrap()
        );
        execute_command_to_stdio("code", &["-a", path_to_file]).unwrap();
    }
    Ok(())
}

pub fn deprecated_execute_command(
    command_name: String,
    command_args: Vec<&str>,
    error_message: String,
) -> Result<String, String> {
    let output = Command::new(command_name)
        .args(command_args)
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "{}: {:?}",
            error_message,
            from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    Ok(from_utf8(output.stdout.as_slice()).unwrap().to_string())
}

pub fn canonicalize_path(path_to_canonicalize: String) -> String {
    Path::new(&(path_to_canonicalize))
        .canonicalize()
        .unwrap()
        .into_os_string()
        .into_string()
        .unwrap()
}

// "rust-analyzer.checkOnSave.command": "clippy",
// "rust-analyzer.rustfmt.extraArgs": [
//   "--fix",
//   "--allow-dirty",
//   "--allow-no-vcs"
// ]
