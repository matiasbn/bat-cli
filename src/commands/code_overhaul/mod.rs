pub mod finish;
pub mod start;
pub mod update;

use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::batbelt::command_line::vs_code_open_file_in_current_window;

use crate::batbelt::constants::{
    CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER, CO_FIGURES,
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;
use crate::config::BatConfig;

use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::batbelt::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results,
    get_table_of_contents_for_results,
};
use crate::batbelt::path::{FilePathType, FolderPathType};
use crate::{batbelt, commands};

use std::{env, fs};

use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::bash::execute_command;
use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::sonar::{get_function_parameters, BatSonar, SonarResult, SonarResultType};
use clap::builder::Str;
use std::path::Path;
use std::process::Command;
use std::string::String;

pub fn count_co_files() -> Result<(), String> {
    let (to_review_count, started_count, finished_count) = batbelt::helpers::count::co_counter()?;
    println!("to-review co files: {}", format!("{to_review_count}").red());
    println!("started co files: {}", format!("{started_count}").yellow());
    println!("finished co files: {}", format!("{finished_count}").green());
    println!(
        "total co files: {}",
        format!("{}", to_review_count + started_count + finished_count).purple()
    );
    Ok(())
}

pub fn open_co() -> Result<(), String> {
    let BatConfig {
        auditor, required, ..
    } = BatConfig::get_validated_config()?;
    // list to start
    if auditor.vs_code_integration {
        let options = vec!["started".green(), "finished".yellow()];
        let prompt_text = format!(
            "Do you want to open a {} or a {} file?",
            options[0], options[1]
        );
        let selection = batbelt::cli_inputs::select(&prompt_text, options.clone(), None)?;
        let open_started = selection == 0;
        let folder_path = if open_started {
            batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true)
        } else {
            batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true)
        };
        let co_files = batbelt::helpers::get::get_only_files_from_folder(folder_path)?;
        let co_files = co_files
            .iter()
            .filter(|f| f.name != ".gitkeep")
            .map(|f| f.name.clone())
            .collect::<Vec<_>>();
        if !co_files.is_empty() {
            let prompt_text = "Select the code-overhaul file to open:";
            let selection = batbelt::cli_inputs::select(prompt_text, co_files.clone(), None)?;
            let file_name = &co_files[selection].clone();
            let co_file_path = if open_started {
                batbelt::path::get_file_path(
                    FilePathType::CodeOverhaulStarted {
                        file_name: file_name.clone(),
                    },
                    true,
                )
            } else {
                batbelt::path::get_file_path(
                    FilePathType::CodeOverhaulFinished {
                        file_name: file_name.clone(),
                    },
                    true,
                )
            };
            let instruction_file_path =
                batbelt::path::get_instruction_file_path_from_co_file_path(co_file_path.clone())?;

            vs_code_open_file_in_current_window(&co_file_path)?;
            vs_code_open_file_in_current_window(&instruction_file_path)?;
        } else {
            println!("Empty {} folder", options[selection].clone());
        }
        vs_code_open_file_in_current_window(&required.program_lib_path)?;
    } else {
        print!("VSCode integration not enabled");
    }
    Ok(())
}

pub fn update_co_templates() -> Result<(), String> {
    let co_to_review_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, true);
    execute_command("rm", &["-rf", &co_to_review_path]).unwrap();
    execute_command("mkdir", &[&co_to_review_path]).unwrap();
    commands::init::initialize_code_overhaul_files().unwrap();
    Ok(())
}

pub fn update_audit_results() -> Result<(), String> {
    let audit_file_path = batbelt::path::get_file_path(FilePathType::AuditResult, true);
    let finished_co_files = get_finished_co_files()?;
    let finished_co_audit_information = get_finished_co_files_info_for_results(finished_co_files)?;
    let mut final_result: Vec<String> = vec!["\n# Code overhaul\n".to_string()];
    let mut table_of_contents: Vec<String> = vec![
        "# Table of contents\n".to_string(),
        "- [Table of contents](#table-of-contents)".to_string(),
        "- [Code overhaul](#code-overhaul)".to_string(),
    ];
    for (idx, result) in finished_co_audit_information.iter().enumerate() {
        // Table of contents
        let insert_contents = get_table_of_contents_for_results(result.clone(), idx)?;
        table_of_contents.push(insert_contents);

        // Result
        let title = format!("## {}\n\n", result.file_name);
        let what_it_does_text = format!(
            "### What it does:\n\n{}\n\n",
            result.what_it_does_content.trim()
        );
        let notes_text = format!("### Notes:\n\n{}\n\n", result.notes_content.trim());
        let miro_frame_text = format!("### Miro frame url:\n\n{}\n", result.miro_frame_url.trim());
        final_result.push([title, what_it_does_text, notes_text, miro_frame_text].join(""));
    }
    table_of_contents.append(&mut final_result);
    fs::write(
        audit_file_path,
        table_of_contents
            .join("\n")
            .replace(CODE_OVERHAUL_NOTES_PLACEHOLDER, "No notes")
            .as_str(),
    )
    .unwrap();
    create_git_commit(GitCommit::AuditResult, None)?;
    Ok(())
}
