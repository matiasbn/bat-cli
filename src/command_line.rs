// VSCode

use std::{process::Command, str::from_utf8, path::{Path, PathBuf}};

pub fn vs_code_open_file_in_current_window(path_to_file: String) {
    let command_name = "code".to_string();
    let canonical_path = canonicalize_path(path_to_file);
    let command_args = vec!["-a", canonical_path.to_str().unwrap() ];
    let error_message = "git commit creation failed with error".to_string();
    execute_command(command_name, command_args, error_message);
}

fn execute_command(command_name: String, command_args: Vec<&str>, error_message: String) {
    println!("{:?}",command_args);
    let output = Command::new(command_name).args(command_args).output().unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "{}: {:?}",
            error_message,
            from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
}

fn canonicalize_path(path_to_canonicalize: String)-> PathBuf{
    Path::new(&(path_to_canonicalize))
    .canonicalize()
    .unwrap()
}
