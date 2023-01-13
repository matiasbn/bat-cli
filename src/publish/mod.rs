use std::{borrow::BorrowMut, fs, process::Command};

use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use crate::command_line::execute_command;

pub fn full() {
    let output = execute_command(
        "git".to_string(),
        vec!["status", "--porcelain"],
        "error running git status".to_string(),
    );
    if !output
        .split("\n")
        .map(|l| l.to_string())
        .collect::<Vec<String>>()
        .is_empty()
    {
        panic!("Commit your changes before executing this command!");
    }
    println!("{output}")
    // clippy();
    // publish();
}

pub fn clippy() {
    execute_command(
        "cargo".to_string(),
        vec!["clippy"],
        "error publishing to crates.io".to_string(),
    );
    create_commit(PublishCommit::Clippy, None);
}

pub fn publish() {
    bump_version(true);
    execute_command(
        "cargo".to_string(),
        vec!["publish"],
        "error publishing to crates.io".to_string(),
    );
}

pub fn bump_version(push: bool) {
    let prompt_text = format!("select the version bump:");
    let cargo_toml = fs::read_to_string("Cargo.toml").unwrap();
    let version_line_index = cargo_toml
        .lines()
        .position(|line| line.split(" = ").next().unwrap() == "version")
        .unwrap();
    let version_line = cargo_toml.lines().collect::<Vec<_>>()[version_line_index];
    let mut version = version_line.to_string().replace("\"", "");
    version = version.split("= ").last().unwrap().to_string();
    let mut version_split = version.split(".");
    let major = version_split.next().unwrap().parse::<i32>().unwrap();
    let minor = version_split.next().unwrap().parse::<i32>().unwrap();
    let patch = version_split.next().unwrap().parse::<i32>().unwrap();
    let options = vec![
        format!("major: {}.0.0", major + 1),
        format!("minor: {}.{}.0", major, minor + 1),
        format!("patch: {}.{}.{}", major, minor, patch + 1),
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .items(&options)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap()
        .unwrap();
    let mut version_vec = vec![major, minor, patch];
    match selection {
        0 => {
            version_vec[0] += 1;
            version_vec[1] = 0;
            version_vec[2] = 0;
        }
        1 => {
            version_vec[1] += 1;
            version_vec[2] = 0;
        }
        2 => {
            version_vec[2] += 1;
        }
        _ => panic!("Bad selection"),
    };
    let new_version = version_vec
        .iter()
        .map(|ver| ver.to_string())
        .collect::<Vec<_>>()
        .join(".");
    fs::write(
        "Cargo.toml",
        cargo_toml.replace(version_line, &format!("version = \"{new_version}\"")),
    )
    .unwrap();

    // create commit with new version
    create_commit(PublishCommit::CommitCargo, Some(vec![new_version.as_str()]));

    if push {
        git_push();
    } else {
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Do you want to push?")
            .item("yes")
            .item("no")
            .default(0)
            .interact_on_opt(&Term::stderr())
            .unwrap()
            .unwrap();
        if selection == 0 {
            git_push();
        }
    }
}

enum PublishCommit {
    CommitCargo,
    Clippy,
}

fn create_commit(commit_type: PublishCommit, commit_options: Option<Vec<&str>>) {
    match commit_type {
        PublishCommit::CommitCargo => {
            let version = commit_options.unwrap()[0];
            // git add Cargo.toml
            execute_command(
                "git".to_string(),
                vec!["add", "Cargo.toml"],
                "error adding Cargo.toml to commit files".to_string(),
            );

            execute_command(
                "git".to_string(),
                vec!["commit", "-m", format!("version bump {version}").as_str()],
                "error creating commit for Cargo.toml".to_string(),
            );
        }
        PublishCommit::Clippy => {
            execute_command(
                "git".to_string(),
                vec!["commit", "-m", format!("clippy commit").as_str()],
                "error creating commit for clippy".to_string(),
            );
        }
    }
}

fn git_push() {
    execute_command(
        "git".to_string(),
        vec!["push"],
        "error pushing commit for Cargo.toml".to_string(),
    );
}
