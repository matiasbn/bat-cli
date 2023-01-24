use std::{fs, io, process::Command};

use crate::utils::{self, git::check_files_not_commited};

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
    println!("Executing cargo clippy --fix");
    execute_package_fn("cargo", &["clippy", "--fix"])?;
    println!("Executing cargo fix");
    execute_package_fn("cargo", &["fix", "--all"])?;
    println!("Executing cargo fmt --all");
    execute_package_fn("cargo", &["fmt", "--all"])?;
    println!("Commiting format changes");
    create_commit(PackageCommit::Format, None)?;
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
    execute_package_fn("git", &["flow", "release", "start", version])?;
    Ok(())
}

fn release_finish(version: &str) -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Finishing release for version {}", version);
    execute_package_fn("git", &["flow", "release", "finish"])?;
    Ok(())
}

fn tag(version: &str) -> io::Result<()> {
    assert!(check_files_not_commited().unwrap());
    println!("Creating tag for version {}", version);
    execute_package_fn("git", &["tag", version])?;
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
    let selection = utils::cli_inputs::select(&prompt_text, options, None).unwrap();
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
    create_commit(PackageCommit::CommitCargo, Some(vec![new_version.as_str()]))?;

    Ok(new_version)
}

fn push_origin_all() -> io::Result<()> {
    execute_package_fn("git", &["push", "origin", "--all"])?;
    execute_package_fn("git", &["push", "origin", "--tags"])?;
    Ok(())
}

enum PackageCommit {
    CommitCargo,
    Format,
}

fn create_commit(commit_type: PackageCommit, commit_options: Option<Vec<&str>>) -> io::Result<()> {
    match commit_type {
        PackageCommit::CommitCargo => {
            let version = commit_options.unwrap()[0];
            // git add Cargo.toml
            execute_package_fn("git", &["add", "Cargo.toml"])?;

            execute_package_fn(
                "git",
                &[
                    "commit",
                    "-m",
                    format!("package: version bump {version}").as_str(),
                ],
            )?;
            Ok(())
        }
        PackageCommit::Format => {
            // commit all files
            execute_package_fn("git", &["add", "--all"])?;
            execute_package_fn("git", &["commit", "-m", "package: format commit"])?;
            Ok(())
        }
    }
}
