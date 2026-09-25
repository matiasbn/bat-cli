#[macro_use]
extern crate log;

extern crate confy;

use clap::{Parser, Subcommand};
use colored::Colorize;
use inflector::Inflector;

use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::path::BatFile;
use crate::commands::sonar_commands::SonarCommand;
use crate::commands::{BatCommandEnumerator, CommandResult};

use crate::batbelt::BatEnumerator;

use commands::CommandError;
use error_stack::fmt::{Charset, ColorMode};
use error_stack::{IntoReport, Result};
use error_stack::{Report, ResultExt};

use crate::commands::project_commands::ProjectCommands;

use regex::Regex;

pub mod batbelt;
pub mod commands;
pub mod config;
pub mod guide;

// pub type BatDerive = #[derive(Debug, PartialEq, Copy, strum_macros::Display, strum_macros::EnumIter)];

#[derive(Parser, Debug, Clone)]
#[command(
    author,
    version,
    about = "Blockchain Auditor Toolkit (BAT) CLI",
    after_help = "AI assistant setup: run `bat-cli refresh-ai-guide` — it installs the assistant \
                  skill and prints how to point an AI (Claude, Codex, Gemini) at bat-cli's guide, \
                  so you can just say \"use the bat-cli skill\"."
)]
struct Cli {
    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
    #[command(subcommand)]
    command: BatCommands,
}

#[derive(strum_macros::Display, Subcommand, Debug, PartialEq, Clone, strum_macros::EnumIter)]
enum BatCommands {
    // Init is the default command (run with no subcommand).
    /// Set up a bat project here: detect the framework, create the Miro board
    /// and scan the source code
    Init {
        /// Run without any prompt (for scripts / AI): use the folder name for the
        /// project and, when logged in to Miro, create a board named after it.
        #[arg(long)]
        yes: bool,
        /// Attach this existing Miro board (URL) instead of creating one. Implies
        /// a non-interactive board step.
        #[arg(long = "board-url", value_name = "URL")]
        board_url: Option<String>,
    },
    /// Authorize bat-cli against Miro once, for every project on this machine
    Login {
        /// Register the Miro app credentials before authorizing
        #[arg(long)]
        setup: bool,
        /// Show who the stored token belongs to, and its scopes
        #[arg(long)]
        status: bool,
        /// Re-authorize even if a valid token is already stored
        #[arg(long)]
        force: bool,
    },
    /// Revoke the stored Miro token and forget it
    Logout,
    /// Update bat-cli to the latest published version
    Update {
        /// Only report whether a newer version exists
        #[arg(long)]
        check: bool,
        /// Reinstall even when already up to date
        #[arg(long)]
        force: bool,
    },
    /// Show or edit the machine-wide preferences (~/.config/bat-cli/config.toml)
    Config {
        /// Re-answer the preferences instead of only printing them
        #[arg(long)]
        edit: bool,
    },
    /// Rescan the source code after it changed, rebuilding the metadata that
    /// deploy reads
    Sonar,
    /// Regenerate the machine-global AI guide and reinstall the assistant skills.
    ///
    /// Install the AI-assistant skill + guide and print how to point an AI at bat-cli
    /// (run this once right after installing). Needs no project.
    #[command(name = "refresh-ai-guide")]
    RefreshAiGuide,
    /// Deploy an entry point's screenshots to a Miro board
    Deploy {
        /// Entry point to deploy, as `name` or `Contract.name`. Omit to pick
        /// one from a list.
        #[arg(long)]
        entry_point: Option<String>,
        /// Print the computed layout without sending anything to Miro
        #[arg(long)]
        dry_run: bool,
        /// Include each function's NatSpec documentation in its screenshot, so the
        /// diagram carries the documented intent next to the code
        #[arg(long = "with-documentation")]
        with_documentation: bool,
        /// Write a local PNG preview of the composed frame to this path
        #[arg(long)]
        preview: Option<String>,
        /// Connector thickness in dp (1-24)
        #[arg(long, default_value_t = 8)]
        stroke_width: u32,
        /// Never draw this contract's functions: pass a contract name (`Math`) or any
        /// part of a path (`openzeppelin-contracts/contracts/utils/math`). Repeatable.
        /// The calls to it stay visible in the callers' screenshots — only its own
        /// boxes are left out, which is how you thin a diagram full of a library you
        /// already know. Deploy it as its own entry point when you do need to read it.
        #[arg(long = "ignore-contract", value_name = "NAME_OR_PATH")]
        ignore_contract: Vec<String>,
        /// Answer the "this entry point already has a deployment — deploy again?"
        /// question with yes, so a re-deploy runs unattended (scripts, assistants).
        #[arg(long)]
        yes: bool,
        /// Draw the frame even if some interface calls in the tree are unresolved,
        /// instead of stopping to list them. Downstream nodes behind those calls are
        /// simply omitted.
        #[arg(long)]
        allow_unresolved: bool,
        /// Draw the whole call graph inline in ONE frame: never cut a branch out to
        /// its own frame, never link an already-deployed frame — every function is a
        /// screenshot. Use it to see how large a big function is with screenshots
        /// only (and how Miro handles it).
        #[arg(long)]
        inline_all: bool,
    },
    /// Draw one declaration's source onto a frame that is already on the board.
    ///
    /// A function's screenshot names things it does not explain — a state variable, a
    /// struct used as a parameter. Name it and it lands on the frame, in free space, for
    /// you to drag where you want it. Run with no `--frame` to list the deployed frames.
    Screenshot {
        /// Symbol to draw: `Name` or `Contract.Name`. Omit when giving --file/--lines.
        name: Option<String>,
        /// The deployment to draw into: the entry point it was deployed for
        /// (`Contract.function`). Omit to list what is deployed.
        #[arg(long)]
        deployment: Option<String>,
        /// Which frame of that deployment, by the function it shows. A deployment draws
        /// one frame per function, so the name is unique inside it. Omit for the
        /// deployment's own root frame.
        #[arg(long)]
        dependency: Option<String>,
        /// Source file, when the symbol is not in the index. Needs --lines.
        #[arg(long)]
        file: Option<String>,
        /// Line range as `start-end`, 1-based and inclusive. Needs --file.
        #[arg(long)]
        lines: Option<String>,
        /// Include the declaration's NatSpec, as `deploy --with-documentation` does
        #[arg(long = "with-documentation")]
        with_documentation: bool,
        /// Extend the frame downwards when the drawing does not fit. By default the
        /// frame is left exactly as the auditor arranged it
        #[arg(long)]
        grow: bool,
    },
    /// Point a deployed frame's record at the frame that is actually on the board.
    ///
    /// Cutting and pasting a frame in Miro gives every widget a new id, orphaning the
    /// registry; the titles survive, so the record can be rebuilt without redeploying
    /// (which would place the frame by auto-layout, losing your arrangement).
    Relink {
        /// Entry point naming the frame. Omit it (or pass --check) to review the registry.
        entry_point: Option<String>,
        /// The frame's Miro link, when several frames share the title.
        #[arg(long = "frame-url", value_name = "URL")]
        frame_url: Option<String>,
        /// Report which records still match the board instead of changing anything.
        #[arg(long)]
        check: bool,
    },
    /// Never draw a contract's functions again (a library you have already read)
    Ignore {
        /// Contract name (`Math`) or any part of a path
        /// (`openzeppelin-contracts/contracts/utils/math`). Omit to list.
        pattern: Option<String>,
        /// List what is ignored instead of adding one.
        #[arg(long)]
        list: bool,
        /// Stop ignoring <pattern>.
        #[arg(long)]
        remove: bool,
    },
    /// Record an interface→contract resolution so `deploy` can follow a runtime-bound
    /// interface call to its concrete implementation. `deploy` stops and lists what to
    /// resolve; add them here, then deploy again. Stored in the metadata.
    Resolve {
        /// Interface type to resolve, e.g. `IBorrowerOperations`.
        interface: Option<String>,
        /// The concrete in-scope contract it points to, e.g. `BorrowerOperations`.
        contract: Option<String>,
        /// List the current resolutions instead of adding one.
        #[arg(long)]
        list: bool,
        /// Remove the resolution for <interface>.
        #[arg(long)]
        remove: bool,
    },
}

impl BatEnumerator for BatCommands {}

impl Default for BatCommands {
    fn default() -> Self {
        BatCommands::Init {
            yes: false,
            board_url: None,
        }
    }
}

impl BatCommands {
    pub async fn execute(&self) -> Result<(), CommandError> {
        self.validate_command()?;
        match self {
            BatCommands::Init { yes, board_url } => {
                ProjectCommands::Init
                    .init_bat_project(*yes, board_url.clone())
                    .await
            }
            BatCommands::Login {
                setup,
                status,
                force,
            } => {
                use crate::batbelt::miro::auth;
                if *status {
                    auth::status().await.change_context(CommandError)
                } else {
                    auth::login(*setup, *force).await.change_context(CommandError)
                }
            }
            BatCommands::Logout => crate::batbelt::miro::auth::logout()
                .await
                .change_context(CommandError),
            BatCommands::Update { check, force } => {
                crate::commands::update_commands::run(*check, *force).await
            }
            BatCommands::Config { edit } => {
                ProjectCommands::show_global_config(*edit).change_context(CommandError)
            }
            BatCommands::Sonar => SonarCommand::Run.execute_command(),
            // The refresh already ran in `main::run`; nothing left to do.
            BatCommands::RefreshAiGuide => {
                // refresh_ai_surface() already ran in main(); just show where the
                // AI integration lives and how to point an assistant at it.
                crate::guide::print_ai_setup_hint();
                Ok(())
            }
            BatCommands::Deploy {
                entry_point,
                dry_run,
                with_documentation,
                preview,
                stroke_width,
                allow_unresolved,
                yes,
                ignore_contract,
                inline_all,
            } => {
                crate::batbelt::evm::miro::auto_deploy::run(
                crate::batbelt::evm::miro::auto_deploy::AutoDeployOptions {
                    entry_point: entry_point.clone(),
                    dry_run: *dry_run,
                    with_documentation: *with_documentation,
                    preview: preview.clone(),
                    stroke_width: *stroke_width,
                    allow_unresolved: *allow_unresolved,
                    assume_yes: *yes,
                    ignore_contracts: ignore_contract.clone(),
                    inline_all: *inline_all,
                    },
                )
                .await
                .change_context(CommandError)
            }
            BatCommands::Screenshot {
                name,
                deployment,
                dependency,
                file,
                lines,
                with_documentation,
                grow,
            } => crate::batbelt::evm::miro::screenshot::run(
                crate::batbelt::evm::miro::screenshot::ScreenshotOptions {
                    name: name.clone(),
                    deployment: deployment.clone(),
                    dependency: dependency.clone(),
                    file: file.clone(),
                    lines: lines.clone(),
                    with_documentation: *with_documentation,
                    grow: *grow,
                },
            )
            .await
            .change_context(CommandError),
            BatCommands::Relink {
                entry_point,
                frame_url,
                check,
            } => crate::batbelt::evm::miro::auto_deploy::run_relink(
                entry_point.clone(),
                frame_url.clone(),
                *check,
            )
            .await
            .change_context(CommandError),
            BatCommands::Ignore {
                pattern,
                list,
                remove,
            } => run_ignore(pattern.clone(), *list, *remove),
            BatCommands::Resolve {
                interface,
                contract,
                list,
                remove,
            } => run_resolve(interface.clone(), contract.clone(), *list, *remove),
        }
    }

    /// Every command that reads project data needs the metadata cache to exist.
    ///
    /// There is no branch check any more: bat-cli does not create commits or
    /// manage git, so it has no business dictating which branch you are on.
    fn validate_command(&self) -> CommandResult<()> {
        let check_metadata = match self {
            BatCommands::Init { .. }
            | BatCommands::Login { .. }
            | BatCommands::Logout
            | BatCommands::Config { .. }
            | BatCommands::RefreshAiGuide
            | BatCommands::Update { .. } => return Ok(()),
            BatCommands::Sonar => false,
            BatCommands::Deploy { .. }
            | BatCommands::Screenshot { .. }
            | BatCommands::Relink { .. }
            | BatCommands::Resolve { .. }
            | BatCommands::Ignore { .. } => true,
        };

        if check_metadata {
            let bat_config = crate::config::BatConfig::get_config().change_context(CommandError)?;
            if bat_config.project_type == crate::config::ProjectType::Foundry {
                crate::batbelt::evm::metadata::bat_metadata::EvmBatMetadata::read_metadata()
                    .change_context(CommandError)?;
            } else {
                BatMetadata::read_metadata()
                    .change_context(CommandError)?
                    .check_metadata_is_initialized()
                    .change_context(CommandError)?;
            }
        }
        Ok(())
    }

    pub fn get_pretty_command(&self) -> CommandResult<String> {
        let multi_line_command_regex = Regex::new(r#"[\w]+(\([\w\s,]+\))+"#)
            .into_report()
            .change_context(CommandError)?;
        let command_string = format!("{self:#?}");
        if multi_line_command_regex.is_match(&command_string) {
            let mut command_string_lines = command_string.lines();
            let command_name = command_string_lines.next().unwrap().to_kebab_case();
            let command_option = command_string_lines.next().unwrap().trim().to_kebab_case();
            return Ok(format!("{} {}", command_name, command_option));
        }
        Ok(self.to_string().to_kebab_case())
    }
}


pub struct Suggestion(String);

impl Suggestion {
    pub fn set_report() {
        Report::set_charset(Charset::Utf8);
        Report::set_color_mode(ColorMode::Color);
        Report::install_debug_hook::<Self>(|Self(value), context| {
            context.push_body(format!("{}: {value}", "suggestion".yellow()))
        });
    }
}


async fn run() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    Suggestion::set_report();

    // Logs go to stderr, controlled by -v/-q or RUST_LOG, rather than to a file
    // inside the project that nobody read.
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .parse_default_env()
        .format_timestamp(None)
        .init();

    // The AI guide describes the binary, so every command is a chance to make sure the one
    // on disk is the one just installed — including `config` and `update`, which have no
    // project at all. Best-effort by construction: it never fails the command.
    crate::guide::refresh_ai_surface();

    cli.command.execute().await
}

/// `bat-cli resolve` — read/update the metadata's interface→contract resolutions.
/// `bat-cli ignore` — the list of contracts a deploy never draws.
///
/// It is a reading decision, so it lives with the project (in `BatMetadata.json`,
/// preserved across a re-scan) rather than being retyped on every deploy. The flag
/// `deploy --ignore-contract` adds to this list for one run; what a deploy leaves
/// out is the union of the two.
fn run_ignore(pattern: Option<String>, list: bool, remove: bool) -> CommandResult<()> {
    use crate::batbelt::evm::metadata::bat_metadata::EvmBatMetadata;
    let mut md = EvmBatMetadata::read_metadata().change_context(CommandError)?;

    if list || pattern.is_none() {
        if md.ignored_contracts.is_empty() {
            println!("nothing ignored yet.");
        } else {
            println!("never drawn:");
            for pattern in &md.ignored_contracts {
                println!("  {pattern}");
            }
        }
        return Ok(());
    }
    let pattern = pattern.expect("checked above");

    if remove {
        md.ignored_contracts.retain(|existing| existing != &pattern);
        md.save_metadata().change_context(CommandError)?;
        println!("no longer ignoring {pattern}");
        return Ok(());
    }
    if md.ignored_contracts.iter().any(|existing| existing == &pattern) {
        println!("{pattern} is already ignored");
        return Ok(());
    }
    md.ignored_contracts.push(pattern.clone());
    md.save_metadata().change_context(CommandError)?;
    println!("✓ {pattern} will not be drawn (deploy it as an entry point to read it)");
    Ok(())
}

fn run_resolve(
    interface: Option<String>,
    contract: Option<String>,
    list: bool,
    remove: bool,
) -> CommandResult<()> {
    use crate::batbelt::evm::metadata::bat_metadata::EvmBatMetadata;
    let mut md = EvmBatMetadata::read_metadata().change_context(CommandError)?;

    // No interface given (or --list) → show what is resolved and what still needs it.
    if list || interface.is_none() {
        if md.resolutions.is_empty() {
            println!("no resolutions yet.");
        } else {
            println!("resolutions:");
            let mut items: Vec<_> = md.resolutions.iter().collect();
            items.sort();
            for (i, c) in items {
                println!("  {i} → {c}");
            }
        }
        return Ok(());
    }
    let interface = interface.expect("checked above");

    if remove {
        md.resolutions.remove(&interface);
        md.save_metadata().change_context(CommandError)?;
        println!("removed resolution for {interface}");
        return Ok(());
    }

    let Some(contract) = contract else {
        return Err(error_stack::Report::new(CommandError).attach_printable(
            "usage: bat-cli resolve <INTERFACE> <CONTRACT>  (or --list / --remove <INTERFACE>)",
        ));
    };
    md.resolutions.insert(interface.clone(), contract.clone());
    md.save_metadata().change_context(CommandError)?;
    println!("✓ resolved {interface} → {contract}");
    Ok(())
}

#[tokio::main]
async fn main() -> CommandResult<()> {
    let cli: Cli = Cli::parse();

    match run().await {
        Ok(_) => {
            println!(
                "{} {} script successfully executed!",
                "bat-cli".green(),
                cli.command.get_pretty_command()?.green()
            );
            Ok(())
        }
        Err(error) => {
            eprintln!(
                "{} {} script finished with error",
                "bat-cli".red(),
                cli.command.get_pretty_command()?.red()
            );
            Err(error)
        }
    }
}
