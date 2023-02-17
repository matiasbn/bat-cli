use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use error_stack::{IntoReport, ResultExt};
use normalize_url::normalizer;

use walkdir::WalkDir;

use crate::batbelt::constants::{
    CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER,
};

use crate::batbelt;
use crate::errors::{BatError, BatErrorType};
use error_stack::Result;
use std::fs;
use std::fs::ReadDir;
use std::string::String;

pub mod get {
    use std::fs::DirEntry;

    use crate::batbelt::path::FolderPathType;
    use crate::batbelt::structs::FileInfo;
    use crate::batbelt::{self};

    use super::*;

    pub fn get_all_rust_files_from_program_path() -> Result<Vec<FileInfo>, BatError> {
        let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false)
            .change_context(BatError)?;
        let mut lib_files_info = get_only_files_from_folder(program_path)
            .unwrap()
            .into_iter()
            .filter(|file_info| file_info.name != "mod.rs" && file_info.name.contains(".rs"))
            .collect::<Vec<FileInfo>>();
        lib_files_info.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(lib_files_info)
    }

    // returns a list of folder and files names
    pub fn get_started_entrypoints() -> Result<Vec<String>, BatError> {
        // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
        let started_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulStarted,
            true,
        )
        .change_context(BatError)?;
        let started_files = fs::read_dir(started_path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_str().unwrap().to_string())
            .filter(|file| file != ".gitkeep")
            .collect::<Vec<String>>();
        if started_files.is_empty() {
            panic!("no started files in code-overhaul folder");
        }
        Ok(started_files)
    }

    pub fn get_finished_co_files() -> Result<Vec<(String, String)>, BatError> {
        // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
        let finished_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulFinished,
            true,
        )
        .change_context(BatError)?;
        let mut finished_folder = fs::read_dir(&finished_path)
            .unwrap()
            .map(|file| file.unwrap())
            .collect::<Vec<DirEntry>>();
        finished_folder.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
        let mut finished_files_content: Vec<(String, String)> = vec![];

        for co_file in finished_folder {
            let file_content = fs::read_to_string(co_file.path()).unwrap();
            let file_name = co_file.file_name();
            if file_name != ".gitkeep" {
                finished_files_content.push((
                    co_file.file_name().to_str().unwrap().to_string(),
                    file_content,
                ));
            }
        }
        Ok(finished_files_content)
    }

    // #[derive(Debug, Clone)]
    // pub struct FinishedCoFileContent {
    //     pub file_name: String,
    //     pub what_it_does_content: String,
    //     pub notes_content: String,
    //     pub miro_frame_url: String,
    // }
    // pub fn get_finished_co_files_info_for_results(
    //     finished_co_files_content: Vec<(String, String)>,
    // ) -> Result<Vec<FinishedCoFileContent>, String> {
    //     let mut finished_content: Vec<FinishedCoFileContent> = vec![];
    //     // get necessary information from co files
    //     for (file_name, file_content) in finished_co_files_content {
    //         let what_it_does_index = file_content
    //             .lines()
    //             .position(|line| line.contains("# What it does?"))
    //             .unwrap()
    //             + 1;
    //         let notes_index = file_content
    //             .lines()
    //             .position(|line| line.contains("# Notes"))
    //             .unwrap()
    //             + 1;
    //         let signers_index = file_content
    //             .lines()
    //             .position(|line| line.contains("# Signers"))
    //             .unwrap();
    //         let content_vec: Vec<String> =
    //             file_content.lines().map(|line| line.to_string()).collect();
    //         let what_it_does_content: Vec<String> = content_vec.clone()
    //             [what_it_does_index..notes_index - 1]
    //             .to_vec()
    //             .iter()
    //             .filter(|line| !line.is_empty())
    //             .map(|line| line.to_string())
    //             .collect();
    //         let notes_content: Vec<String> = content_vec.clone()[notes_index..signers_index - 1]
    //             .to_vec()
    //             .iter()
    //             .filter(|line| !line.is_empty())
    //             .map(|line| line.to_string())
    //             .collect();
    //         let miro_frame_url = content_vec
    //             .iter()
    //             .find(|line| line.contains("https://miro.com/app/board"))
    //             .unwrap()
    //             .split(": ")
    //             .last()
    //             .unwrap();
    //         finished_content.push(FinishedCoFileContent {
    //             file_name: file_name.replace(".md", ""),
    //             what_it_does_content: what_it_does_content.join("\n"),
    //             notes_content: notes_content.join("\n"),
    //             miro_frame_url: miro_frame_url.to_string(),
    //         });
    //     }
    //     Ok(finished_content)
    // }
    // pub fn get_table_of_contents_for_results(
    //     result: FinishedCoFileContent,
    //     result_idx: usize,
    // ) -> Result<String, String> {
    //     let result_id = if result_idx == 0 {
    //         "".to_string()
    //     } else {
    //         format!("-{result_idx}")
    //     };
    //     let toc_title = format!(
    //         "  - [{}](#{})",
    //         result.file_name.replace("_", "\\_"),
    //         result.file_name
    //     );
    //     let toc_wid = format!("    - [What it does:](#what-it-does{})", result_id);
    //     let toc_notes = format!("    - [Notes:](#notes{})", result_id);
    //     let toc_miro = format!("    - [Miro frame url:](#miro-frame-url{})", result_id);
    //
    //     let insert_contents = vec![toc_title, toc_wid, toc_notes, toc_miro].join("\n");
    //     Ok(insert_contents)
    // }

    pub fn get_only_files_from_folder(folder_path: String) -> Result<Vec<FileInfo>, BatError> {
        let state_folder_files_info = WalkDir::new(folder_path)
            .into_iter()
            .filter(|f| {
                f.as_ref().unwrap().metadata().unwrap().is_file()
                    && f.as_ref().unwrap().file_name() != ".gitkeep"
            })
            .map(|entry| {
                let path = entry.as_ref().unwrap().path().display().to_string();
                let name = entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_os_string()
                    .into_string()
                    .unwrap();
                let info = FileInfo::new(path, name);
                info
            })
            .collect::<Vec<FileInfo>>();
        Ok(state_folder_files_info)
    }
}

pub mod check {
    use super::*;
    use crate::batbelt::constants::CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER;
    pub fn code_overhaul_file_completed(file_path: String, file_name: String) {
        let file_data = fs::read_to_string(file_path).unwrap();
        if file_data.contains(CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER) {
            panic!("Please complete the \"What it does?\" section of the {file_name} file");
        }

        if file_data.contains(CODE_OVERHAUL_NOTES_PLACEHOLDER) {
            let options = vec!["yes", "no"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Notes section not completed, do you want to proceed anyway?")
                .items(&options)
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            if options[selection] == "no" {
                panic!("Aborted by the user");
            }
        }

        if file_data.contains(CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER) {
            panic!("Please complete the \"Signers\" section of the {file_name} file");
        }

        if file_data.contains(CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER) {
            let options = vec!["yes", "no"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Validations section not completed, do you want to proceed anyway?")
                .items(&options)
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            if options[selection] == "no" {
                panic!("Aborted by the user");
            }
        }

        if file_data.contains(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER) {
            panic!("Please complete the \"Miro board frame\" section of the {file_name} file");
        }
    }
}

pub mod count {
    use crate::errors::BatError;

    use super::*;
    pub fn count_filtering_gitkeep(dir_to_count: ReadDir) -> usize {
        dir_to_count
            .filter(|file| {
                !file
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .contains(".gitkeep")
            })
            .collect::<Vec<_>>()
            .len()
    }
    pub fn co_counter() -> error_stack::Result<(usize, usize, usize), BatError> {
        // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
        let to_review_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulToReview,
            true,
        )
        .change_context(BatError)?;
        let to_review_folder = fs_read_dir(&to_review_path).change_context(BatError)?;
        let to_review_count = count_filtering_gitkeep(to_review_folder);
        let started_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulStarted,
            true,
        )
        .change_context(BatError)?;
        let started_folder = fs_read_dir(&started_path)?;
        let started_count = count_filtering_gitkeep(started_folder);
        let finished_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulFinished,
            true,
        )
        .change_context(BatError)?;
        let finished_folder = fs_read_dir(&finished_path)?;
        let finished_count = count_filtering_gitkeep(finished_folder);
        Ok((to_review_count, started_count, finished_count))
    }
}

// pub fn read_to_string(path: &str) -> error_stack::Result<String, BatError> {}
pub fn fs_read_dir(path: &str) -> error_stack::Result<ReadDir, BatError> {
    let dir = fs::read_dir(path)
        .into_report()
        .change_context(BatError)
        .attach_printable_lazy(|| format!("Error reading dir: \n path: {} ", path))?;
    // let dir = fs::read_dir(path).map_err(|_| BatErrorType::ReadDir { path }.parse_error())?;
    Ok(dir)
}

// pub fn fs_write(path: &str, content: &str) -> error_stack::Result<(), BatError> {
//     fs::write(path, content).map_err(|_| BatErrorType::Write { path }.parse_error())?;
//     Ok(())
// }

pub fn normalize_url(url_to_normalize: &str) -> Result<String, String> {
    let url = normalizer::UrlNormalizer::new(url_to_normalize)
        .expect(format!("Bad formated url {}", url_to_normalize).as_str())
        .normalize(None)
        .expect(format!("Error normalizing url {}", url_to_normalize).as_str());
    Ok(url)
}
