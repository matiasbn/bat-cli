use std::{fs, process::Command};

use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use crate::{
    command_line::execute_command,
    commands::git::{check_files_not_commited, git_push},
};

pub fn full() {
    assert!(check_files_not_commited());
    println!("Executing full package publish");
    clippy();
    publish();
}

pub fn clippy() {
    assert!(check_files_not_commited());
    println!("Executing cargo clippy --fix");
    Command::new("cargo")
        .args(["clippy", "--fix"])
        .output()
        .unwrap();
    println!("Executing cargo fix");
    Command::new("cargo").args(["fix"]).output().unwrap();
    println!("Commiting clippy changes");
    create_commit(PublishCommit::Clippy, None);
}

pub fn publish() {
    assert!(check_files_not_commited());
    bump(true);
    println!("Executing cargo publish");
    Command::new("cargo").arg("publish").output().unwrap();
}

pub fn bump(push: bool) {
    assert!(check_files_not_commited());
    let prompt_text = "select the version bump:".to_string();
    let cargo_toml = fs::read_to_string("Cargo.toml").unwrap();
    let version_line_index = cargo_toml
        .lines()
        .position(|line| line.split(" = ").next().unwrap() == "version")
        .unwrap();
    let version_line = cargo_toml.lines().collect::<Vec<_>>()[version_line_index];
    let mut version = version_line.to_string().replace('\"', "");
    version = version.split("= ").last().unwrap().to_string();
    let mut version_split = version.split('.');
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
    println!("Bumping the version {new_version} on Cargo.toml");
    fs::write(
        "Cargo.toml",
        cargo_toml.replace(version_line, &format!("version = \"{new_version}\"")),
    )
    .unwrap();

    // create commit with new version
    println!("Creating commit for version bump {new_version}");
    create_commit(PublishCommit::CommitCargo, Some(vec![new_version.as_str()]));

    if push {
        println!("Pushing changes");
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
            println!("Pushing changes");
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
                vec![
                    "commit",
                    "-m",
                    format!("package: version bump {version}").as_str(),
                ],
                "error creating commit for Cargo.toml".to_string(),
            );
        }
        PublishCommit::Clippy => {
            // commit all files
            execute_command(
                "git".to_string(),
                vec!["add", "--all"],
                "error adding Cargo.toml to commit files".to_string(),
            );
            execute_command(
                "git".to_string(),
                vec![
                    "commit",
                    "-m",
                    "package: clippy commit".to_string().as_str(),
                ],
                "error creating commit for clippy".to_string(),
            );
        }
    }
}
