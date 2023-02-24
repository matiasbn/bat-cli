use crate::config::{BatAuditorConfig, BatConfig};
use error_stack::{IntoReport, Result, ResultExt};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, path::Path};
use walkdir::{DirEntry, WalkDir};

#[derive(Debug)]
pub struct BatPathError;

impl fmt::Display for BatPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatPath error")
    }
}

impl Error for BatPathError {}

pub enum BatFile {
    BatToml,
    BatAuditorToml,
    FunctionsMetadata,
    StructsMetadata,
    TraitMetadata,
    TraitImplMetadata,
    EntrypointsMetadata,
    ThreatModeling,
    AuditResult,
    FindingsResult,
    FindingsRobotResult,
    CodeOverhaulResult,
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
}

impl BatFile {
    pub fn get_path(&self, canonicalize: bool) -> Result<String, BatPathError> {
        let path = match self {
            BatFile::BatToml => "Bat.toml".to_string(),
            BatFile::BatAuditorToml => "BatAuditor.toml".to_string(),
            BatFile::PackageJson => "./package.json".to_string(),
            BatFile::ProgramLib => {
                BatConfig::get_config()
                    .change_context(BatPathError)?
                    .program_lib_path
            }
            BatFile::AuditResult => {
                format!("./audit_result.md")
            }
            BatFile::FindingsResult => {
                format!("./audit_result/findings_result.md")
            }
            BatFile::FindingsRobotResult => {
                format!("./audit_result/02_findings_result.md")
            }
            BatFile::CodeOverhaulResult => {
                format!("./audit_result/co_result.md")
            }
            BatFile::Readme => {
                format!("./README.md")
            }
            BatFile::RobotFile => format!("{}/robot.md", BatFolder::AuditorNotes.get_path(false)?),
            BatFile::FindingCandidates => {
                format!(
                    "{}/finding_candidates.md",
                    BatFolder::AuditorNotes.get_path(false)?
                )
            }
            BatFile::OpenQuestions => {
                format!(
                    "{}/open_questions.md",
                    BatFolder::AuditorNotes.get_path(false)?
                )
            }
            BatFile::ThreatModeling => {
                format!(
                    "{}/threat_modeling.md",
                    BatFolder::AuditorNotes.get_path(false)?
                )
            }
            BatFile::StructsMetadata => {
                format!("{}/structs.md", BatFolder::Metadata.get_path(false)?)
            }
            BatFile::EntrypointsMetadata => {
                format!("{}/entrypoints.md", BatFolder::Metadata.get_path(false)?)
            }
            BatFile::FunctionsMetadata => {
                format!("{}/functions.md", BatFolder::Metadata.get_path(false)?)
            }
            BatFile::TraitMetadata => {
                format!("{}/traits.md", BatFolder::Metadata.get_path(false)?)
            }
            BatFile::TraitImplMetadata => {
                format!("{}/trait_impl.md", BatFolder::Metadata.get_path(false)?)
            }
            BatFile::CodeOverhaulToReview { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulToReview.get_path(false)?
                )
            }
            BatFile::CodeOverhaulStarted { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulStarted.get_path(false)?
                )
            }
            BatFile::CodeOverhaulFinished { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::CodeOverhaulFinished.get_path(false)?
                )
            }
            BatFile::FindingToReview { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsToReview.get_path(false)?
                )
            }
            BatFile::FindingAccepted { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsAccepted.get_path(false)?
                )
            }
            BatFile::FindingRejected { file_name } => {
                let entrypoint_name = file_name.trim_end_matches(".md");
                format!(
                    "{}/{entrypoint_name}.md",
                    BatFolder::FindingsRejected.get_path(false)?
                )
            }
        };

        if canonicalize {
            return canonicalize_path(path);
        }

        Ok(path)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BatFolder {
    Metadata,
    ProgramPath,
    Templates,
    NotesTemplate,
    FindingsToReview,
    FindingsAccepted,
    FindingsRejected,
    CodeOverhaulToReview,
    CodeOverhaulStarted,
    CodeOverhaulFinished,
    AuditorNotes,
    AuditorFigures,
    Notes,
    AuditResult,
    AuditResultFigures,
    AuditResultTemp,
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
            BatFolder::AuditResult => "./audit_result".to_string(),
            BatFolder::AuditResultFigures => "./audit_result/figures".to_string(),
            BatFolder::AuditResultTemp => "./audit_result/temp".to_string(),
            BatFolder::AuditorNotes => auditor_notes_folder_path,
            BatFolder::AuditorFigures => format!("{auditor_notes_folder_path}/figures"),
            BatFolder::Metadata => format!("{auditor_notes_folder_path}/metadata"),
            BatFolder::ProgramPath => bat_config
                .program_lib_path
                .trim_end_matches("/lib.rs")
                .to_string(),
            BatFolder::Templates => {
                format!("./templates")
            }
            BatFolder::NotesTemplate => {
                format!("./templates/notes-folder-template")
            }
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
