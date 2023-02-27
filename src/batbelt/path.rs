use crate::batbelt::command_line::{execute_command, vs_code_open_file_in_current_window};
use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::BatMetadataType;
use crate::config::{BatAuditorConfig, BatConfig};
use clap::builder::Str;
use error_stack::{FutureExt, IntoReport, Result, ResultExt};
use inflector::Inflector;
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs, path::Path};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
pub struct BatPathError;

impl fmt::Display for BatPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatPath error")
    }
}

impl Error for BatPathError {}

type BatPathResult<T> = Result<T, BatPathError>;

pub enum BatFile {
    BatToml,
    BatAuditorToml,
    FunctionsMetadataFile,
    StructsMetadataFile,
    TraitsMetadataFile,
    ThreatModeling,
    FindingCandidates,
    OpenQuestions,
    ProgramLib,
    Readme,
    PackageJson,
    RobotFile,
    CodeOverhaulToReview { file_name: String },
    CodeOverhaulStarted { file_name: String },
    CodeOverhaulFinished { file_name: String },
    FindingToReview { file_name: String },
    FindingAccepted { file_name: String },
    FindingRejected { file_name: String },
    Generic { file_path: String },
}

impl BatFile {
    pub fn get_path(&self, canonicalize: bool) -> BatPathResult<String> {
        let path = match self {
            BatFile::BatToml => "Bat.toml".to_string(),
            BatFile::BatAuditorToml => "BatAuditor.toml".to_string(),
            BatFile::PackageJson => "./package.json".to_string(),
            BatFile::ProgramLib => {
                BatConfig::get_config()
                    .change_context(BatPathError)?
                    .program_lib_path
            }
            BatFile::Readme => {
                format!("./README.md")
            }
            BatFile::RobotFile => format!(
                "{}/robot.md",
                BatFolder::AuditorNotes.get_path(canonicalize)?
            ),
            BatFile::FindingCandidates => {
                format!(
                    "{}/finding_candidates.md",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFile::OpenQuestions => {
                format!(
                    "{}/open_questions.md",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFile::ThreatModeling => {
                format!(
                    "{}/threat_modeling.md",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFile::StructsMetadataFile => {
                format!(
                    "{}/{}.md",
                    BatFolder::Metadata.get_path(canonicalize)?,
                    BatMetadataType::Struct
                        .to_string()
                        .to_lowercase()
                        .to_plural()
                )
            }
            BatFile::FunctionsMetadataFile => {
                format!(
                    "{}/{}.md",
                    BatFolder::Metadata.get_path(canonicalize)?,
                    BatMetadataType::Function
                        .to_string()
                        .to_lowercase()
                        .to_plural()
                )
            }
            BatFile::TraitsMetadataFile => {
                format!(
                    "{}/{}.md",
                    BatFolder::Metadata.get_path(canonicalize)?,
                    BatMetadataType::Trait
                        .to_string()
                        .to_lowercase()
                        .to_plural()
                )
            }
            BatFile::CodeOverhaulToReview { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulToReview.get_path(canonicalize)?
                )
            }
            BatFile::CodeOverhaulStarted { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulStarted.get_path(canonicalize)?
                )
            }
            BatFile::CodeOverhaulFinished { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulFinished.get_path(canonicalize)?
                )
            }
            BatFile::FindingToReview { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsToReview.get_path(canonicalize)?
                )
            }
            BatFile::FindingAccepted { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsAccepted.get_path(canonicalize)?
                )
            }
            BatFile::FindingRejected { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsRejected.get_path(canonicalize)?
                )
            }
            BatFile::Generic { file_path } => file_path.clone(),
        };

        if canonicalize {
            return canonicalize_path(path);
        }

        Ok(path)
    }

    pub fn read_content(&self, canonicalize: bool) -> BatPathResult<String> {
        fs::read_to_string(self.get_path(canonicalize)?)
            .into_report()
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error reading content for file in path:\n {}",
                self.get_path(canonicalize)?
            ))
    }

    pub fn write_content(&self, canonicalize: bool, content: &str) -> BatPathResult<()> {
        fs::write(&self.get_path(canonicalize)?, content)
            .into_report()
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error writing content for file in path:\n {}",
                self.get_path(canonicalize)?
            ))
    }

    pub fn remove_file(&self) -> BatPathResult<()> {
        fs::remove_file(&self.get_path(true)?)
            .into_report()
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error removing file in path:\n {}",
                self.get_path(true)?
            ))
    }

    pub fn create_empty(&self, canonicalize: bool) -> BatPathResult<()> {
        execute_command("touch", &[&self.get_path(canonicalize)?])
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error creating empty file in path:\n {}",
                self.get_path(canonicalize)?
            ))?;
        Ok(())
    }

    pub fn move_file(&self, destination_path: &str) -> BatPathResult<()> {
        execute_command("mv", &[&self.get_path(true)?, destination_path])
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error moving file :\n{} \nto path:\n {}",
                self.get_path(true)?,
                destination_path
            ))?;
        Ok(())
    }

    pub fn open_in_vs_code(&self) -> BatPathResult<()> {
        vs_code_open_file_in_current_window(&self.get_path(true)?).change_context(BatPathError)?;
        Ok(())
    }

    pub fn file_exists(&self) -> BatPathResult<bool> {
        Ok(Path::new(&self.get_path(false)?).is_file())
    }

    pub fn get_file_name(&self) -> BatPathResult<String> {
        Ok(Path::new(&self.get_path(true)?)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string())
    }

    pub fn to_markdown_file(&self, content: String) -> BatPathResult<MarkdownFile> {
        Ok(MarkdownFile::new_from_path_and_content(
            &self.get_path(false)?,
            content,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BatFolder {
    Metadata,
    ProgramPath,
    FindingsToReview,
    FindingsAccepted,
    FindingsRejected,
    CodeOverhaulToReview,
    CodeOverhaulStarted,
    CodeOverhaulFinished,
    AuditorNotes,
    AuditorFigures,
    Notes,
}

impl BatFolder {
    pub fn get_path(&self, canonicalize: bool) -> Result<String, BatPathError> {
        let bat_config = BatConfig::get_config().change_context(BatPathError)?;
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(BatPathError)?;

        let auditor_notes_folder_path =
            format!("./notes/{}-notes", bat_auditor_config.auditor_name);
        let findings_path = format!("{}/findings", auditor_notes_folder_path);
        let code_overhaul_path = format!("{}/code-overhaul", auditor_notes_folder_path);

        let path = match self {
            BatFolder::Notes => "./notes".to_string(),
            BatFolder::AuditorNotes => auditor_notes_folder_path,
            BatFolder::AuditorFigures => format!("{auditor_notes_folder_path}/figures"),
            BatFolder::Metadata => format!("{auditor_notes_folder_path}/metadata"),
            BatFolder::ProgramPath => bat_config
                .program_lib_path
                .trim_end_matches("/lib.rs")
                .to_string(),
            BatFolder::FindingsToReview => {
                format!("{}/to-review", findings_path)
            }
            BatFolder::FindingsAccepted => {
                format!("{}/accepted", findings_path)
            }
            BatFolder::FindingsRejected => {
                format!("{}/rejected", findings_path)
            }
            BatFolder::CodeOverhaulToReview => {
                format!("{}/to-review", code_overhaul_path)
            }
            BatFolder::CodeOverhaulStarted => {
                format!("{}/started", code_overhaul_path)
            }
            BatFolder::CodeOverhaulFinished => {
                format!("{}/finished", code_overhaul_path)
            }
        };

        if canonicalize {
            return canonicalize_path(path);
        }

        Ok(path)
    }

    pub fn get_all_files_dir_entries(
        &self,
        sorted: bool,
        file_name_to_exclude_filters: Option<Vec<String>>,
        file_extension_to_include_filters: Option<Vec<String>>,
    ) -> Result<Vec<DirEntry>, BatPathError> {
        let folder_path = self.get_path(false)?;
        let mut dir_entries = WalkDir::new(folder_path)
            .into_iter()
            .filter_map(|f| {
                let dir_entry = f.unwrap();
                if !dir_entry.metadata().unwrap().is_file() || dir_entry.file_name() == ".gitkeep" {
                    return None;
                }
                if file_name_to_exclude_filters.is_some()
                    && file_name_to_exclude_filters
                        .clone()
                        .unwrap()
                        .into_iter()
                        .any(|filter| dir_entry.file_name().to_str().unwrap() == filter)
                {
                    return None;
                }
                if file_extension_to_include_filters.is_some()
                    && !file_extension_to_include_filters
                        .clone()
                        .unwrap()
                        .into_iter()
                        .any(|filter| dir_entry.file_name().to_str().unwrap().ends_with(&filter))
                {
                    return None;
                }
                Some(dir_entry)
            })
            .collect::<Vec<_>>();
        if sorted {
            dir_entries.sort_by(|dir_entry_a, dir_entry_b| {
                dir_entry_a.file_name().cmp(&dir_entry_b.file_name())
            });
        }
        Ok(dir_entries)
    }

    pub fn get_all_files_names(
        &self,
        sorted: bool,
        file_name_to_exclude_filters: Option<Vec<String>>,
        file_extension_to_include_filters: Option<Vec<String>>,
    ) -> Result<Vec<String>, BatPathError> {
        let dir_entries = self.get_all_files_dir_entries(
            sorted,
            file_name_to_exclude_filters,
            file_extension_to_include_filters,
        )?;
        Ok(dir_entries
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>())
    }

    pub fn get_all_bat_files(
        &self,
        sorted: bool,
        file_name_to_exclude_filters: Option<Vec<String>>,
        file_extension_to_include_filters: Option<Vec<String>>,
    ) -> BatPathResult<Vec<BatFile>> {
        let generic_vec = self
            .get_all_files_dir_entries(
                sorted,
                file_name_to_exclude_filters,
                file_extension_to_include_filters,
            )?
            .into_iter()
            .map(|entry| BatFile::Generic {
                file_path: entry.path().to_str().unwrap().to_string(),
            })
            .collect::<Vec<_>>();
        Ok(generic_vec)
    }

    pub fn folder_exists(&self) -> BatPathResult<bool> {
        Ok(Path::new(&self.get_path(false)?).is_dir())
    }
}

pub fn get_file_path(file_type: BatFile, canonicalize: bool) -> Result<String, BatPathError> {
    Ok(file_type.get_path(canonicalize)?)
}

pub fn get_folder_path(folder_type: BatFolder, canonicalize: bool) -> Result<String, BatPathError> {
    Ok(folder_type.get_path(canonicalize)?)
}

pub fn canonicalize_path(path_to_canonicalize: String) -> Result<String, BatPathError> {
    let error_message = format!("Error canonicalizing path: {}", path_to_canonicalize);
    let canonicalized_path = Path::new(&(path_to_canonicalize))
        .canonicalize()
        .into_report()
        .change_context(BatPathError)
        .attach_printable(error_message)?
        .into_os_string()
        .into_string()
        .unwrap();
    Ok(canonicalized_path)
}
