use std::{fs, path::Path};

use crate::{commands::miro::MiroConfig, config::BatConfig};

pub enum FilePathType {
    Metadata,
    ThreatModeling,
    AuditResults,
    FindingCandidates,
    OpenQuestions,
    ProgramLib,
    Readme,
    TemplateFinding,
    TemplateInformational,
    TemplateCodeOverhaul,
    CodeOverhaulToReview { file_name: String },
    CodeOverhaulStarted { file_name: String },
    CodeOverhaulFinished { file_name: String },
    FindingToReview { file_name: String },
    FindingAccepted { file_name: String },
    FindingRejected { file_name: String },
}

pub fn get_file_path(file_type: FilePathType, canonicalize: bool) -> String {
    let bat_config = BatConfig::get_validated_config().unwrap();

    let auditor_notes_folder_path = format!("./notes/{}-notes", bat_config.auditor.auditor_name);
    let findings_path = format!("{}/findings", auditor_notes_folder_path);
    let code_overhaul_path = format!("{}/code-overhaul", auditor_notes_folder_path);

    let path = match file_type {
        //File
        FilePathType::ProgramLib => bat_config.required.program_lib_path,
        FilePathType::FindingCandidates => {
            format!("{}/finding_candidates.md", auditor_notes_folder_path)
        }
        FilePathType::OpenQuestions => format!("{}/open_questions.md", auditor_notes_folder_path),
        FilePathType::Metadata => {
            format!("{}/metadata.md", auditor_notes_folder_path)
        }
        FilePathType::ThreatModeling => {
            format!("{}/threat_modeling.md", auditor_notes_folder_path)
        }
        FilePathType::AuditResults => {
            format!("./audit_result.md")
        }
        FilePathType::TemplateFinding => {
            format!("./templates/finding.md")
        }
        FilePathType::TemplateInformational => {
            format!("./templates/informational.md")
        }
        FilePathType::TemplateCodeOverhaul => {
            format!("./templates/code-overhaul.md")
        }
        FilePathType::Readme => {
            format!("./README.md")
        }
        FilePathType::CodeOverhaulToReview { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            format!("{code_overhaul_path}/to-review/{entrypoint_name}.md")
        }
        FilePathType::CodeOverhaulStarted { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            if MiroConfig::new().miro_enabled() {
                format!("{code_overhaul_path}/started/{entrypoint_name}/{entrypoint_name}.md")
            } else {
                format!("{code_overhaul_path}/started/{entrypoint_name}.md")
            }
        }
        FilePathType::CodeOverhaulFinished { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            format!("{code_overhaul_path}/finished/{entrypoint_name}.md")
        }
        FilePathType::FindingToReview { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            format!("{findings_path}/to-review/{entrypoint_name}.md",)
        }
        FilePathType::FindingAccepted { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            format!("{findings_path}/accepted/{entrypoint_name}.md",)
        }
        FilePathType::FindingRejected { file_name } => {
            let entrypoint_name = file_name.replace(".md", "");
            format!("{findings_path}/rejected/{entrypoint_name}.md",)
        }
    };

    if canonicalize {
        return canonicalize_path(path);
    }

    path
}

pub enum FolderPathType {
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
}

pub fn get_folder_path(folder_type: FolderPathType, canonicalize: bool) -> String {
    let bat_config = BatConfig::get_validated_config().unwrap();

    let auditor_notes_folder_path = format!("./notes/{}-notes", bat_config.auditor.auditor_name);
    let findings_path = format!("{}/findings", auditor_notes_folder_path);
    let code_overhaul_path = format!("{}/code-overhaul", auditor_notes_folder_path);

    let path = match folder_type {
        //File
        FolderPathType::Notes => "./notes".to_string(),
        FolderPathType::AuditorNotes => auditor_notes_folder_path,
        FolderPathType::AuditorFigures => format!("{auditor_notes_folder_path}/figures"),
        FolderPathType::ProgramPath => bat_config.required.program_lib_path.replace("/lib.rs", ""),
        FolderPathType::Templates => {
            format!("./templates")
        }
        FolderPathType::NotesTemplate => {
            format!("./templates/notes-folder-template")
        }
        FolderPathType::FindingsToReview => {
            format!("{}/to-review", findings_path)
        }
        FolderPathType::FindingsAccepted => {
            format!("{}/accepted", findings_path)
        }
        FolderPathType::FindingsRejected => {
            format!("{}/rejected", findings_path)
        }
        FolderPathType::CodeOverhaulToReview => {
            format!("{}/to-review", code_overhaul_path)
        }
        FolderPathType::CodeOverhaulStarted => {
            format!("{}/started", code_overhaul_path)
        }
        FolderPathType::CodeOverhaulFinished => {
            format!("{}/finished", code_overhaul_path)
        }
    };

    if canonicalize {
        return canonicalize_path(path);
    }

    path
}

fn canonicalize_path(path_to_canonicalize: String) -> String {
    let error_message = format!("Error canonicalizing path: {}", path_to_canonicalize);
    let canonicalized_path = Path::new(&(path_to_canonicalize))
        .canonicalize()
        .expect(&error_message)
        .into_os_string()
        .into_string()
        .expect(&error_message);
    canonicalized_path
}

pub fn get_instruction_file_path_from_started_entrypoint_co_file(
    entrypoint_name: String,
) -> Result<String, String> {
    let co_file_path = get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: entrypoint_name.clone(),
        },
        false,
    );
    let program_path = BatConfig::get_validated_config()?
        .required
        .program_lib_path
        .replace("/lib.rs", "")
        .replace("../", "");
    let started_file_string = fs::read_to_string(co_file_path.clone()).unwrap();
    let instruction_file_path = started_file_string
        .lines()
        .into_iter()
        .find(|f| f.contains(&program_path))
        .expect(&format!(
            "co file of {} does not contain the instruction path yet",
            entrypoint_name,
        ))
        .to_string();
    Ok(instruction_file_path)
}
