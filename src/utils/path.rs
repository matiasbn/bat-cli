use std::{fs, path::Path};

use crate::{commands::miro::MiroConfig, config::BatConfig};

pub enum FileType {
    Metadata,
    ThreatModeling,
    AuditResults,
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

pub fn get_file_path(file_type: FileType, canonicalize: bool) -> String {
    let bat_config = BatConfig::get_validated_config().unwrap();

    let auditor_notes_folder_path = format!("./notes/{}-notes", bat_config.auditor.auditor_name);
    let findings_path = format!("{}/findings", auditor_notes_folder_path);
    let code_overhaul_path = format!("{}/code-overhaul", auditor_notes_folder_path);

    let path = match file_type {
        //File
        FileType::ProgramLib => bat_config.required.program_lib_path,
        FileType::Metadata => {
            format!("{}/metadata.md", auditor_notes_folder_path)
        }
        FileType::ThreatModeling => {
            format!("./threat_modeling.md")
        }
        FileType::AuditResults => {
            format!("./audit_result.md")
        }
        FileType::TemplateFinding => {
            format!("./templates/finding.md")
        }
        FileType::TemplateInformational => {
            format!("./templates/informational.md")
        }
        FileType::TemplateCodeOverhaul => {
            format!("./templates/code-overhaul.md")
        }
        FileType::Readme => {
            format!("./README.md")
        }
        FileType::CodeOverhaulToReview { file_name } => {
            format!("{}/to-review/{file_name}.md", code_overhaul_path)
        }
        FileType::CodeOverhaulStarted { file_name } => {
            if MiroConfig::new().miro_enabled() {
                format!("{}/started/{file_name}/{file_name}.md", code_overhaul_path)
            } else {
                format!("{}/started/{file_name}.md", code_overhaul_path)
            }
        }
        FileType::CodeOverhaulFinished { file_name } => {
            format!("{}/finished/{file_name}.md", code_overhaul_path)
        }
        FileType::FindingToReview { file_name } => {
            format!("{}/to-review/{file_name}.md", findings_path)
        }
        FileType::FindingAccepted { file_name } => {
            format!("{}/accepted/{file_name}.md", findings_path)
        }
        FileType::FindingRejected { file_name } => {
            format!("{}/rejected/{file_name}.md", findings_path)
        }
    };

    if canonicalize {
        canonicalize_path(path);
    }

    path
}

pub enum FolderType {
    ProgramPath,
    Templates,
    FindingsToReview,
    FindingsAccepted,
    FindingsRejected,
    CodeOverhaulToReview,
    CodeOverhaulStarted,
    CodeOverhaulFinished,
}

pub fn get_folder_path(folder_type: FolderType, canonicalize: bool) -> String {
    let bat_config = BatConfig::get_validated_config()?;

    let auditor_notes_folder_path = format!("./notes/{}-notes", bat_config.auditor.auditor_name);
    let findings_path = format!("{}/findings", auditor_notes_folder_path);
    let code_overhaul_path = format!("{}/code-overhaul", auditor_notes_folder_path);

    let path = match folder_type {
        //File
        FolderType::ProgramPath => bat_config.required.program_lib_path.replace("/lib.rs", ""),
        FolderType::Templates => {
            format!("./templates")
        }
        FolderType::FindingsToReview => {
            format!("{}/to-review", findings_path)
        }
        FolderType::FindingsAccepted => {
            format!("{}/accepted", findings_path)
        }
        FolderType::FindingsRejected => {
            format!("{}/rejected", findings_path)
        }
        FolderType::CodeOverhaulToReview => {
            format!("{}/to-review", code_overhaul_path)
        }
        FolderType::CodeOverhaulStarted => {
            format!("{}/started", code_overhaul_path)
        }
        FolderType::CodeOverhaulFinished => {
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
        FileType::CodeOverhaulStarted {
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
