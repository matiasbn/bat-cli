//! Paths inside a bat project.
//!
//! Everything lives at the root of the audited project rather than under a
//! `bat-audit/` subfolder, so a checkout that contains `Bat.toml` is visibly a
//! bat project. The audit-report tree this module used to describe —
//! code-overhaul stages, finding folders, per-auditor notes — is gone along
//! with the workflow that produced it.

use std::error::Error;
use std::path::Path;
use std::{fmt, fs};

use error_stack::{IntoReport, Result, ResultExt};
use serde::{Deserialize, Serialize};

use crate::batbelt::command_line::{execute_command, CodeEditor};
use crate::batbelt::BatEnumerator;
use crate::config::BatConfig;

#[derive(Debug)]
pub struct BatPathError;

impl fmt::Display for BatPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatPath error")
    }
}

impl Error for BatPathError {}

pub type BatPathResult<T> = Result<T, BatPathError>;

#[derive(
    Debug, PartialEq, Clone, strum_macros::Display, strum_macros::EnumIter, Serialize, Deserialize,
)]
pub enum BatFile {
    BatToml,
    BatMetadataFile,
    GitIgnore,
    ProgramLib,
    Generic { file_path: String },
}

impl BatEnumerator for BatFile {}

impl BatFile {
    pub fn get_path(&self, canonicalize: bool) -> BatPathResult<String> {
        let path = match self {
            BatFile::BatToml => "Bat.toml".to_string(),
            BatFile::BatMetadataFile => "BatMetadata.json".to_string(),
            BatFile::GitIgnore => "./.gitignore".to_string(),
            BatFile::ProgramLib => {
                BatConfig::get_config()
                    .change_context(BatPathError)?
                    .program_lib_path
            }
            BatFile::Generic { file_path } => file_path.clone(),
        };

        if canonicalize {
            return canonicalize_path(path);
        }
        Ok(path)
    }

    pub fn read_content(&self, canonicalize: bool) -> BatPathResult<String> {
        let path = self.get_path(canonicalize)?;
        fs::read_to_string(&path)
            .into_report()
            .change_context(BatPathError)
            .attach_printable_lazy(|| format!("cannot read {path}"))
    }

    pub fn write_content(&self, canonicalize: bool, content: &str) -> BatPathResult<()> {
        let path = self.get_path(canonicalize)?;
        log::debug!("{}.write_content:\n{}", self, content);
        fs::write(&path, content)
            .into_report()
            .change_context(BatPathError)
            .attach_printable_lazy(|| format!("cannot write {path}"))
    }

    pub fn remove_file(&self) -> BatPathResult<()> {
        if self.file_exists()? {
            let path = self.get_path(false)?;
            fs::remove_file(&path)
                .into_report()
                .change_context(BatPathError)
                .attach_printable_lazy(|| format!("cannot remove {path}"))?;
        }
        Ok(())
    }

    pub fn create_empty(&self, canonicalize: bool) -> BatPathResult<()> {
        let path = self.get_path(canonicalize)?;
        execute_command("touch", &[&path], false)
            .change_context(BatPathError)
            .attach_printable_lazy(|| format!("cannot create {path}"))?;
        Ok(())
    }

    pub fn open_in_editor(
        &self,
        canonicalize: bool,
        line_index: Option<usize>,
    ) -> BatPathResult<()> {
        CodeEditor::open_file_in_editor(&self.get_path(canonicalize)?, line_index)
            .change_context(BatPathError)
    }

    pub fn file_exists(&self) -> BatPathResult<bool> {
        Ok(Path::new(&self.get_path(false)?).is_file())
    }
}

#[derive(
    Debug, PartialEq, Clone, strum_macros::Display, strum_macros::EnumIter, Serialize, Deserialize,
)]
pub enum BatFolder {
    /// Directory of the program or contracts being audited.
    ProgramPath,
    /// Where silicon writes the screenshots before they are uploaded.
    Figures,
}

impl BatEnumerator for BatFolder {}

impl BatFolder {
    pub fn get_path(&self, canonicalize: bool) -> BatPathResult<String> {
        let path = match self {
            BatFolder::Figures => "figures".to_string(),
            BatFolder::ProgramPath => BatConfig::get_config()
                .change_context(BatPathError)?
                .program_lib_path
                .trim_end_matches("/src/lib.rs")
                .trim_end_matches("/src/main.rs")
                .to_string(),
        };

        if canonicalize {
            return canonicalize_path(path);
        }
        Ok(path)
    }

    /// Every program directory to scan. Multi-program projects list them all.
    pub fn get_all_program_paths() -> BatPathResult<Vec<String>> {
        let bat_config = BatConfig::get_config().change_context(BatPathError)?;
        let trim = |path: &str| {
            path.trim_end_matches("/src/lib.rs")
                .trim_end_matches("/src/main.rs")
                .to_string()
        };
        if bat_config.program_lib_paths.is_empty() {
            return Ok(vec![trim(&bat_config.program_lib_path)]);
        }
        Ok(bat_config
            .program_lib_paths
            .iter()
            .map(|path| trim(path))
            .collect())
    }

    pub fn folder_exists(&self) -> BatPathResult<bool> {
        Ok(Path::new(&self.get_path(false)?).is_dir())
    }

    pub fn create_folder(&self) -> BatPathResult<()> {
        let path = self.get_path(false)?;
        fs::create_dir_all(&path)
            .into_report()
            .change_context(BatPathError)
            .attach_printable_lazy(|| format!("cannot create {path}"))
    }
}

/// Shorten a source path for display, dropping the prefix above the program.
pub fn prettify_source_code_path(path: &str) -> BatPathResult<String> {
    if path.ends_with(".sol") {
        return Ok(path.trim_start_matches("./").trim_start_matches("../").to_string());
    }
    let mut path_split = path.split("/src/");
    let prefix_with_program = path_split.next().unwrap_or("");
    let program_name = prefix_with_program.split('/').next_back().unwrap_or("");
    let prefix = prefix_with_program.trim_end_matches(program_name);
    Ok(path.trim_start_matches(prefix).to_string())
}

pub fn canonicalize_path(path_to_canonicalize: String) -> BatPathResult<String> {
    let canonicalized = Path::new(&path_to_canonicalize)
        .canonicalize()
        .into_report()
        .change_context(BatPathError)
        .attach_printable_lazy(|| format!("path does not exist: {path_to_canonicalize}"))?;
    Ok(canonicalized.to_string_lossy().to_string())
}
