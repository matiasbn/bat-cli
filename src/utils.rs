use std::{process::Command, str};

// Gets auditor name from branch name
pub fn get_branch_name(audit_repository_path: String) -> String {
    let git_symbolic = Command::new("git")
        .current_dir(audit_repository_path)
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
