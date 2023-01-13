// VSCode

use std::{
    path::{Path, PathBuf},
    process::Command,
    str::from_utf8,
};

pub fn vs_code_open_file_in_current_window(path_to_file: &str) {
    let command_name = "code".to_string();
    let command_args = vec!["-a", path_to_file];
    let error_message = "git commit creation failed with error".to_string();
    execute_command(command_name, command_args, error_message);
}

pub fn execute_command(
    command_name: String,
    command_args: Vec<&str>,
    error_message: String,
) -> String {
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
    from_utf8(output.stdout.as_slice())
        .unwrap()
        .to_string()
        .clone()
}

fn canonicalize_path(path_to_canonicalize: String) -> PathBuf {
    Path::new(&(path_to_canonicalize)).canonicalize().unwrap()
}

// "rust-analyzer.checkOnSave.command": "clippy",
// "rust-analyzer.rustfmt.extraArgs": [
//   "--fix",
//   "--allow-dirty",
//   "--allow-no-vcs"
// ]
