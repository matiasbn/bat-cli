use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use normalize_url::normalizer;

use walkdir::WalkDir;

use crate::batbelt::constants::{
    CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER,
    CONTEXT_ACCOUNTS_PNG_NAME, ENTRYPOINT_PNG_NAME, HANDLER_PNG_NAME, VALIDATIONS_PNG_NAME,
};
use crate::config::{BatConfig, RequiredConfig};

use std::borrow::Borrow;

use crate::batbelt;
use std::fs;
use std::fs::ReadDir;
use std::io::BufRead;
use std::path::Path;
use std::string::String;

pub mod get {
    use std::fs::DirEntry;

    use crate::batbelt::path::FolderPathType;
    use crate::batbelt::structs::FileInfo;
    use crate::batbelt::{self};

    use super::*;

    pub fn get_screenshot_id(file_name: &str, started_co_file_path: &String) -> String {
        let screenshot_contains = match file_name {
            ENTRYPOINT_PNG_NAME => "- entrypoint",
            CONTEXT_ACCOUNTS_PNG_NAME => "- context",
            VALIDATIONS_PNG_NAME => "- validations",
            HANDLER_PNG_NAME => "- handler",
            _ => todo!(),
        };
        let file_content = fs::read_to_string(started_co_file_path).unwrap();
        let item_id = file_content
            .lines()
            .find(|line| line.contains(screenshot_contains))
            .unwrap()
            .split("id: ")
            .last()
            .unwrap();
        item_id.to_string()
    }

    pub fn get_context_name(co_file_name: String) -> Result<String, String> {
        let BatConfig { required, .. } = BatConfig::get_validated_config()?;
        let RequiredConfig {
            program_lib_path, ..
        } = required;

        let lib_file = fs::read_to_string(program_lib_path).unwrap();
        let lib_file_lines: Vec<&str> = lib_file.lines().collect();

        let entrypoint_index = lib_file
            .lines()
            .position(|line| {
                if line.contains("pub fn") {
                    let function_name = line
                        .split('(')
                        .next()
                        .unwrap()
                        .split('<')
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .last()
                        .unwrap();
                    function_name == co_file_name.replace(".md", "")
                } else {
                    false
                }
            })
            .unwrap();
        let canditate_lines = vec![
            lib_file_lines[entrypoint_index],
            lib_file_lines[entrypoint_index + 1],
        ];

        // if is not in the same line as the entrypoint name, is in the next line
        let context_line = if canditate_lines[0].contains("Context<") {
            canditate_lines[0]
        } else {
            canditate_lines[1]
        };

        // replace all the extra strings to get the Context name
        let parsed_context_name = context_line
            .replace("'_, ", "")
            .replace("'info, ", "")
            .replace("<'info>", "")
            .split("Context<")
            .map(|l| l.to_string())
            .collect::<Vec<String>>()[1]
            .split('>')
            .map(|l| l.to_string())
            .collect::<Vec<String>>()[0]
            .clone();
        Ok(parsed_context_name)
    }

    pub fn get_instruction_files() -> Result<Vec<FileInfo>, String> {
        let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
        let mut lib_files_info = get_only_files_from_folder(program_path)
            .unwrap()
            .into_iter()
            .filter(|file_info| file_info.name != "mod.rs" && file_info.name.contains(".rs"))
            .collect::<Vec<FileInfo>>();
        lib_files_info.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(lib_files_info)
    }

    // returns a list of folder and files names
    pub fn get_started_entrypoints() -> Result<Vec<String>, String> {
        // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
        let started_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulStarted,
            true,
        );
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

    pub fn get_instruction_file_with_prompts(
        to_start_file_name: &String,
    ) -> Result<String, String> {
        let instruction_files_info = get_instruction_files()?;

        let entrypoint_name = to_start_file_name.replace(".md", "");
        let instruction_match = instruction_files_info
            .iter()
            .filter(|ifile| ifile.name.replace(".rs", "") == entrypoint_name.as_str())
            .collect::<Vec<&FileInfo>>();

        // if instruction exists, prompt the user if the file is correct
        let is_match = if instruction_match.len() == 1 {
            let instruction_match_path = Path::new(&instruction_match[0].path);
            let prompt_text = format!(
                "{}  <--- is this the correct instruction file?:",
                instruction_match_path.to_str().unwrap()
            );
            let correct_path = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
            correct_path
        } else {
            false
        };

        let instruction_file_path = if is_match {
            &instruction_match[0].path
        } else {
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select the instruction file: ")
                .items(
                    &instruction_files_info
                        .as_slice()
                        .iter()
                        .map(|f| &f.name)
                        .collect::<Vec<&String>>(),
                )
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            let name = instruction_files_info.as_slice()[selection].path.borrow();
            name
        };
        Ok(instruction_file_path.clone())
    }

    pub fn get_finished_co_files() -> Result<Vec<(String, String)>, String> {
        // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
        let finished_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulFinished,
            true,
        );
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
    #[derive(Debug, Clone)]
    pub struct FinishedCoFileContent {
        pub file_name: String,
        pub what_it_does_content: String,
        pub notes_content: String,
        pub miro_frame_url: String,
    }
    pub fn get_finished_co_files_info_for_results(
        finished_co_files_content: Vec<(String, String)>,
    ) -> Result<Vec<FinishedCoFileContent>, String> {
        let mut finished_content: Vec<FinishedCoFileContent> = vec![];
        // get necessary information from co files
        for (file_name, file_content) in finished_co_files_content {
            let what_it_does_index = file_content
                .lines()
                .position(|line| line.contains("# What it does?"))
                .unwrap()
                + 1;
            let notes_index = file_content
                .lines()
                .position(|line| line.contains("# Notes"))
                .unwrap()
                + 1;
            let signers_index = file_content
                .lines()
                .position(|line| line.contains("# Signers"))
                .unwrap();
            let content_vec: Vec<String> =
                file_content.lines().map(|line| line.to_string()).collect();
            let what_it_does_content: Vec<String> = content_vec.clone()
                [what_it_does_index..notes_index - 1]
                .to_vec()
                .iter()
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect();
            let notes_content: Vec<String> = content_vec.clone()[notes_index..signers_index - 1]
                .to_vec()
                .iter()
                .filter(|line| !line.is_empty())
                .map(|line| line.to_string())
                .collect();
            let miro_frame_url = content_vec
                .iter()
                .find(|line| line.contains("https://miro.com/app/board"))
                .unwrap()
                .split(": ")
                .last()
                .unwrap();
            finished_content.push(FinishedCoFileContent {
                file_name: file_name.replace(".md", ""),
                what_it_does_content: what_it_does_content.join("\n"),
                notes_content: notes_content.join("\n"),
                miro_frame_url: miro_frame_url.to_string(),
            });
        }
        Ok(finished_content)
    }
    pub fn get_table_of_contents_for_results(
        result: FinishedCoFileContent,
        result_idx: usize,
    ) -> Result<String, String> {
        let result_id = if result_idx == 0 {
            "".to_string()
        } else {
            format!("-{result_idx}")
        };
        let toc_title = format!(
            "  - [{}](#{})",
            result.file_name.replace("_", "\\_"),
            result.file_name
        );
        let toc_wid = format!("    - [What it does:](#what-it-does{})", result_id);
        let toc_notes = format!("    - [Notes:](#notes{})", result_id);
        let toc_miro = format!("    - [Miro frame url:](#miro-frame-url{})", result_id);

        let insert_contents = vec![toc_title, toc_wid, toc_notes, toc_miro].join("\n");
        Ok(insert_contents)
    }
    pub fn get_only_files_from_folder(folder_path: String) -> Result<Vec<FileInfo>, String> {
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
    pub fn get_structs_in_files(state_file_infos: Vec<FileInfo>) -> Result<Vec<String>, String> {
        let mut structs_in_state_files: Vec<String> = vec![];
        for file in state_file_infos {
            let file_string = fs::read_to_string(file.path.clone())
                .expect(&format!("Error reading the {} file", file.path.clone()));
            let mut last_read_line = 0;
            for (file_line_index, _) in file_string.lines().into_iter().enumerate() {
                if last_read_line < file_line_index {
                    continue;
                }
                let start_index = file_string.lines().into_iter().enumerate().position(|l| {
                    l.1.contains("struct") && l.1.contains("{") && l.0 > last_read_line
                });
                let start_struct_index = if let Some(start_index) = start_index {
                    start_index
                } else {
                    continue;
                };
                let final_struct_index = file_string
                    .lines()
                    .into_iter()
                    .enumerate()
                    .position(|l| l.1.trim() == "}" && l.0 > start_struct_index)
                    .expect(&format!(
                        "Error looking for opening line of struct in {} file",
                        file.path.clone()
                    ));
                let struct_lines = file_string.clone().lines().collect::<Vec<_>>()
                    [start_struct_index..=final_struct_index]
                    .to_vec()
                    .join("\n");
                structs_in_state_files.push(struct_lines.clone());
                last_read_line = final_struct_index;
            }
        }
        Ok(structs_in_state_files)
    }
    pub fn get_string_between_two_str_from_string(
        content: String,
        str_start: &str,
        str_end: &str,
    ) -> Result<String, String> {
        let start_index = content
            .lines()
            .into_iter()
            .position(|f| f.contains(str_start))
            .unwrap();
        let end_index = content
            .lines()
            .into_iter()
            .position(|f| f.contains(str_end))
            .unwrap();
        let context_account_lines = content.lines().collect::<Vec<_>>()[start_index..end_index]
            .to_vec()
            .join("\n");
        Ok(context_account_lines)
    }
    pub fn get_string_between_two_str_from_path(
        file_path: String,
        str_start: &str,
        str_end: &str,
    ) -> Result<String, String> {
        let content_string = fs::read_to_string(file_path.clone())
            .expect(format!("Error reading: {}", file_path).as_str());
        let content_lines = content_string.lines();
        let start_index = content_lines
            .clone()
            .into_iter()
            .position(|f| f.contains(str_start))
            .unwrap();
        let end_index = content_lines
            .clone()
            .into_iter()
            .position(|f| f.contains(str_end))
            .unwrap();
        let context_account_lines = content_lines.clone().collect::<Vec<_>>()
            [start_index..end_index]
            .to_vec()
            .join("\n");
        Ok(context_account_lines)
    }
    pub fn get_string_between_two_index_from_string(
        content: String,
        start_index: usize,
        end_index: usize,
    ) -> Result<String, String> {
        let content_result = content.lines().collect::<Vec<_>>()[start_index..=end_index]
            .to_vec()
            .join("\n");
        Ok(content_result)
    }

    /// Returns (instruction handler string, the instruction path,  the starting index and the end index)
    pub fn get_instruction_handler_of_entrypoint(
        entrypoint_name: String,
    ) -> Result<(String, String, usize, usize), String> {
        let mut handler_string: String = "".to_string();
        let instruction_file_path =
            batbelt::path::get_instruction_file_path_from_started_co_file(entrypoint_name.clone())?;
        let instruction_file_string =
            fs::read_to_string(format!("../{}", instruction_file_path)).unwrap();
        let context_name = get_context_name(entrypoint_name.clone())?;
        let mut start_index = 0;
        let mut end_index = 0;
        for (line_index, line) in instruction_file_string.lines().enumerate() {
            if line.contains("pub") && line.contains("fn") {
                let closing_index = instruction_file_string
                    .clone()
                    .lines()
                    .into_iter()
                    .enumerate()
                    .position(|(l_index, l)| l == "}" && l_index > line_index)
                    .unwrap();
                let handler_string_candidate = get_string_between_two_index_from_string(
                    instruction_file_string.clone(),
                    line_index,
                    closing_index,
                )?;
                if handler_string_candidate
                    .lines()
                    .into_iter()
                    .any(|l| l.contains("Context") && l.contains(&context_name))
                {
                    handler_string = handler_string_candidate;
                    start_index = line_index;
                    end_index = closing_index;
                }
            }
        }
        Ok((
            handler_string,
            instruction_file_path,
            start_index,
            end_index,
        ))
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
    pub fn co_counter() -> Result<(usize, usize, usize), String> {
        // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
        let to_review_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulToReview,
            true,
        );
        let to_review_folder = fs::read_dir(to_review_path).unwrap();
        let to_review_count = count_filtering_gitkeep(to_review_folder);
        let started_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulStarted,
            true,
        );
        let started_folder = fs::read_dir(started_path).unwrap();
        let started_count = count_filtering_gitkeep(started_folder);
        let finished_path = batbelt::path::get_folder_path(
            batbelt::path::FolderPathType::CodeOverhaulFinished,
            true,
        );
        let finished_folder = fs::read_dir(finished_path).unwrap();
        let finished_count = count_filtering_gitkeep(finished_folder);
        Ok((to_review_count, started_count, finished_count))
    }
}

pub mod format {

    pub fn format_to_rust_comment(comment: &str) -> String {
        let mut formmated_comment_lines: Vec<String> = vec![];
        for (comment_line_index, comment_line) in comment.lines().enumerate() {
            let trimmed = comment_line.trim();
            if comment_line_index == 0
                || comment_line_index == comment.lines().collect::<Vec<_>>().len() - 1
            {
                formmated_comment_lines.push(format!("  {}", trimmed))
            } else {
                formmated_comment_lines.push(format!("    {}", trimmed))
            }
        }
        format!("- ```rust\n{}\n  ```", formmated_comment_lines.join("\n"))
    }
}

pub mod print {
    use std::fmt::Display;

    pub fn print_string_vector<T: Display>(to_print: Vec<T>, comment: &str) {
        for text in to_print {
            println!("{}:\n {}\n", comment, text);
        }
    }

    pub fn print_string<T: Display>(to_print: T, comment: T) {
        println!("{}:\n {}\n", comment, to_print);
    }
}

pub fn normalize_url(url_to_normalize: &str) -> Result<String, String> {
    let url = normalizer::UrlNormalizer::new(url_to_normalize)
        .expect(format!("Bad formated url {}", url_to_normalize).as_str())
        .normalize(None)
        .expect(format!("Error normalizing url {}", url_to_normalize).as_str());
    Ok(url)
}
