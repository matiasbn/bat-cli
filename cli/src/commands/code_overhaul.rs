use crate::{get_path, CODE_OVERHAUL_TEMPLATE_PATH};

use std::path::Path;
use std::process::Command;
use std::str;
use std::string::String;

pub fn create_overhaul_file(entrypoint: String, audit_repo_path: Option<String>) {
    let repository_path = get_path(audit_repo_path);
    let branch_name = get_branch_name(repository_path.clone());
    create_code_overhaul_file(entrypoint, branch_name, repository_path);
}

fn get_branch_name(repository_path: String) -> String {
    let git_symbolic = Command::new("git")
        .current_dir(repository_path)
        .args(["symbolic-ref", "-q", "head"])
        .output();
    let output = git_symbolic.unwrap();
    let git_branch_slice = str::from_utf8(output.stdout.as_slice()).unwrap();
    let git_branch_tokenized = git_branch_slice.split('/').collect::<Vec<&str>>();
    let git_branch = git_branch_tokenized
        .last()
        .unwrap()
        .split('\n')
        .collect::<Vec<&str>>()[0];
    git_branch.to_owned()
}

fn create_code_overhaul_file(
    entrypoint: String,
    branch_name: String,
    repository_path: String,
) -> Result<(), ()> {
    let code_overhaul_path = repository_path + &String::from("/notes/") + &branch_name;
    if !Path::new(&code_overhaul_path).exists() {
        panic!(
            "{:?} auditor folder does not exist, aborting",
            code_overhaul_path
        )
    };

    let full_overhaul_path =
        code_overhaul_path + &String::from("/code-overhaul/") + &entrypoint + &String::from(".md");
    if Path::new(&full_overhaul_path).exists() {
        panic!("{:?} file already exist, aborting", entrypoint)
    };
    Command::new("cp")
        .args([CODE_OVERHAUL_TEMPLATE_PATH, &full_overhaul_path])
        .output();
    println!("Creating {:?} file", entrypoint);
    Ok(())
}

// #!/usr/bin/env bash
//
// BRANCH_NAME=$(git symbolic-ref -q HEAD)
// BRANCH_NAME=${BRANCH_NAME##refs/heads/}
// BRANCH_NAME=${BRANCH_NAME:-HEAD}
//
// FILE_PATH=notes/$BRANCH_NAME/code-overhaul
// echo $FILE_PATH
// BASENAME=$(basename $1)
// FILENAME="${BASENAME%.*}"
// FILE=$FILE_PATH/$FILENAME.md
//
// if [ ! -d $FILE_PATH ]; then
// echo "$FILE_PATH folder does not exist, aborting"
// exit 1
// elif test -f "$FILE"; then
// echo "$FILE already exist, aborting"
// exit 1
// else
// echo "Creating $FILE file"
// cp templates/code-overhaul.md $FILE
// fi
