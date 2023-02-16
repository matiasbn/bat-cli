#![feature(core_panic)]
#![feature(exit_status_error)]
extern crate core;

#[macro_use]
extern crate log;

use batbelt::git::{check_correct_branch, GitCommit};
use clap::{Parser, Subcommand};

mod batbelt;
mod commands;
mod config;
mod package;

use std::{error, result};

type Result<T> = result::Result<T, Box<dyn error::Error>>;

#[derive(Parser, Debug)]
#[command(author, version, about = "Blockchain Auditor Toolkit (BAT) CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Creates a Bat project
    Create,
    /// Initializes the project from the Bat.toml config file
    Init {
        /// Skips the initial commit process
        #[arg(short, long)]
        skip_initial_commit: bool,
    },
    /// code-overhaul files management
    #[command(subcommand)]
    CO(CodeOverhaulActions),
    /// findings files management
    #[command(subcommand)]
    Finding(FindingActions),
    /// Miro integration
    #[command(subcommand)]
    Miro(MiroActions),
    /// Update the templates folder and the package.json of the audit repository
    Update,
    /// Commits the open_questions, smellies and threat_modeling notes
    Notes,
    /// Initializes the metadata and deploy the Initial Miro frames
    Sonar,
    // /// Updates the audit_result.md file in the root of the audit
    // #[command(subcommand)]
    // Result(ResultActions),
    /// Updates the metadata.md file
    #[command(subcommand)]
    Metadata(MetadataActions),
    /// Cargo publish operations, available only for dev
    #[command(subcommand)]
    Package(PackageActions),
}

// #[derive(Subcommand, Debug)]
// enum ResultActions {
//     /// Updates the Code Overhaul section of the audit_result.md file
//     CodeOverhaul,
//     /// Updates the Findings section of the audit_result.md file
//     Findings {
//         /// updates the result, formatting with html structure
//         #[arg(long)]
//         html: bool,
//     },
//     /// Creates the commit for the results files
//     Commit,
// }
#[derive(Subcommand, Debug)]
enum MetadataActions {
    /// Updates the Structs section of the metadata.md file
    Structs,
    /// Updates the Functions section of the metadata.md file
    Functions,
}

#[derive(Subcommand, Debug)]
enum MiroActions {
    /// Deploy or updates a code-overhaul frame
    Deploy,
    /// Creates or updates the Accounts frame
    Accounts,
    /// Deploys the entrypoint, context accounts and handler to a Miro frame
    Entrypoint {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
        /// shows the list of entrypoints sorted by name
        #[arg(long)]
        sorted: bool,
    },
    /// Creates an screenshot in a determined frame
    Metadata {
        /// deploy the screenshots with the default configuration
        #[arg(long)]
        default: bool,
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
}

#[derive(Subcommand, Debug)]
enum FindingActions {
    /// Creates a finding file
    Create,
    /// Finish a finding file by creating a commit
    Finish,
    /// Update a finding file by creating a commit
    Update,
    /// Moves all the to-review findings to accepted
    AcceptAll,
    /// Moves a finding from to-review to rejected
    Reject,
}

#[derive(Subcommand, Debug)]
enum CodeOverhaulActions {
    /// Starts a code-overhaul file audit
    Start,
    /// Moves the code-overhaul file from to-review to finished
    Finish,
    /// Update a code-overhaul file by creating a commit
    Update,
    /// Copies the images to the co Miro frame
    Miro,
    /// Counts the to-review, started, finished and total co files
    Count,
    /// Opens the co file and the instruction of a started entrypoint
    Open,
    /// Updates the templates in to-review folder
    Templates,
}

#[derive(Subcommand, Debug)]
enum PackageActions {
    /// run cargo clippy and commit the changes
    Format,
    /// Creates a git flow release, bumps the version, formats the code and publish
    Release,
}
#[tokio::main]
async fn main() {
    env_logger::init();
    let cli: Cli = Cli::parse();
    match cli.command {
        Commands::Init { .. }
        | Commands::Create
        | Commands::Package(PackageActions::Format)
        | Commands::Package(PackageActions::Release) => {}
        _ => {
            check_correct_branch().unwrap();
        }
    }
    match cli.command {
        Commands::Create => commands::create::create_project().unwrap(),
        Commands::Sonar => commands::sonar::start_sonar(),
        Commands::Init {
            skip_initial_commit,
        } => commands::init::initialize_bat_project(skip_initial_commit)
            .await
            .unwrap(),
        Commands::CO(CodeOverhaulActions::Start) => {
            commands::code_overhaul::start::start_co_file().unwrap()
        }
        Commands::CO(CodeOverhaulActions::Finish) => {
            commands::code_overhaul::finish::finish_co_file()
                .await
                .unwrap()
        }
        Commands::CO(CodeOverhaulActions::Update) => {
            commands::code_overhaul::update::update_co_file().unwrap()
        }
        Commands::CO(CodeOverhaulActions::Count) => {
            commands::code_overhaul::count_co_files().unwrap()
        }
        Commands::CO(CodeOverhaulActions::Open) => commands::code_overhaul::open_co().unwrap(),
        Commands::CO(CodeOverhaulActions::Templates) => {
            commands::code_overhaul::update_co_templates().unwrap()
        }
        Commands::Miro(MiroActions::Deploy) => commands::miro::deploy_co().await.unwrap(),
        Commands::Miro(MiroActions::Accounts) => commands::miro::deploy_accounts().await.unwrap(),
        Commands::Miro(MiroActions::Entrypoint { select_all, sorted }) => {
            commands::miro::deploy_entrypoint_screenshots_to_frame(select_all, sorted)
                .await
                .unwrap()
        }
        Commands::Miro(MiroActions::Metadata {
            default,
            select_all,
        }) => commands::miro::deploy_metadata_screenshot_to_frame(default, select_all)
            .await
            .unwrap(),
        Commands::Metadata(MetadataActions::Structs) => commands::metadata::structs().unwrap(),
        Commands::Metadata(MetadataActions::Functions) => commands::metadata::functions().unwrap(),
        Commands::Finding(FindingActions::Create) => commands::finding::create_finding().unwrap(),
        Commands::Finding(FindingActions::Finish) => commands::finding::finish_finding().unwrap(),
        Commands::Finding(FindingActions::Update) => commands::finding::update_finding().unwrap(),
        Commands::Finding(FindingActions::AcceptAll) => commands::finding::accept_all().unwrap(),
        Commands::Finding(FindingActions::Reject) => commands::finding::reject().unwrap(),
        Commands::Update => commands::update::update_repository().unwrap(),
        Commands::Notes => batbelt::git::create_git_commit(GitCommit::Notes, None).unwrap(),
        // Commands::Result(ResultActions::Findings { html }) => {
        //     commands::result::findings_result(html).unwrap()
        // }
        // Commands::Result(ResultActions::Commit) => commands::result::results_commit().unwrap(),
        // only for dev
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Format) => package::format().unwrap(),
        #[cfg(debug_assertions)]
        Commands::Package(PackageActions::Release) => package::release().unwrap(),
        _ => unimplemented!("Command only implemented for dev operations"),
    }
}

// mod test {
//     use std::{thread, time::Duration};

//     use colored::Colorize;
//     use indicatif::{ProgressBar, ProgressStyle};

//     #[test]
//     fn test_bat_cli_loader() {
//         let pb = ProgressBar::new_spinner();
//         pb.enable_steady_tick(Duration::from_millis(200));
//         pb.set_style(
//             ProgressStyle::with_template("{spinner:.blue} {msg}")
//                 .unwrap()
//                 .tick_strings(&[
//                     "📂                  〰️🦇",
//                     "📂                  〰️🦇",
//                     "📂                〰️  🦇",
//                     "📂              〰️    🦇",
//                     "📂            〰️      🦇",
//                     "📂          〰️        🦇",
//                     "📂        〰️          🦇",
//                     "📂      〰️            🦇",
//                     "📂    〰️              🦇",
//                     "📂  〰️                🦇",
//                     "📂〰️                  🦇",
//                     "📂  〰️                🦇",
//                     "📂    〰️              🦇",
//                     "📂      〰️            🦇",
//                     "📂        〰️          🦇",
//                     "📂          〰️        🦇",
//                     "📂            〰️      🦇",
//                     "📂              〰️    🦇",
//                     "📂                〰️  🦇",
//                     "📂                  〰️🦇",
//                 ]),
//         );
//         pb.set_message(format!("Looking for Structs with {}...", "BatSonar".red()));
//         thread::sleep(Duration::from_millis(3400));
//         pb.finish_with_message("Done");
//     }
// }
