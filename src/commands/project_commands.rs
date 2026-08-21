//! `init` and `reload`: set up a bat project, and refresh its metadata.
//!
//! bat-cli produces Miro diagrams, so a project is just `Bat.toml`, a
//! `BatMetadata.json` cache and a `figures/` directory, all at the root of the
//! audited repository. It creates no branches and no commits: what you do with
//! version control is yours to decide.

use clap::Subcommand;
use colored::Colorize;
use error_stack::{Report, Result, ResultExt};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::miro::auth;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::BatEnumerator;
use crate::commands::sonar_commands::SonarCommand;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};
use crate::config::{BatConfig, BatGlobalConfig};

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum ProjectCommands {
    #[default]
    Init,
    Reload,
}

impl BatEnumerator for ProjectCommands {}

impl BatCommandEnumerator for ProjectCommands {
    fn execute_command(&self) -> CommandResult<()> {
        match self {
            ProjectCommands::Init => unimplemented!("init is async, call init_bat_project"),
            ProjectCommands::Reload => Self::reload_bat_project(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }
}

impl ProjectCommands {
    /// Create `Bat.toml`, the workspace directories and the metadata cache, then
    /// scan the code.
    pub async fn init_bat_project(&self) -> Result<(), CommandError> {
        if BatFile::BatToml.file_exists().change_context(CommandError)? {
            println!(
                "{} already exists here; running {} instead",
                "Bat.toml".yellow(),
                "reload".yellow()
            );
            return Self::reload_bat_project();
        }

        BatConfig::new_with_prompt().change_context(CommandError)?;

        TemplateGenerator
            .create_workspace_folders()
            .change_context(CommandError)?;
        TemplateGenerator
            .create_metadata_json()
            .change_context(CommandError)?;
        Self::suggest_git_ignore()?;

        Self::configure_miro_board().await?;

        println!("\nScanning the source code...");
        SonarCommand::Run.execute_command()?;

        let board_url = BatConfig::get_config()
            .change_context(CommandError)?
            .miro_board_url;
        println!("\n{} project ready.", "✓".green());
        if !board_url.trim().is_empty() {
            println!("  board: {}", board_url.blue());
        }
        println!(
            "  run {} to deploy an entry point",
            "bat-cli deploy".yellow()
        );
        Ok(())
    }

    /// Rebuild the metadata cache from the current source.
    fn reload_bat_project() -> CommandResult<()> {
        BatConfig::get_config().change_context(CommandError)?;
        TemplateGenerator
            .create_workspace_folders()
            .change_context(CommandError)?;
        SonarCommand::Run.execute_command()?;
        println!("{} metadata reloaded", "✓".green());
        Ok(())
    }

    /// Ask for the Miro board once and record it in `Bat.toml`.
    ///
    /// The token itself is machine-wide and comes from `bat-cli login`; only the
    /// board belongs to the project.
    async fn configure_miro_board() -> CommandResult<()> {
        let token = auth::stored_access_token();
        if token.trim().is_empty() {
            println!(
                "\n{} not logged in to Miro. Run {} before deploying.",
                "note:".yellow(),
                "bat-cli login".yellow()
            );
            return Ok(());
        }

        let board_url = Self::prompt_miro_board_url(&token).await?;
        let mut bat_config = BatConfig::get_config().change_context(CommandError)?;
        bat_config.miro_board_url = board_url;
        bat_config.save().change_context(CommandError)?;
        Ok(())
    }

    /// Get a board without sending the user to the browser: offer to create one
    /// named after the project, otherwise pick from the ones the token can see.
    async fn prompt_miro_board_url(token: &str) -> Result<String, CommandError> {
        // Default to the directory being audited: that is the name the user
        // will look for in Miro.
        let suggested_name = std::env::current_dir()
            .ok()
            .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| {
                BatConfig::get_config()
                    .map(|config| config.project_name)
                    .unwrap_or_else(|_| "bat-cli".to_string())
            });
        let boards = MiroConfig::list_boards(token).await.unwrap_or_default();

        let mut labels = vec!["Create a new board".to_string()];
        labels.extend(boards.iter().map(|(name, _)| name.clone()));
        labels.push("Enter a board URL manually".to_string());

        let selection =
            BatDialoguer::select("Miro board:".to_string(), labels.clone(), None)
                .change_context(CommandError)?;

        if selection == 0 {
            let board_name =
                BatDialoguer::input_with_default("Board name".to_string(), suggested_name)
                    .change_context(CommandError)?;
            match MiroConfig::create_board(token, board_name.trim()).await {
                Ok((name, url)) => {
                    println!("{} created board \"{}\"\n  {}", "✓".green(), name, url.blue());
                    return Ok(url);
                }
                Err(report) => {
                    // The free plan allows only three team boards, so failing
                    // here is expected often enough to keep going rather than
                    // abort the whole init.
                    println!("{} could not create the board: {:?}", "warning:".yellow(), report);
                }
            }
        } else if selection <= boards.len() {
            return Ok(boards[selection - 1].1.clone());
        }

        loop {
            let input = BatDialoguer::input("Miro board URL".to_string())
                .change_context(CommandError)?;
            match MiroConfig::validate_board(token, input.trim()).await {
                Ok(board_name) => {
                    println!("{} board \"{}\"", "✓".green(), board_name);
                    return Ok(input.trim().to_string());
                }
                Err(report) => {
                    println!("{} {:?}", "could not use that board:".red(), report);
                }
            }
        }
    }

    /// Screenshots are regenerated on demand, so keep them out of git — but only
    /// suggest it, since bat-cli does not manage the repository.
    fn suggest_git_ignore() -> CommandResult<()> {
        let git_ignore = BatFile::GitIgnore;
        let ignore_content = TemplateGenerator.get_git_ignore_content();
        let already_ignored = git_ignore
            .read_content(false)
            .map(|content| content.contains("figures/"))
            .unwrap_or(false);
        if !already_ignored {
            println!(
                "\nConsider adding this to {}:\n{}",
                ".gitignore".yellow(),
                ignore_content.trim()
            );
        }
        Ok(())
    }

    /// Print — and optionally re-answer — the preferences shared by every
    /// project on this machine.
    pub fn show_global_config(edit: bool) -> CommandResult<()> {
        use crate::batbelt::command_line::CodeEditor;
        use crate::config::global_config_dir;

        let mut global = BatGlobalConfig::load().change_context(CommandError)?;

        if edit {
            let editors = CodeEditor::get_colorized_type_vec(false);
            let selection = BatDialoguer::select(
                format!(
                    "Select a code editor, choose {} to disable:",
                    CodeEditor::None.get_colored_name(false)
                ),
                editors,
                None,
            )
            .change_context(CommandError)?;
            global.code_editor = CodeEditor::from_index(selection);
            global.use_code_editor = global.code_editor != CodeEditor::None;
            global.save().change_context(CommandError)?;
        }

        println!("{}", "bat-cli preferences".bold());
        println!("  directory:    {}", global_config_dir().display());
        println!("  config:       {}", BatGlobalConfig::path().display());
        println!("  credentials:  {}", auth::MiroCredentials::path());
        println!();
        println!(
            "  code_editor:      {}",
            global.code_editor.get_colored_name(false)
        );
        println!("  use_code_editor:  {}", global.use_code_editor);
        println!(
            "  miro:             {}",
            if auth::stored_access_token().is_empty() {
                "not logged in — run `bat-cli login`".yellow().to_string()
            } else {
                "logged in".green().to_string()
            }
        );
        Ok(())
    }
}
