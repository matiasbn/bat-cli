use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use std::path::Path;
use std::process::Command;
use std::{error::Error, fmt, fs, str};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use crate::batbelt::path::BatFile;
use normalize_url::normalizer;
use crate::batbelt::{bat_dialoguer, BatEnumerator};

use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use walkdir::WalkDir;

/// Overrides the machine-wide config directory, mostly for tests and CI.
pub const CONFIG_DIR_ENV: &str = "BAT_CLI_CONFIG_DIR";

/// Serializes the tests that point `CONFIG_DIR_ENV` at a temporary directory.
/// They live in different modules but share one process, and `cargo test` runs
/// them on parallel threads, so without this they clobber each other.
#[cfg(test)]
pub static CONFIG_ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Directory holding everything that belongs to the user rather than to a
/// project: `config.toml` (preferences) and `miro.toml` (credentials).
///
/// XDG layout on every platform (`~/.config/bat-cli`), which is where CLI users
/// expect to find it and where a dotfiles repo can pick it up.
pub fn global_config_dir() -> std::path::PathBuf {
    if let Ok(directory) = std::env::var(CONFIG_DIR_ENV) {
        if !directory.trim().is_empty() {
            return std::path::PathBuf::from(directory);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return std::path::PathBuf::from(xdg).join("bat-cli");
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".config").join("bat-cli")
}

/// User-level preferences, shared by every audit on this machine.
///
/// `BatAuditor.toml` holds the same fields so a project can override them, but
/// nothing here has to be answered again for each new audit. Project-scoped
/// settings (`external_bat_metadata`, and everything in `Bat.toml`) stay local.
#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct BatGlobalConfig {
    /// Default name to preselect when joining a project.
    #[serde(default)]
    pub auditor_name: String,
    #[serde(default)]
    pub use_code_editor: bool,
    #[serde(default)]
    pub code_editor: CodeEditor,
}

impl BatGlobalConfig {
    pub fn path() -> std::path::PathBuf {
        global_config_dir().join("config.toml")
    }

    pub fn load() -> BatConfigResult<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .into_report()
            .change_context(BatConfigError)
            .attach_printable_lazy(|| format!("cannot read {}", path.display()))?;
        toml::from_str(&content)
            .into_report()
            .change_context(BatConfigError)
            .attach_printable_lazy(|| format!("cannot parse {}", path.display()))
    }

    pub fn save(&self) -> BatConfigResult<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .into_report()
                .change_context(BatConfigError)?;
        }
        let content = toml::to_string_pretty(self)
            .into_report()
            .change_context(BatConfigError)?;
        fs::write(&path, content)
            .into_report()
            .change_context(BatConfigError)
            .attach_printable_lazy(|| format!("cannot write {}", path.display()))
    }
}

#[derive(Debug)]
pub struct BatConfigError;

impl fmt::Display for BatConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatConfig error")
    }
}

impl Error for BatConfigError {}

pub type BatConfigResult<T> = Result<T, BatConfigError>;


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum ProjectType {
    Anchor,
    Pinocchio,
    VanillaSolana,
    Foundry,
    #[default]
    GenericRust,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BatConfig {
    pub initialized: bool,
    /// Name of the directory being audited; also the suggested Miro board name.
    pub project_name: String,
    pub miro_board_url: String,
    /// Primary program lib path (first selected, used by Anchor entrypoint detection).
    pub program_lib_path: String,
    /// All selected program lib paths (for multi-program scanning).
    #[serde(default)]
    pub program_lib_paths: Vec<String>,
    #[serde(default)]
    pub program_name: String,
    #[serde(default)]
    pub project_type: ProjectType,
}

impl BatConfig {
    /// The directory being audited. bat-cli lives at the project root, so this
    /// is the project's name.
    fn current_directory_name() -> String {
        std::env::current_dir()
            .ok()
            .and_then(|path| path.file_name().map(|name| name.to_string_lossy().to_string()))
            .unwrap_or_else(|| "bat-project".to_string())
    }

    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let new = Self::create_bat_config_file()?;
        Ok(new)
    }

    fn create_bat_config_file() -> Result<BatConfig, BatConfigError> {
        // Auto-detect project type (initial guess; refined after Cargo.toml discovery)
        let mut project_type = if Path::new("Anchor.toml").is_file() {
            println!("Detected {} project (Anchor.toml found)", "Anchor".green());
            ProjectType::Anchor
        } else if Path::new("foundry.toml").is_file() {
            println!(
                "Detected {} project (foundry.toml found)",
                "Foundry".green()
            );
            ProjectType::Foundry
        } else {
            ProjectType::GenericRust
        };

        // Validate bat-audit doesn't already exist
        if Path::new("bat-audit").is_dir() {
            return Err(
                Report::new(BatConfigError).attach_printable("bat-audit/ folder already exists")
            );
        }

        // Foundry projects: scan .sol files instead of Cargo.toml
        if project_type == ProjectType::Foundry {
            return Self::create_foundry_config();
        }

        // Step 1: List root-level directories that contain at least one Cargo.toml
        let root_dirs: Vec<String> = std::fs::read_dir(".")
            .into_report()
            .change_context(BatConfigError)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let name = path.file_name()?.to_str()?.to_string();
                // Skip hidden dirs, target/ and bat-audit/
                if name.starts_with('.') || name == "target" || name == "bat-audit" {
                    return None;
                }
                let dir_str = format!("./{}", name);
                // Only include if there's at least one Cargo.toml inside
                let has_cargo = WalkDir::new(&dir_str)
                    .into_iter()
                    .filter_map(|f| f.ok())
                    .any(|e| {
                        e.file_name().to_str() == Some("Cargo.toml")
                            && !e.path().to_str().unwrap_or("").contains("target")
                    });
                if has_cargo {
                    Some(dir_str)
                } else {
                    None
                }
            })
            .collect();
        let mut root_dirs = root_dirs;
        root_dirs.sort();

        if root_dirs.is_empty() {
            return Err(Report::new(BatConfigError)
                .attach_printable("No directories found in the current folder"));
        }

        let dir_defaults = vec![true; root_dirs.len()];
        let dir_selections = bat_dialoguer::multiselect(
            "Select the folders to scan for programs",
            root_dirs.clone(),
            Some(&dir_defaults),
        )
        .change_context(BatConfigError)?;

        if dir_selections.is_empty() {
            return Err(Report::new(BatConfigError).attach_printable("No folders selected"));
        }

        // Step 2: Find all Cargo.toml inside selected folders
        let mut cargo_programs_paths: Vec<String> = vec![];
        for &sel_idx in &dir_selections {
            let dir_path = &root_dirs[sel_idx];
            for entry in WalkDir::new(dir_path).into_iter().filter_map(|f| f.ok()) {
                let entry_path = entry.path().to_str().unwrap_or("").to_string();
                if entry.file_name().to_str() == Some("Cargo.toml")
                    && !entry_path.contains("target")
                    && entry_path != "./Cargo.toml"
                {
                    cargo_programs_paths
                        .push(entry_path.trim_end_matches("/Cargo.toml").to_string());
                }
            }
        }

        if cargo_programs_paths.is_empty() {
            return Err(Report::new(BatConfigError)
                .attach_printable("No programs with Cargo.toml found in selected folders"));
        }

        // Refine project type: if not Anchor, check if any Cargo.toml has pinocchio dependency
        if project_type == ProjectType::GenericRust {
            let has_pinocchio = cargo_programs_paths.iter().any(|prog_path| {
                let cargo_toml_path = format!("{}/Cargo.toml", prog_path);
                fs::read_to_string(&cargo_toml_path)
                    .map(|content| content.contains("pinocchio"))
                    .unwrap_or(false)
            });
            if has_pinocchio {
                println!(
                    "Detected {} project (pinocchio dependency found)",
                    "Pinocchio".green()
                );
                project_type = ProjectType::Pinocchio;
            } else {
                println!(
                    "{} No {} or {} dependency detected.",
                    "Warning:".yellow(),
                    "Anchor.toml".green(),
                    "pinocchio".green(),
                );
                println!(
                    "bat-cli will run in {} mode (no entry points or context accounts).",
                    "generic Rust".yellow()
                );
                let continue_anyway = bat_dialoguer::select_yes_or_no("Do you want to continue?")
                    .change_context(BatConfigError)?;
                if !continue_anyway {
                    return Err(Report::new(BatConfigError).attach_printable("Aborted by user"));
                }
            }
        }

        // Step 3: Let the user select which programs to analyze
        let prog_defaults = vec![true; cargo_programs_paths.len()];
        let prog_selections = bat_dialoguer::multiselect(
            "Select the programs to analyze",
            cargo_programs_paths.clone(),
            Some(&prog_defaults),
        )
        .change_context(BatConfigError)?;

        if prog_selections.is_empty() {
            return Err(Report::new(BatConfigError).attach_printable("No programs selected"));
        }

        // Step 4: Resolve lib.rs or main.rs for each selected program
        let mut normalized_program_lib_paths: Vec<String> = vec![];
        for &sel_idx in &prog_selections {
            let program_path = &cargo_programs_paths[sel_idx];
            let lib_path = format!("{}/src/lib.rs", program_path);
            let main_path = format!("{}/src/main.rs", program_path);
            let resolved = if Path::new(&lib_path).is_file() {
                lib_path
            } else if Path::new(&main_path).is_file() {
                main_path
            } else {
                log::warn!(
                    "Neither lib.rs nor main.rs found in {}, skipping",
                    program_path
                );
                continue;
            };
            let normalized = format!("../{}", resolved.trim_start_matches("./"));
            normalized_program_lib_paths.push(normalized);
        }

        if normalized_program_lib_paths.is_empty() {
            return Err(Report::new(BatConfigError)
                .attach_printable("No valid programs found (no lib.rs or main.rs)"));
        }

        // First selected program is the primary (used for Anchor entrypoint detection)
        let normalized_program_lib_path = normalized_program_lib_paths[0].clone();
        let selected_program_path = &cargo_programs_paths[prog_selections[0]];
        let program_name = selected_program_path
            .split('/')
            .next_back()
            .unwrap()
            .to_string()
            .replace('_', "-");
        log::debug!("program_lib_paths: {:#?}", normalized_program_lib_paths);

        let project_name = Self::current_directory_name();

        // Filled in by `init` once a board is created or chosen.
        let miro_board_url = String::new();

        let bat_config = BatConfig {
            initialized: true,
            program_name,
            project_name,
            miro_board_url,
            program_lib_path: normalized_program_lib_path,
            program_lib_paths: normalized_program_lib_paths,
            project_type,
        };
        bat_config.save().change_context(BatConfigError)?;
        Ok(bat_config)
    }


    pub fn normalize_miro_board_url(url_to_normalize: &str) -> Result<String, BatConfigError> {
        let _normalized = normalizer::UrlNormalizer::new(url_to_normalize)
            .into_report()
            .attach_printable(format!(
                "Error normalizing Miro board url, got {}",
                url_to_normalize
            ))
            .change_context(BatConfigError)?
            .normalize(Some(&["moveToWidget", "cot"]))
            .into_report()
            .attach_printable(format!(
                "Error normalizing Miro board url, got {}",
                url_to_normalize
            ))
            .change_context(BatConfigError)?;

        // Extract board ID and reconstruct canonical URL to discard any trailing garbage
        let board_id =
            crate::batbelt::miro::MiroConfig::get_miro_board_id(url_to_normalize.to_string())
                .change_context(BatConfigError)?;
        Ok(format!("https://miro.com/app/board/{}/", board_id))
    }


    /// Read `Bat.toml` from the project root.
    pub fn get_config() -> Result<Self, BatConfigError> {
        if !Path::new("Bat.toml").exists() {
            let working_directory = std::env::current_dir()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|_| ".".to_string());
            return Err(Report::new(BatConfigError)
                .attach_printable(format!("no Bat.toml in {working_directory}"))
                .attach(crate::Suggestion(
                    "run `bat-cli init` in the project root".to_string(),
                )));
        }

        let path = BatFile::BatToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let mut bat_config: BatConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("cannot parse Bat.toml")?;

        if bat_config.program_name.is_empty() {
            bat_config.program_name = bat_config
                .program_lib_path
                .trim_end_matches("/src/lib.rs")
                .split('/')
                .next_back()
                .unwrap_or_default()
                .to_string();
        }
        Ok(bat_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        let path = BatFile::BatToml
            .get_path(false)
            .change_context(BatConfigError)?;
        confy::store_path(path, self)
            .into_report()
            .change_context(BatConfigError)
    }

    /// Create BatConfig for a Foundry/Solidity project.
    fn create_foundry_config() -> Result<BatConfig, BatConfigError> {
        // Detect src directory from foundry.toml
        let foundry_content = fs::read_to_string("foundry.toml").unwrap_or_default();
        let src_dir = foundry_content
            .lines()
            .find(|l| l.trim().starts_with("src"))
            .and_then(|l| l.split('=').nth(1))
            .map(|v| v.trim().trim_matches('"').trim_matches('\'').to_string())
            .unwrap_or_else(|| "src".to_string());

        // Verify src directory exists and has .sol files
        let has_sol_files = WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "sol")
                    .unwrap_or(false)
                    && !e.path().to_str().unwrap_or("").contains("test")
                    && !e.path().to_str().unwrap_or("").contains("script")
            });

        if !has_sol_files {
            return Err(Report::new(BatConfigError)
                .attach_printable(format!("No .sol files found in {}/", src_dir)));
        }

        // For Foundry, program_lib_path points to the src directory.
        // Sonar will scan all .sol files; contract selection happens post-sonar.
        let src_path = src_dir.trim_start_matches("./").to_string();
        let program_name = "solidity-contracts".to_string();
        let project_name = Self::current_directory_name();

        let bat_config = BatConfig {
            initialized: true,
            program_name,
            project_name,
            miro_board_url: String::new(),
            program_lib_path: src_path.clone(),
            program_lib_paths: vec![src_path],
            project_type: ProjectType::Foundry,
        };
        bat_config.save().change_context(BatConfigError)?;
        Ok(bat_config)
    }

    pub fn is_multi_program(&self) -> bool {
        self.program_lib_paths.len() > 1
    }

    pub fn prompt_select_program(&self) -> Result<String, BatConfigError> {
        let program_names = self.get_program_names();
        let prompt_text = "Select the program:".to_string();
        let selection = BatDialoguer::select(prompt_text, program_names.clone(), None)
            .change_context(BatConfigError)?;
        Ok(program_names[selection].clone())
    }

    pub fn get_program_lib_path_by_name(&self, program_name: &str) -> Option<String> {
        let paths = if self.program_lib_paths.is_empty() {
            vec![self.program_lib_path.clone()]
        } else {
            self.program_lib_paths.clone()
        };
        paths.into_iter().find(|p| {
            let name = p
                .trim_end_matches("/src/lib.rs")
                .trim_end_matches("/src/main.rs")
                .split('/')
                .next_back()
                .unwrap_or("");
            name == program_name
        })
    }

    pub fn get_program_names(&self) -> Vec<String> {
        let paths = if self.program_lib_paths.is_empty() {
            vec![self.program_lib_path.clone()]
        } else {
            self.program_lib_paths.clone()
        };
        paths
            .iter()
            .map(|p| {
                p.trim_end_matches("/src/lib.rs")
                    .trim_end_matches("/src/main.rs")
                    .split('/')
                    .next_back()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }
}


#[cfg(test)]
mod global_config_test {
    use super::*;

    /// One test because these mutate process-wide environment variables and
    /// `cargo test` runs tests in parallel threads.
    #[test]
    fn test_global_config_round_trips() {
        let _guard = CONFIG_ENV_GUARD.lock().unwrap_or_else(|error| error.into_inner());
        let directory = std::env::temp_dir().join("bat_cli_global_config_test");
        let _ = fs::remove_dir_all(&directory);
        let previous = std::env::var(CONFIG_DIR_ENV).ok();
        std::env::set_var(CONFIG_DIR_ENV, &directory);

        // A missing file reads as defaults rather than failing.
        assert_eq!(BatGlobalConfig::load().unwrap(), BatGlobalConfig::default());

        let global = BatGlobalConfig {
            auditor_name: "matiasbn".to_string(),
            use_code_editor: true,
            code_editor: CodeEditor::VSCode,
        };
        global.save().unwrap();
        assert_eq!(BatGlobalConfig::load().unwrap(), global);

        match previous {
            Some(previous) => std::env::set_var(CONFIG_DIR_ENV, previous),
            None => std::env::remove_var(CONFIG_DIR_ENV),
        }
        let _ = fs::remove_dir_all(&directory);
    }
}
