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
use crate::batbelt::{bat_dialoguer, BatEnumerator};

use crate::batbelt::git::git_commit::GitCommit;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use normalize_url::normalizer;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct BatConfigError;

impl fmt::Display for BatConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatConfig error")
    }
}

impl Error for BatConfigError {}

pub type BatConfigResult<T> = Result<T, BatConfigError>;

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialOrd, PartialEq)]
pub struct BatAuditorConfig {
    pub auditor_name: String,
    pub miro_oauth_access_token: String,
    #[serde(default)]
    pub use_code_editor: bool,
    #[serde(default)]
    pub code_editor: CodeEditor,
    #[serde(default)]
    pub external_bat_metadata: Vec<String>,
}

impl BatAuditorConfig {
    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let mut bat_auditor_config = BatAuditorConfig {
            auditor_name: "".to_string(),
            miro_oauth_access_token: "".to_string(),
            use_code_editor: false,
            code_editor: Default::default(),
            external_bat_metadata: vec![],
        };
        bat_auditor_config.prompt_auditor_name()?;
        bat_auditor_config.prompt_code_editor_integration()?;
        bat_auditor_config.get_external_bat_metadata()?;
        bat_auditor_config.save()?;
        Ok(bat_auditor_config)
    }

    pub fn get_external_bat_metadata(&mut self) -> BatConfigResult<()> {
        let BatConfig { project_name, .. } =
            BatConfig::get_config().change_context(BatConfigError)?;
        println!(
            "Looking for {} files on the parent directory (..) \n",
            "BatMetadata.json".bright_green()
        );
        let bat_metadata_folders = WalkDir::new("..")
            .into_iter()
            .map(|f| f.unwrap())
            .filter(|f| {
                f.file_type().is_dir()
                    && ![".", "target", &project_name]
                        .iter()
                        .any(|y| f.file_name().to_str().unwrap().contains(y))
            })
            .filter(|f| {
                let path = f.path();
                let dir = fs::read_dir(path).unwrap();
                let file_names = dir
                    .map(|f| f.unwrap().file_name().to_str().unwrap().to_string())
                    .collect::<Vec<_>>();

                file_names.contains(&"BatMetadata.json".to_string())
            })
            .map(|f| format!("{}/BatMetadata.json", f.path().to_str().unwrap()))
            .collect::<Vec<_>>();
        if bat_metadata_folders.is_empty() {
            println!(
                "0 folders with {} file were found on the parent directory (..) \n",
                "BatMetadata.json".bright_green()
            );
            println!(
                "You can add folders with {} manually on BatAuditor.toml, section {}",
                "BatMetadata.json".bright_green(),
                "external_bat_metadata".bright_blue()
            );
            return Ok(());
        }
        println!(
            "Adding these {} files to external_bat_metadata :\n{:#?}",
            "BatMetadata.json".bright_green(),
            bat_metadata_folders
        );
        self.external_bat_metadata = bat_metadata_folders;
        Ok(())
    }

    fn prompt_auditor_name(&mut self) -> BatConfigResult<()> {
        let bat_config = BatConfig::get_config()?;
        let auditor_names = bat_config.auditor_names;
        let prompt_text = "Select your name:".to_string();
        let selection = BatDialoguer::select(prompt_text, auditor_names.clone(), None)
            .change_context(BatConfigError)?;
        let auditor_name = auditor_names.get(selection).unwrap().clone();
        self.auditor_name = auditor_name;
        Ok(())
    }

    fn prompt_code_editor_integration(&mut self) -> BatConfigResult<()> {
        let prompt_text = format!(
            "Select a code editor, choose {} to disable:",
            CodeEditor::None.get_colored_name(false)
        );
        let editor_colorized_vec = CodeEditor::get_colorized_type_vec(false);
        let editor_integration = BatDialoguer::select(prompt_text, editor_colorized_vec, None)
            .change_context(BatConfigError)?;
        self.code_editor = CodeEditor::from_index(editor_integration);
        self.use_code_editor = self.code_editor != CodeEditor::None;
        Ok(())
    }

    pub fn get_config() -> Result<Self, BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let bat_auditor_config: BatAuditorConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing BatAuditor.toml")?;
        bat_auditor_config.save()?;
        Ok(bat_auditor_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(false)
            .change_context(BatConfigError)?;
        confy::store_path(path, self)
            .into_report()
            .change_context(BatConfigError)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub enum ProjectType {
    Anchor,
    Pinocchio,
    VanillaSolana,
    #[default]
    GenericRust,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BatConfig {
    pub initialized: bool,
    pub project_name: String,
    pub client_name: String,
    pub commit_hash_url: String,
    pub starting_date: String,
    pub miro_board_url: String,
    pub auditor_names: Vec<String>,
    /// Primary program lib path (first selected, used by Anchor entrypoint detection).
    pub program_lib_path: String,
    /// All selected program lib paths (for multi-program scanning).
    #[serde(default)]
    pub program_lib_paths: Vec<String>,
    #[serde(default)]
    pub program_name: String,
    pub project_repository_url: String,
    #[serde(default)]
    pub project_type: ProjectType,
}

impl BatConfig {
    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let new = Self::create_bat_config_file()?;
        Ok(new)
    }

    fn create_bat_config_file() -> Result<BatConfig, BatConfigError> {
        // Auto-detect project type (initial guess; refined after Cargo.toml discovery)
        let mut project_type = if Path::new("Anchor.toml").is_file() {
            println!("Detected {} project (Anchor.toml found)", "Anchor".green());
            ProjectType::Anchor
        } else {
            ProjectType::GenericRust
        };

        // Validate bat-audit doesn't already exist
        if Path::new("bat-audit").is_dir() {
            return Err(
                Report::new(BatConfigError).attach_printable("bat-audit/ folder already exists")
            );
        }

        // Auto-detect git remote info
        let (remote_https_url, owner_name, commit_hash) = Self::detect_remote_info(".")
            .unwrap_or(("".to_string(), "".to_string(), "".to_string()));

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
                let continue_anyway =
                    bat_dialoguer::select_yes_or_no("Do you want to continue?")
                        .change_context(BatConfigError)?;
                if !continue_anyway {
                    return Err(
                        Report::new(BatConfigError).attach_printable("Aborted by user")
                    );
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

        // Project name is always bat-audit
        let project_name = "bat-audit".to_string();

        // Auditor names - always manual input
        let auditor_names_prompt: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Auditor names (comma separated, example: alice,bob):")
                .change_context(BatConfigError)?
        } else {
            "test_user".to_string()
        };
        let auditor_names: Vec<String> = auditor_names_prompt
            .split(',')
            .map(|l| l.trim().to_string())
            .collect();

        // Client name - default to repo owner
        let client_name: String = if !cfg!(debug_assertions) {
            if owner_name.is_empty() {
                bat_dialoguer::input("Client name:").change_context(BatConfigError)?
            } else {
                bat_dialoguer::input_with_default("Client name:", &owner_name)
                    .change_context(BatConfigError)?
            }
        } else {
            "test_client".to_string()
        };

        // Commit hash URL - auto-detect from git
        let default_commit_url = if !remote_https_url.is_empty() && !commit_hash.is_empty() {
            format!("{}/commit/{}", remote_https_url, commit_hash)
        } else {
            String::new()
        };

        let mut commit_hash_url: String = if !cfg!(debug_assertions) {
            if default_commit_url.is_empty() {
                bat_dialoguer::input("Commit hash url:").change_context(BatConfigError)?
            } else {
                bat_dialoguer::input_with_default("Commit hash url:", &default_commit_url)
                    .change_context(BatConfigError)?
            }
        } else {
            "https://github.com/test_repo/test_program/commit/641bdb72210edcafe555102f2ecd2952a7b60722"
                .to_string()
        };

        commit_hash_url = Self::normalize_commit_hash_url(&commit_hash_url)?;

        // Starting date - default to today
        let today = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let days = now / 86400;
            let years = (days * 4 + 2) / 1461;
            let day_of_year = days - (365 * years + years / 4 - years / 100 + years / 400);
            let month_days: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let year = 1970 + years;
            let is_leap =
                (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400);
            let mut remaining = day_of_year;
            let mut month = 0u64;
            for (i, &d) in month_days.iter().enumerate() {
                let d = if i == 1 && is_leap { d + 1 } else { d };
                if remaining < d {
                    month = i as u64 + 1;
                    break;
                }
                remaining -= d;
            }
            let day = remaining + 1;
            format!("{:02}/{:02}/{}", day, month, year)
        };

        let starting_date: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input_with_default("Starting date:", &today)
                .change_context(BatConfigError)?
        } else {
            today
        };

        // Miro board URL - set to "none" by default (configured later in new_bat_project if user wants Miro)
        let miro_board_url = "none".to_string();

        // Project repository URL - auto-detect from git remote
        let project_repository_url = remote_https_url.clone();

        let bat_config = BatConfig {
            initialized: true,
            program_name,
            auditor_names,
            project_name,
            client_name,
            miro_board_url,
            starting_date,
            commit_hash_url,
            project_repository_url,
            program_lib_path: normalized_program_lib_path,
            program_lib_paths: normalized_program_lib_paths,
            project_type,
        };
        bat_config.save().change_context(BatConfigError)?;
        Ok(bat_config)
    }

    /// Detects remote URL, owner name, and latest commit hash from a git repo
    fn detect_remote_info(repo_path: &str) -> Option<(String, String, String)> {
        let remote_output = Command::new("git")
            .args(["-C", repo_path, "remote", "get-url", "origin"])
            .output()
            .ok()?;
        let remote_raw = String::from_utf8(remote_output.stdout)
            .ok()?
            .trim()
            .to_string();

        // Convert SSH URL to HTTPS if needed
        // git@github.com:owner/repo.git -> https://github.com/owner/repo
        let remote_https = if remote_raw.starts_with("git@") {
            let without_prefix = remote_raw.trim_start_matches("git@");
            // Replace the first ":" (host:path separator) with "/"
            let converted = without_prefix.replacen(":", "/", 1);
            format!("https://{}", converted.trim_end_matches(".git"))
        } else {
            remote_raw.trim_end_matches(".git").to_string()
        };

        // Extract owner from URL: https://github.com/owner/repo -> owner
        let parts: Vec<&str> = remote_https.split('/').collect();
        let owner = if parts.len() >= 4 {
            parts[parts.len() - 2].to_string()
        } else {
            String::new()
        };

        let hash_output = Command::new("git")
            .args(["-C", repo_path, "log", "-1", "--format=%H"])
            .output()
            .ok()?;
        let commit_hash = String::from_utf8(hash_output.stdout)
            .ok()?
            .trim()
            .to_string();

        Some((remote_https, owner, commit_hash))
    }

    pub fn normalize_miro_board_url(url_to_normalize: &str) -> Result<String, BatConfigError> {
        let url = normalizer::UrlNormalizer::new(url_to_normalize)
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
        Ok(url)
    }

    fn normalize_commit_hash_url(url_to_normalize: &str) -> Result<String, BatConfigError> {
        let url = normalizer::UrlNormalizer::new(url_to_normalize)
            .into_report()
            .attach_printable(format!(
                "Error normalizing commit hash url, got {}",
                url_to_normalize
            ))
            .change_context(BatConfigError)?
            .normalize(None)
            .into_report()
            .attach_printable(format!(
                "Error normalizing commit hash url, got {}",
                url_to_normalize
            ))
            .change_context(BatConfigError)?;
        Ok(url)
    }

    pub fn get_config() -> Result<Self, BatConfigError> {
        let path = BatFile::BatToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let mut bat_config: BatConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing Bat.toml")?;
        if bat_config.program_name.is_empty() {
            bat_config.program_name = bat_config
                .program_lib_path
                .clone()
                .trim_end_matches("/src/lib.rs")
                .split("/")
                .last()
                .unwrap()
                .to_string();
            bat_config.save()?;

            GitCommit::UpdateBatToml
                .create_commit(true)
                .change_context(BatConfigError)?;
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
