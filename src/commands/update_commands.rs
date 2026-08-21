//! `bat-cli update`: bring the installed binary up to the latest published
//! version.
//!
//! Everything here goes through `cargo install`, which is how bat-cli is
//! distributed. The one non-obvious part is `--locked`: without it, `cargo
//! install` re-resolves the dependency graph from scratch and can pull
//! transitive crates that require a newer rustc than the published `Cargo.lock`
//! pins, so an update fails on a toolchain that builds the same version fine.

use std::process::{Command, Stdio};

use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use serde_json::Value;

use crate::commands::{CommandError, CommandResult};

const CRATE_NAME: &str = "bat-cli";
const REGISTRY_URL: &str = "https://crates.io/api/v1/crates/bat-cli";

/// The version this binary was built as.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Latest version published to crates.io.
pub async fn latest_version() -> CommandResult<String> {
    // crates.io rejects requests without a descriptive user agent.
    let response = reqwest::Client::new()
        .get(REGISTRY_URL)
        .header(
            "User-Agent",
            format!("bat-cli/{} (https://github.com/matiasbn/bat-cli)", current_version()),
        )
        .send()
        .await
        .into_report()
        .change_context(CommandError)
        .attach_printable("cannot reach crates.io")?;

    let status = response.status();
    let body = response
        .text()
        .await
        .into_report()
        .change_context(CommandError)?;
    if !status.is_success() {
        return Err(Report::new(CommandError)
            .attach_printable(format!("crates.io answered {status}: {body}")));
    }

    let json: Value = serde_json::from_str(&body)
        .into_report()
        .change_context(CommandError)
        .attach_printable("malformed response from crates.io")?;

    json["crate"]["max_stable_version"]
        .as_str()
        .or_else(|| json["crate"]["max_version"].as_str())
        .map(|version| version.to_string())
        .ok_or_else(|| {
            Report::new(CommandError).attach_printable("crates.io returned no version")
        })
}

/// Run the update.
///
/// `check` only reports. `force` reinstalls even when already up to date, which
/// is the way to recover a broken install.
pub async fn run(check: bool, force: bool) -> CommandResult<()> {
    let current = current_version();
    println!("Installed: {}", current.green());

    let latest = latest_version().await?;
    println!("Latest:    {}", latest.green());

    let up_to_date = !is_older(current, &latest);
    if up_to_date && !force {
        println!("\nAlready up to date.");
        if check {
            return Ok(());
        }
        println!("Use {} to reinstall anyway.", "--force".yellow());
        return Ok(());
    }

    if check {
        println!(
            "\n{} is available. Run {} to install it.",
            latest.green(),
            "bat-cli update".yellow()
        );
        return Ok(());
    }

    println!("\nInstalling {} with cargo...", latest.green());
    install(&latest)?;

    println!(
        "\n{} bat-cli {} installed. Run {} to confirm.",
        "✓".green(),
        latest.green(),
        "bat-cli --version".yellow()
    );
    Ok(())
}

fn install(version: &str) -> CommandResult<()> {
    // `--locked` is not optional: it makes cargo honour the Cargo.lock published
    // with the crate instead of re-resolving, which is what keeps the build on
    // the same toolchain that produced the release.
    let status = Command::new("cargo")
        .args([
            "install",
            CRATE_NAME,
            "--version",
            version,
            "--force",
            "--locked",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .into_report()
        .change_context(CommandError)
        .attach_printable("cannot run cargo; is it on PATH?")?;

    if !status.success() {
        return Err(Report::new(CommandError)
            .attach_printable(format!("cargo install failed with {status}"))
            .attach(crate::Suggestion(format!(
                "run `cargo install {CRATE_NAME} --version {version} --force --locked` by hand to see the full output"
            ))));
    }
    Ok(())
}

/// Compare two semver strings numerically.
///
/// A plain string comparison gets `0.9.0` and `0.14.0` backwards, which would
/// silently stop offering updates after the minor rolled past nine.
fn is_older(current: &str, latest: &str) -> bool {
    parse(current) < parse(latest)
}

fn parse(version: &str) -> (u64, u64, u64, bool) {
    // Pre-release suffixes sort before the plain release: 1.0.0-rc1 < 1.0.0.
    let (core, pre) = match version.split_once(['-', '+']) {
        Some((core, _)) => (core, true),
        None => (version, false),
    };
    let mut parts = core
        .split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        !pre,
    )
}

#[cfg(test)]
mod update_test {
    use super::*;

    #[test]
    fn test_version_ordering_is_numeric_not_lexicographic() {
        // The case a string comparison gets wrong.
        assert!(is_older("0.9.0", "0.14.0"));
        assert!(!is_older("0.14.0", "0.9.0"));

        assert!(is_older("0.13.3", "0.14.0"));
        assert!(is_older("0.14.0", "0.14.1"));
        assert!(is_older("0.14.0", "1.0.0"));
        assert!(!is_older("0.14.0", "0.14.0"));
        assert!(!is_older("0.14.1", "0.14.0"));
    }

    #[test]
    fn test_pre_release_sorts_before_the_release() {
        assert!(is_older("1.0.0-rc1", "1.0.0"));
        assert!(!is_older("1.0.0", "1.0.0-rc1"));
    }

    #[test]
    fn test_malformed_versions_do_not_panic() {
        assert!(is_older("", "0.1.0"));
        assert!(!is_older("0.1.0", "not-a-version"));
    }

    #[test]
    fn test_current_version_matches_the_manifest() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
        assert!(!current_version().is_empty());
    }
}
