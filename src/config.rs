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
use error_stack::{FutureExt, IntoReport, Report, Result, ResultExt};
use figment::error::Kind;
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
        bat_auditor_config.prompt_miro_integration()?;
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
            .map(|f| {
                format!(
                    "{}/BatMetadata.json",
                    f.path().to_str().unwrap().to_string()
                )
            })
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

    fn prompt_miro_integration(&mut self) -> BatConfigResult<()> {
        let prompt_text = "Do you want to use the Miro integration?";
        let include_miro = BatDialoguer::select_yes_or_no(prompt_text.to_string())
            .change_context(BatConfigError)?;
        let moat = if include_miro {
            let prompt_text = "Miro OAuth access token";
            BatDialoguer::input(prompt_text.to_string()).change_context(BatConfigError)?
        } else {
            "".to_string()
        };
        self.miro_oauth_access_token = moat;
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

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BatConfig {
    pub initialized: bool,
    pub project_name: String,
    pub client_name: String,
    pub commit_hash_url: String,
    pub starting_date: String,
    pub miro_board_url: String,
    pub auditor_names: Vec<String>,
    pub program_lib_path: String,
    #[serde(default)]
    pub program_name: String,
    pub project_repository_url: String,
}

impl BatConfig {
    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let new = Self::create_bat_config_file()?;
        Ok(new)
    }

    fn create_bat_config_file() -> Result<BatConfig, BatConfigError> {
        // Validate we're inside an Anchor project
        if !Path::new("Anchor.toml").is_file() {
            return Err(Report::new(BatConfigError).attach_printable(
                "Anchor.toml not found in current directory. Run bat new inside the target repo.",
            ));
        }

        // Validate bat-audit doesn't already exist
        if Path::new("bat-audit").is_dir() {
            return Err(Report::new(BatConfigError)
                .attach_printable("bat-audit/ folder already exists"));
        }

        // Auto-detect git remote info
        let (remote_https_url, owner_name, commit_hash) = Self::detect_remote_info(".")
            .unwrap_or(("".to_string(), "".to_string(), "".to_string()));

        // Find programs with Cargo.toml (excluding target/)
        let cargo_programs_files_info = WalkDir::new(".")
            .into_iter()
            .map(|f| f.unwrap())
            .filter(|dir_entry| {
                dir_entry
                    .file_name()
                    .to_str()
                    .unwrap()
                    .contains("Cargo.toml")
                    && !dir_entry.path().to_str().unwrap().contains("target")
                    && dir_entry.path().to_str().unwrap() != "./Cargo.toml"
            })
            .collect::<Vec<_>>();

        // Program to audit selection
        let prompt_text = "Select the program to audit";
        let cargo_programs_paths = cargo_programs_files_info
            .iter()
            .map(|f| {
                f.path()
                    .to_str()
                    .unwrap()
                    .trim_end_matches("/Cargo.toml")
                    .to_string()
            })
            .collect::<Vec<_>>();

        if cargo_programs_paths.is_empty() {
            return Err(Report::new(BatConfigError)
                .attach_printable("No programs with Cargo.toml found in this repository"));
        }

        let selection = bat_dialoguer::select(prompt_text, cargo_programs_paths.clone(), None)
            .change_context(BatConfigError)?;
        let selected_program_path = &cargo_programs_paths[selection];
        log::debug!("selected_program: {:#?}", selected_program_path);
        let program_name = selected_program_path
            .split('/')
            .last()
            .unwrap()
            .to_string()
            .replace('_', "-");
        log::debug!("program_name: {:#?}", program_name);
        let program_lib_path = format!("{}/src/lib.rs", selected_program_path);
        log::debug!("program_lib_path: {:#?}", program_lib_path);

        if !Path::new(&program_lib_path).is_file() {
            return Err(Report::new(BatConfigError)
                .attach_printable("lib.rs file not found in selected folder"));
        }

        // Normalize path relative to bat-audit/ folder
        // From bat-audit/, we need ../programs/xxx/src/lib.rs
        let normalized_program_lib_path = format!(
            "../{}",
            program_lib_path.trim_start_matches("./")
        );

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
            let is_leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
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

        // Miro board URL - default to "none"
        let mut miro_board_url: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input_with_default("Miro board url:", "none")
                .change_context(BatConfigError)?
        } else {
            "none".to_string()
        };

        if miro_board_url != "none" {
            miro_board_url = Self::normalize_miro_board_url(&miro_board_url)?;
        }

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
        let remote_raw = String::from_utf8(remote_output.stdout).ok()?.trim().to_string();

        // Convert SSH URL to HTTPS if needed
        let remote_https = if remote_raw.starts_with("git@") {
            // git@github.com:owner/repo.git -> https://github.com/owner/repo
            remote_raw
                .replace("git@", "https://")
                .replace(":", "/")
                .replace(".git", "")
                .replacen("//", "//", 1)
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
        let commit_hash = String::from_utf8(hash_output.stdout).ok()?.trim().to_string();

        Some((remote_https, owner, commit_hash))
    }

    fn normalize_miro_board_url(url_to_normalize: &str) -> Result<String, BatConfigError> {
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
}
