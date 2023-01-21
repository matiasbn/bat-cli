use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::command_line::{canonicalize_path, vs_code_open_file_in_current_window};

use crate::commands::miro::{self, MiroConfig};
use crate::config::BatConfig;
use crate::constants::{
    AUDIT_RESULT_FILE_NAME, CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER,
    CODE_OVERHAUL_NOTES_PLACEHOLDER, CO_FIGURES,
};

use crate::utils::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::utils::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results,
    get_table_of_contents_for_results,
};
use crate::utils::*;

use std::fs;

use std::path::Path;
use std::process::Command;
use std::string::String;

pub fn create_overhaul_file(entrypoint_name: String) -> Result<(), String> {
    let code_overhaul_auditor_file_path =
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(entrypoint_name.clone()))?;
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        panic!("code overhaul file already exists for: {entrypoint_name:?}");
    }
    let output = Command::new("cp")
        .args([
            "-r",
            BatConfig::get_code_overhaul_template_path()?.as_str(),
            code_overhaul_auditor_file_path.as_str(),
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "create auditors note folder failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    println!("code-overhaul file created: {entrypoint_name}.md");
    Ok(())
}

pub async fn start_code_overhaul_file() -> Result<(), String> {
    check_correct_branch()?;

    // check if program_lib_path is not empty or panic
    let BatConfig { optional, .. } = BatConfig::get_validated_config()?;

    if optional.program_instructions_path.is_empty() {
        panic!("Optional program_instructions_path parameter not set in Bat.toml")
    }

    if !Path::new(&optional.program_instructions_path).is_dir() {
        panic!("program_instructions_path is not a correct folder")
    }

    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None)?;
    // get to-review files
    let mut review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    review_files.sort();

    if review_files.is_empty() {
        panic!("no to-review files in code-overhaul folder");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select the code-overhaul file to start:")
        .items(&review_files)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    let to_start_file_name = match selection {
        // move selected file to rejected
        Some(index) => review_files[index].clone(),
        None => panic!("User did not select anything"),
    };

    let to_review_file_path =
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(to_start_file_name.clone()))?;

    let (entrypoint_name, instruction_file_path) =
        helpers::get::get_instruction_file_with_prompts(&to_start_file_name)?;
    let to_review_file_string = fs::read_to_string(to_review_file_path.clone()).unwrap();
    fs::write(
        to_review_file_path.clone(),
        to_review_file_string
            .replace(
                CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER,
                &instruction_file_path.replace("../", ""),
            )
            .as_str(),
    )
    .unwrap();
    let instruction_file_path = Path::new(&instruction_file_path).canonicalize().unwrap();
    let context_lines =
        helpers::get::get_context_lines(instruction_file_path.clone(), to_start_file_name.clone())?;

    // open instruction file in VSCode
    vs_code_open_file_in_current_window(instruction_file_path.to_str().unwrap())?;

    // parse text into co file
    helpers::parse::parse_validations_into_co(
        to_review_file_path.clone(),
        context_lines.clone(),
        instruction_file_path.to_str().unwrap().to_string(),
    );
    helpers::parse::parse_context_accounts_into_co(
        Path::new(&(to_review_file_path.clone()))
            .canonicalize()
            .unwrap(),
        context_lines.clone(),
    );

    helpers::parse::parse_signers_into_co(to_review_file_path.clone(), context_lines);
    helpers::parse::parse_function_parameters_into_co(
        to_review_file_path.clone(),
        to_start_file_name.clone(),
    )?;

    println!("{to_start_file_name} file updated with instruction information");

    // create  co subfolder if user provided miro_oauth_access_token
    let miro_enabled = MiroConfig::new().miro_enabled();
    if miro_enabled {
        // if miro enabled, then create a subfolder
        let started_folder_path = BatConfig::get_auditor_code_overhaul_started_file_path(None)?;
        let started_co_folder_path = started_folder_path + entrypoint_name.clone().as_str();
        let started_co_file_path = format!("{started_co_folder_path}/{to_start_file_name}");
        // create the co subfolder
        Command::new("mkdir")
            .args([&started_co_folder_path])
            .output()
            .unwrap();
        // move the co file inside the folder: mv
        Command::new("mv")
            .args([&to_review_file_path, &started_co_folder_path])
            .output()
            .unwrap();
        println!("{to_start_file_name} file moved to started");
        // create the screenshots empty images: entrypoint, handler, context accounts and validations
        Command::new("touch")
            .current_dir(&started_co_folder_path)
            .args(CO_FIGURES)
            .output()
            .unwrap();
        println!("Empty screenshots created, remember to complete them");

        create_git_commit(
            GitCommit::StartCOMiro,
            Some(vec![to_start_file_name.clone()]),
        )?;

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_co_file_path.as_str())?;
    } else {
        let started_path = BatConfig::get_auditor_code_overhaul_started_file_path(Some(
            to_start_file_name.clone(),
        ))?;
        Command::new("mv")
            .args([to_review_file_path, started_path.clone()])
            .output()
            .unwrap();
        println!("{to_start_file_name} file moved to started");

        create_git_commit(GitCommit::StartCO, Some(vec![to_start_file_name]))?;

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_path.as_str())?;
    }
    Ok(())
}

pub async fn finish_code_overhaul_file() -> Result<(), String> {
    check_correct_branch()?;
    // get to-review files
    let started_endpoints = helpers::get::get_started_entrypoints()?;

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&started_endpoints)
        .default(0)
        .with_prompt("Select the code-overhaul to finish:")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to finished
        Some(index) => {
            if MiroConfig::new().miro_enabled() {
                let finished_endpoint = started_endpoints[index].clone();
                let finished_folder_path =
                    BatConfig::get_auditor_code_overhaul_finished_path(None)?;
                let started_folder_path =
                    BatConfig::get_auditor_code_overhaul_started_file_path(None)?;
                let started_co_folder_path =
                    canonicalize_path(format!("{started_folder_path}/{finished_endpoint}"));
                let started_co_file_path = canonicalize_path(format!(
                    "{started_folder_path}/{finished_endpoint}/{finished_endpoint}.md"
                ));
                helpers::check::code_overhaul_file_completed(
                    started_co_file_path.clone(),
                    finished_endpoint.clone(),
                );
                // move Miro frame to final positon
                let (_, _, finished_co) = helpers::count::co_counter()?;
                miro::api::frame::update_frame_position(
                    finished_endpoint.clone(),
                    finished_co as i32,
                )
                .await?;
                // move into finished
                Command::new("mv")
                    .args([started_co_file_path, finished_folder_path])
                    .output()
                    .unwrap();
                // remove co subfolder
                Command::new("rm")
                    .args(["-rf", &started_co_folder_path])
                    .output()
                    .unwrap();
                println!("{finished_endpoint} moved to finished");
                create_git_commit(GitCommit::FinishCOMiro, Some(vec![finished_endpoint]))?;
            } else {
                let finished_file_name = started_endpoints[index].clone();
                let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(Some(
                    finished_file_name.clone(),
                ))?;
                let started_path = BatConfig::get_auditor_code_overhaul_started_file_path(Some(
                    finished_file_name.clone(),
                ))?;
                helpers::check::code_overhaul_file_completed(
                    started_path.clone(),
                    finished_file_name.clone(),
                );
                Command::new("mv")
                    .args([started_path, finished_path])
                    .output()
                    .unwrap();
                println!("{finished_file_name} file moved to finished");
                create_git_commit(GitCommit::FinishCO, Some(vec![finished_file_name]))?;
            }
        }
        None => panic!("User did not select anything"),
    }
    Ok(())
}

pub fn update_code_overhaul_file() -> Result<(), String> {
    println!("Select the code-overhaul file to finish:");
    let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(None)?;
    // get to-review files
    let finished_files = fs::read_dir(finished_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if finished_files.is_empty() {
        panic!("no finished files in code-overhaul folder");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&finished_files)
        .default(0)
        .with_prompt("Select the code-overhaul file to update:")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to finished
        Some(index) => {
            let finished_file_name = finished_files[index].clone();
            check_correct_branch()?;
            create_git_commit(GitCommit::UpdateCO, Some(vec![finished_file_name]))?;
            Ok(())
        }
        None => panic!("User did not select anything"),
    }
}

pub fn count_co_files() -> Result<(), String> {
    let (to_review_count, started_count, finished_count) = helpers::count::co_counter()?;
    println!("to-review co files: {}", format!("{to_review_count}").red());
    println!("started co files: {}", format!("{started_count}").yellow());
    println!("finished co files: {}", format!("{finished_count}").green());
    println!(
        "total co files: {}",
        format!("{}", to_review_count + started_count + finished_count).purple()
    );
    Ok(())
}

pub async fn open_co() -> Result<(), String> {
    let BatConfig {
        auditor, required, ..
    } = BatConfig::get_validated_config()?;
    // list to start
    if auditor.vs_code_integration {
        let started_path = BatConfig::get_auditor_code_overhaul_path()? + "started";
        let co_files = helpers::get::get_only_files_from_folder(started_path)?;
        let co_files = co_files
            .iter()
            .filter(|f| f.name != ".gitkeep")
            .collect::<Vec<_>>();
        if !co_files.is_empty() {
            let started_entrypoints = helpers::get::get_started_entrypoints()?;
            let selection = Select::with_theme(&ColorfulTheme::default())
                .items(&started_entrypoints)
                .default(0)
                .with_prompt("Select the code-overhaul file to open:")
                .interact_on_opt(&Term::stderr())
                .unwrap();
            // user select file
            let (started_file_name, started_file_path) = match selection {
                // move selected file to finished
                Some(index) => (
                    started_entrypoints[index].clone(),
                    BatConfig::get_auditor_code_overhaul_started_file_path(Some(
                        started_entrypoints[index].clone(),
                    ))?,
                ),
                None => panic!("User did not select anything"),
            };
            // select to start
            // get instruction
            let (_, instruction_file_path) =
                helpers::get::get_instruction_file_with_prompts(&started_file_name)?;

            vs_code_open_file_in_current_window(&started_file_path)?;
            vs_code_open_file_in_current_window(&instruction_file_path)?;
        }
        vs_code_open_file_in_current_window(&required.program_lib_path)?;
    } else {
        print!("VSCode integration not enabled");
    }
    Ok(())
}

pub fn update_audit_results() -> Result<(), String> {
    let audit_file_path =
        BatConfig::get_audit_folder_path(Some(AUDIT_RESULT_FILE_NAME.to_string()))?;
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
    create_git_commit(GitCommit::Results, None)?;
    Ok(())
}
