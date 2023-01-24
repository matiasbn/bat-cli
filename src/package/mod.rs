use std::{fs, io, process::Command};

use dialoguer::{console::Term, theme::ColorfulTheme, Select};

use crate::{command_line::execute_command, utils::git::check_files_not_commited};

pub fn release() -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Starting the release process");
    let version = bump()?;
    release_start(&version)?;
    format()?;
    tag(&version)?;
    release_finish(&version)?;
    push_origin_all()?;
    Ok(())
}

pub fn format() -> io::Result<()> {
    // assert!(check_files_not_commited().unwrap());
    println!("Executing cargo clippy --fix");
    execute_package_fn("cargo", &["clippy", "--fix"])?;
    println!("Executing cargo fix");
    execute_package_fn("cargo", &["fix"])?;
    println!("Executing cargo fmt --all");
    execute_package_fn("cargo", &["fmt", "--all"])?;
    println!("Commiting format changes");
    create_commit(PackageCommit::Format, None);
    Ok(())
}

fn execute_package_fn(command: &str, args: &[&str]) -> io::Result<()> {
    let mut output = Command::new(command).args(args).spawn()?;
    output.wait()?;
    Ok(())
}

fn release_start(version: &str) -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Starting release for version {}", version);
    Command::new("git")
        .args(["flow", "release", "start", version])
        .output()
        .unwrap();
    Ok(())
}

fn release_finish(version: &str) -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Finishing release for version {}", version);
    Command::new("git")
        .args(["flow", "release", "finish"])
        .output()
        .unwrap();
    Ok(())
}

fn tag(version: &str) -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Creating tag for version {}", version);
    Command::new("git").args(["tag", version]).output().unwrap();
    Ok(())
}

fn bump() -> io::Result<String> {
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
    create_commit(PackageCommit::CommitCargo, Some(vec![new_version.as_str()]));

    Ok(new_version)
}

fn push_origin_all() -> io::Result<()> {
    // git push origin --all && git push origin --tags
    Command::new("git")
        .args(["push", "origin", "--all"])
        .output()
        .unwrap();
    Command::new("git")
        .args(["push", "origin", "--tags"])
        .output()
        .unwrap();
    Ok(())
}

enum PackageCommit {
    CommitCargo,
    Format,
}

fn create_commit(commit_type: PackageCommit, commit_options: Option<Vec<&str>>) {
    match commit_type {
        PackageCommit::CommitCargo => {
            let version = commit_options.unwrap()[0];
            // git add Cargo.toml
            execute_command(
                "git".to_string(),
                vec!["add", "Cargo.toml"],
                "error adding Cargo.toml to commit files".to_string(),
            )
            .unwrap();

            execute_command(
                "git".to_string(),
                vec![
                    "commit",
                    "-m",
                    format!("package: version bump {version}").as_str(),
                ],
                "error creating commit for Cargo.toml".to_string(),
            )
            .unwrap();
        }
        PackageCommit::Format => {
            // commit all files
            execute_command(
                "git".to_string(),
                vec!["add", "--all"],
                "error adding Cargo.toml to commit files".to_string(),
            )
            .unwrap();
            execute_command(
                "git".to_string(),
                vec![
                    "commit",
                    "-m",
                    "package: format commit".to_string().as_str(),
                ],
                "error creating commit for clippy".to_string(),
            )
            .unwrap();
        }
    }
}
