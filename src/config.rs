use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use std::path::Path;
use std::{error::Error, fmt, fs, str};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use crate::batbelt::path::BatFile;
use crate::batbelt::{bat_dialoguer, BatEnumerator};

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
        bat_auditor_config.prompt_external_bat_metadata()?;
        bat_auditor_config.save()?;
        Ok(bat_auditor_config)
    }

    pub fn prompt_external_bat_metadata(&mut self) -> BatConfigResult<()> {
        let prompt_text = if self.external_bat_metadata.is_empty() {
            format!(
                "Do you want to add external {} files?",
                BatFile::BatMetadataFile
                    .get_file_name()
                    .change_context(BatConfigError)?
                    .bright_green()
            )
        } else {
            format!(
                "Do you want to update the external {} files?",
                BatFile::BatMetadataFile
                    .get_file_name()
                    .change_context(BatConfigError)?
                    .bright_green()
            )
        };
        let add_external_metadata =
            BatDialoguer::select_yes_or_no(prompt_text).change_context(BatConfigError)?;
        return if add_external_metadata {
            println!(
                "Looking for {} files on the parent directory (..) \n",
                "BatMetadata.json".bright_green()
            );
            let bat_metadata_folders = WalkDir::new("..")
                .into_iter()
                .map(|f| f.unwrap())
                .filter(|f| {
                    f.file_type().is_dir()
                        && ![".", "target"]
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
        } else {
            Ok(())
        };
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
    pub program_name: String,
    pub project_repository_url: String,
}

impl BatConfig {
    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let new = Self::create_bat_config_file()?;
        Ok(new)
    }

    fn create_bat_config_file() -> Result<BatConfig, BatConfigError> {
        let local_anchor_project_folders = WalkDir::new(".")
            .into_iter()
            .map(|f| f.unwrap())
            .filter(|f| {
                f.file_type().is_dir()
                    && ![".", "target"]
                        .iter()
                        .any(|y| f.file_name().to_str().unwrap().contains(y))
            })
            .filter(|f| {
                let path = f.path();
                let dir = fs::read_dir(path).unwrap();
                let file_names = dir
                    .map(|f| f.unwrap().file_name().to_str().unwrap().to_string())
                    .collect::<Vec<_>>();

                file_names.contains(&"Anchor.toml".to_string())
            })
            .map(|f| f.path().to_str().unwrap().to_string())
            .collect::<Vec<_>>();
        if local_anchor_project_folders.is_empty() {
            let message =
                "No Anchor projects were found on the current working directory".to_string();
            return Err(Report::new(BatConfigError).attach_printable(message));
        }
        // Folder with the program to audit selection
        let prompt_text = "Select the folder with the program to audit";
        let selection =
            bat_dialoguer::select(prompt_text, local_anchor_project_folders.clone(), None)
                .change_context(BatConfigError)?;
        let selected_folder_path = &local_anchor_project_folders[selection];
        let cargo_programs_files_info = WalkDir::new(selected_folder_path)
            .into_iter()
            .map(|f| f.unwrap())
            .filter(|dir_entry| {
                dir_entry
                    .file_name()
                    .to_str()
                    .unwrap()
                    .contains("Cargo.toml")
                    && !dir_entry.path().to_str().unwrap().contains("target")
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
        let normalized_to_audit_program_lib_path = program_lib_path.replace("./", "../");

        if !Path::new(&program_lib_path).is_file() {
            return Err(Report::new(BatConfigError)
                .attach_printable("lib.rs file not found in selected folder"));
        }

        // Project name selection
        let mut project_name: String = program_name.replace('_', "-") + "-audit";
        let prompt_text = format!(
            "Do you want to use the name {} for this project?",
            project_name.yellow()
        );

        let use_default = if !cfg!(debug_assertions) {
            bat_dialoguer::select_yes_or_no(prompt_text.as_str()).change_context(BatConfigError)?
        } else {
            true
        };

        if !use_default {
            project_name = bat_dialoguer::input("Project name:").change_context(BatConfigError)?;
        }
        let project_path = format!("./{project_name}");

        if Path::new(&project_path).is_dir() {
            return Err(Report::new(BatConfigError)
                .attach_printable(format!("Folder {} already exists", project_name)));
        }

        let auditor_names_prompt: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Auditor names (comma separated, example: alice,bob):")
                .change_context(BatConfigError)?
        } else {
            "test_user".to_string()
        };
        let auditor_names: Vec<String> = auditor_names_prompt
            .split(',')
            .map(|l| l.to_string())
            .collect();

        let client_name: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Client name:").change_context(BatConfigError)?
        } else {
            "test_client".to_string()
        };

        let mut commit_hash_url: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Commit hash url:").change_context(BatConfigError)?
        } else {
            "https://github.com/test_repo/test_program/commit/641bdb72210edcafe555102f2ecd2952a7b60722"
                .to_string()
        };

        commit_hash_url = Self::normalize_commit_hash_url(&commit_hash_url)?;

        let starting_date: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Starting date, example: (01/01/2023):")
                .change_context(BatConfigError)?
        } else {
            "test_date".to_string()
        };

        let mut miro_board_url: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Miro board url:").change_context(BatConfigError)?
        } else {
            "https://miro.com/app/board/uXjVPzsgmiY=/".to_string()
        };

        miro_board_url = Self::normalize_miro_board_url(&miro_board_url)?;

        let project_repository_url: String = if !cfg!(debug_assertions) {
            bat_dialoguer::input("Project repo url, where this audit folder would be pushed:")
                .change_context(BatConfigError)?
        } else {
            "https://github.com/matiasbn/test-repo".to_string()
        };

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
            program_lib_path: normalized_to_audit_program_lib_path,
        };
        bat_config.save().change_context(BatConfigError)?;
        Ok(bat_config)
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
        let bat_config: BatConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing Bat.toml")?;
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
