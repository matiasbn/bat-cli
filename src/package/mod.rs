use crate::batbelt::command_line::{execute_child_process, execute_command};
use crate::batbelt::{self, git::check_files_not_committed};
use error_stack::{Result, ResultExt};
use std::fs;

use std::{error::Error, fmt};

#[derive(Debug)]
pub struct PackageError;

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Packager error")
    }
}

impl Error for PackageError {}

pub type PackageResult<T> = Result<T, PackageError>;

pub fn release() -> PackageResult<()> {
    check_files_not_committed().change_context(PackageError)?;
    println!("Starting the release process");
    let version = bump()?;
    release_start(&version)?;
    format()?;
    tag(&version)?;
    release_finish(&version)?;
    push_origin_all()?;
    publish()?;
    install()?;
    Ok(())
}

pub fn format() -> PackageResult<()> {
    check_files_not_committed().change_context(PackageError)?;
    println!("Executing cargo clippy --fix");
    execute_command("cargo", &["clippy", "--fix"], true).change_context(PackageError)?;
    println!("Executing cargo fix");
    execute_command("cargo", &["fix", "--all"], true).change_context(PackageError)?;
    println!("Executing cargo fmt --all");
    execute_command("cargo", &["fmt", "--all"], true).change_context(PackageError)?;
    println!("Committing format changes");
    create_commit(PackageCommit::Format, None)?;
    Ok(())
}

fn release_start(version: &str) -> PackageResult<()> {
    println!("Starting release for version {}", version);
    execute_command("git", &["flow", "release", "start", version], true)
        .change_context(PackageError)?;
    Ok(())
}

fn release_finish(version: &str) -> PackageResult<()> {
    println!("Finishing release for version {}", version);
    execute_command("git", &["flow", "release", "finish"], false).change_context(PackageError)?;
    Ok(())
}

fn publish() -> PackageResult<()> {
    println!("Publishing a new bat-cli version ");
    execute_child_process("cargo", &["publish"]).change_context(PackageError)?;
    Ok(())
}

fn install() -> PackageResult<()> {
    println!("Installing the published version");
    execute_child_process("cargo", &["install", "bat-cli"]).change_context(PackageError)?;
    Ok(())
}

fn tag(version: &str) -> PackageResult<()> {
    println!("Creating tag for version {}", version);
    execute_command("git", &["tag", version], false).change_context(PackageError)?;
    Ok(())
}

fn bump() -> Result<String, PackageError> {
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
    let selection = batbelt::bat_dialoguer::select(&prompt_text, options, None).unwrap();
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
        _ => unimplemented!(),
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
    create_commit(PackageCommit::CommitCargo, Some(vec![new_version.as_str()]))?;

    Ok(new_version)
}

fn push_origin_all() -> PackageResult<()> {
    execute_command("git", &["push", "origin", "--all"], false).change_context(PackageError)?;
    execute_command("git", &["push", "origin", "--tags"], false).change_context(PackageError)?;
    Ok(())
}

enum PackageCommit {
    CommitCargo,
    Format,
}

fn create_commit(
    commit_type: PackageCommit,
    commit_options: Option<Vec<&str>>,
) -> PackageResult<()> {
    match commit_type {
        PackageCommit::CommitCargo => {
            let version = commit_options.unwrap()[0];
            // git add Cargo.toml
            execute_command("git", &["add", "Cargo.toml"], false).change_context(PackageError)?;

            execute_command(
                "git",
                &[
                    "commit",
                    "-m",
                    format!("package: version bump {version}").as_str(),
                ],
                false,
            )
            .change_context(PackageError)?;
            Ok(())
        }
        PackageCommit::Format => {
            // commit all files
            execute_command("git", &["add", "--all"], false).change_context(PackageError)?;
            execute_command("git", &["commit", "-m", "package: format commit"], false)
                .change_context(PackageError)?;
            Ok(())
        }
    }
}
