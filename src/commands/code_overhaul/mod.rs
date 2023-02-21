pub mod finish;
pub mod start;
pub mod update;
use super::CommandError;
use crate::batbelt::command_line::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::config::{BatAuditorConfig, BatConfig};
use crate::{batbelt, commands};
use colored::Colorize;
use error_stack::{Result, ResultExt};

pub fn count_co_files() -> Result<(), CommandError> {
    let (to_review_count, started_count, finished_count) = co_counter()?;
    println!("to-review co files: {}", format!("{to_review_count}").red());
    println!("started co files: {}", format!("{started_count}").yellow());
    println!("finished co files: {}", format!("{finished_count}").green());
    println!(
        "total co files: {}",
        format!("{}", to_review_count + started_count + finished_count).purple()
    );
    Ok(())
}

fn co_counter() -> error_stack::Result<(usize, usize, usize), CommandError> {
    let to_review_count = BatFolder::CodeOverhaulToReview
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    let started_count = BatFolder::CodeOverhaulStarted
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    let finished_count = BatFolder::CodeOverhaulFinished
        .get_all_files_names(true, None, None)
        .change_context(CommandError)?
        .len();
    Ok((to_review_count, started_count, finished_count))
}

pub fn open_co() -> Result<(), CommandError> {
    let bat_config = BatConfig::get_config().change_context(CommandError)?;
    let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
    // list to start
    if bat_auditor_config.vs_code_integration {
        let options = vec!["started".green(), "finished".yellow()];
        let prompt_text = format!(
            "Do you want to open a {} or a {} file?",
            options[0], options[1]
        );
        let selection = batbelt::cli_inputs::select(&prompt_text, options.clone(), None)
            .change_context(CommandError)?;
        let open_started = selection == 0;
        let co_folder = if open_started {
            BatFolder::CodeOverhaulStarted
        } else {
            BatFolder::CodeOverhaulFinished
        };
        let co_files = co_folder
            .get_all_files_dir_entries(true, None, None)
            .change_context(CommandError)?
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>();
        if !co_files.is_empty() {
            let prompt_text = "Select the code-overhaul file to open:";
            let selection = batbelt::cli_inputs::select(prompt_text, co_files.clone(), None)
                .change_context(CommandError)?;
            let file_name = &co_files[selection].clone();
            let co_file_path = if open_started {
                batbelt::path::get_file_path(
                    BatFile::CodeOverhaulStarted {
                        file_name: file_name.clone(),
                    },
                    true,
                )
                .change_context(CommandError)?
            } else {
                batbelt::path::get_file_path(
                    BatFile::CodeOverhaulFinished {
                        file_name: file_name.clone(),
                    },
                    true,
                )
                .change_context(CommandError)?
            };
            let instruction_file_path =
                batbelt::path::get_instruction_file_path_from_co_file_path(co_file_path.clone())
                    .change_context(CommandError)?;

            vs_code_open_file_in_current_window(&co_file_path).change_context(CommandError)?;
            vs_code_open_file_in_current_window(&instruction_file_path)
                .change_context(CommandError)?;
        } else {
            println!("Empty {} folder", options[selection].clone());
        }
        vs_code_open_file_in_current_window(&bat_config.program_lib_path)
            .change_context(CommandError)?;
    } else {
        print!("VSCode integration not enabled");
    }
    Ok(())
}

pub fn update_co_templates() -> Result<(), CommandError> {
    let co_to_review_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulToReview, true)
        .change_context(CommandError)?;
    execute_command("rm", &["-rf", &co_to_review_path]).change_context(CommandError)?;
    execute_command("mkdir", &[&co_to_review_path]).change_context(CommandError)?;
    commands::init::initialize_code_overhaul_files().unwrap();
    Ok(())
}

// pub fn update_audit_results() -> Result<(), String> {
//     let audit_file_path = batbelt::path::get_file_path(FilePathType::AuditResult, true);
//     let finished_co_files = get_finished_co_files()?;
//     let finished_co_audit_information = get_finished_co_files_info_for_results(finished_co_files)?;
//     let mut final_result: Vec<String> = vec!["\n# Code overhaul\n".to_string()];
//     let mut table_of_contents: Vec<String> = vec![
//         "# Table of contents\n".to_string(),
//         "- [Table of contents](#table-of-contents)".to_string(),
//         "- [Code overhaul](#code-overhaul)".to_string(),
//     ];
//     for (idx, result) in finished_co_audit_information.iter().enumerate() {
//         // Table of contents
//         let insert_contents = get_table_of_contents_for_results(result.clone(), idx)?;
//         table_of_contents.push(insert_contents);
//
//         // Result
//         let title = format!("## {}\n\n", result.file_name);
//         let what_it_does_text = format!(
//             "### What it does:\n\n{}\n\n",
//             result.what_it_does_content.trim()
//         );
//         let notes_text = format!("### Notes:\n\n{}\n\n", result.notes_content.trim());
//         let miro_frame_text = format!("### Miro frame url:\n\n{}\n", result.miro_frame_url.trim());
//         final_result.push([title, what_it_does_text, notes_text, miro_frame_text].join(""));
//     }
//     table_of_contents.append(&mut final_result);
//     fs::write(
//         audit_file_path,
//         table_of_contents
//             .join("\n")
//             .replace(CODE_OVERHAUL_NOTES_PLACEHOLDER, "No notes")
//             .as_str(),
//     )
//     .unwrap();
//     create_git_commit(GitCommit::AuditResult, None)?;
//     Ok(())
// }
