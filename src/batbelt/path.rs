use crate::batbelt::command_line::{execute_command, CodeEditor};
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
    Batlog,
    FunctionsMetadataFile,
    StructsMetadataFile,
    TraitsMetadataFile,
    ThreatModeling,
    FindingCandidates,
    OpenQuestions,
    ProgramLib,
    Readme { for_init: bool },
    GitIgnore { for_init: bool },
    PackageJson { for_init: bool },
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
            BatFile::Batlog => format!("Batlog.log"),
            BatFile::PackageJson { for_init } => {
                format!(
                    "{}/package.json",
                    if *for_init {
                        BatFolder::ProjectFolderPath.get_path(true)?
                    } else {
                        ".".to_string()
                    }
                )
            }
            BatFile::GitIgnore { for_init } => {
                format!(
                    "{}/.gitignore",
                    if *for_init {
                        BatFolder::ProjectFolderPath.get_path(true)?
                    } else {
                        ".".to_string()
                    }
                )
            }
            BatFile::ProgramLib => {
                BatConfig::get_config()
                    .change_context(BatPathError)?
                    .program_lib_path
            }
            BatFile::Readme { for_init } => {
                format!(
                    "{}/README.md",
                    if *for_init {
                        BatFolder::ProjectFolderPath.get_path(true)?
                    } else {
                        ".".to_string()
                    }
                )
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
        if self.file_exists()? {
            fs::remove_file(&self.get_path(false)?)
                .into_report()
                .change_context(BatPathError)
                .attach_printable(format!(
                    "Error removing file in path:\n {}",
                    self.get_path(false)?
                ))?;
        }
        Ok(())
    }

    pub fn create_empty(&self, canonicalize: bool) -> BatPathResult<()> {
        execute_command("touch", &[&self.get_path(canonicalize)?], false)
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error creating empty file in path:\n {}",
                self.get_path(canonicalize)?
            ))?;
        Ok(())
    }

    pub fn move_file(&self, destination_path: &str) -> BatPathResult<()> {
        execute_command("mv", &[&self.get_path(true)?, destination_path], false)
            .change_context(BatPathError)
            .attach_printable(format!(
                "Error moving file :\n{} \nto path:\n {}",
                self.get_path(true)?,
                destination_path
            ))?;
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

    pub fn get_file_name(&self) -> BatPathResult<String> {
        Ok(Path::new(&self.get_path(false)?)
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
    ProjectFolderPath,
    FindingsFolderPath,
    FindingsToReview,
    FindingsAccepted,
    FindingsRejected,
    CodeOverhaulFolderPath,
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

        let path = match self {
            BatFolder::Notes => "./notes".to_string(),
            BatFolder::ProjectFolderPath => format!("./{}", bat_config.project_name),
            BatFolder::AuditorNotes => {
                let bat_auditor_config =
                    BatAuditorConfig::get_config().change_context(BatPathError)?;

                let auditor_notes_folder_path =
                    format!("./notes/{}-notes", bat_auditor_config.auditor_name);
                auditor_notes_folder_path
            }
            BatFolder::AuditorFigures => {
                format!(
                    "{}/figures",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFolder::Metadata => {
                format!(
                    "{}/metadata",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFolder::ProgramPath => bat_config
                .program_lib_path
                .trim_end_matches("/lib.rs")
                .to_string(),
            BatFolder::FindingsFolderPath => {
                format!("{}/findings", BatFolder::AuditorNotes.get_path(true)?)
            }
            BatFolder::FindingsToReview => {
                format!(
                    "{}/to-review",
                    BatFolder::FindingsFolderPath.get_path(canonicalize)?
                )
            }
            BatFolder::FindingsAccepted => {
                format!(
                    "{}/accepted",
                    BatFolder::FindingsFolderPath.get_path(canonicalize)?
                )
            }
            BatFolder::FindingsRejected => {
                format!(
                    "{}/rejected",
                    BatFolder::FindingsFolderPath.get_path(canonicalize)?
                )
            }
            BatFolder::CodeOverhaulFolderPath => {
                format!(
                    "{}/code-overhaul",
                    BatFolder::AuditorNotes.get_path(canonicalize)?
                )
            }
            BatFolder::CodeOverhaulToReview => {
                format!(
                    "{}/to-review",
                    BatFolder::CodeOverhaulFolderPath.get_path(canonicalize)?
                )
            }
            BatFolder::CodeOverhaulStarted => {
                format!(
                    "{}/started",
                    BatFolder::CodeOverhaulFolderPath.get_path(canonicalize)?
                )
            }
            BatFolder::CodeOverhaulFinished => {
                format!(
                    "{}/finished",
                    BatFolder::CodeOverhaulFolderPath.get_path(canonicalize)?
                )
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
