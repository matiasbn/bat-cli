#[derive(Debug)]
struct FileInfo {
    path: String,
    name: String,
}

use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{MultiSelect, Select};

use walkdir::WalkDir;

use crate::command_line::{canonicalize_path, vs_code_open_file_in_current_window};
use crate::commands::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::commands::miro::miro_api::frame::{create_frame, create_image_from_device};
use crate::commands::miro::miro_api::miro_enabled;
use crate::config::{BatConfig, RequiredConfig};
use crate::constants::{
    CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER, CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER,
    CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER, CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
    CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER,
    CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER,
    CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER, CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER,
    CO_FIGURES,
};

use std::borrow::{Borrow, BorrowMut};
use std::fmt::format;
use std::fs::{File, ReadDir};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::String;
use std::{fs, io};

pub fn create_overhaul_file(entrypoint_name: String) {
    let code_overhaul_auditor_file_path =
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(entrypoint_name.clone()));
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        panic!("code overhaul file already exists for: {entrypoint_name:?}");
    }
    let output = Command::new("cp")
        .args([
            "-r",
            BatConfig::get_code_overhaul_template_path().as_str(),
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
}

pub fn start_code_overhaul_file() {
    check_correct_branch();

    // check if program_lib_path is not empty or panic
    let BatConfig { optional, .. } = BatConfig::get_validated_config();

    if optional.program_instructions_path.is_empty() {
        panic!("Optional program_instructions_path parameter not set in Bat.toml")
    }

    if !Path::new(&optional.program_instructions_path).is_dir() {
        panic!("program_instructions_path is not a correct folder")
    }

    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None);
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
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(to_start_file_name.clone()));

    let instruction_files_info = get_instruction_files();

    let entrypoint_name = to_start_file_name.replace(".md", "");
    let instruction_match = instruction_files_info
        .iter()
        .filter(|ifile| ifile.name.replace(".rs", "") == entrypoint_name.as_str())
        .collect::<Vec<&FileInfo>>();

    // if instruction exists, prompt the user if the file is correct
    let is_match = if instruction_match.len() == 1 {
        let instruction_match_path = Path::new(&instruction_match[0].path)
            .canonicalize()
            .unwrap();
        let options = vec!["yes", "no"];
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(
                instruction_match_path
                    .into_os_string()
                    .into_string()
                    .unwrap()
                    + " <--- is this the correct instruction file?:",
            )
            .items(&options)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .unwrap()
            .unwrap();

        options[selection] == "yes"
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
    let instruction_file_path = Path::new(&instruction_file_path).canonicalize().unwrap();
    let context_lines =
        get_context_lines(instruction_file_path.clone(), to_start_file_name.clone());

    // open instruction file in VSCode
    vs_code_open_file_in_current_window(instruction_file_path.to_str().unwrap());

    // parse text into co file
    parse_context_accounts_into_co(
        Path::new(&(to_review_file_path.clone()))
            .canonicalize()
            .unwrap(),
        context_lines.clone(),
    );
    parse_validations_into_co(to_review_file_path.clone(), context_lines.clone());
    parse_signers_into_co(to_review_file_path.clone(), context_lines);
    parse_function_parameters_into_co(to_review_file_path.clone(), to_start_file_name.clone());

    println!("{to_start_file_name} file updated with instruction information");

    // create  co subfolder if user provided miro_oauth_access_token
    let miro_enabled = miro_enabled();
    if miro_enabled {
        // if miro enabled, then create a subfolder
        let started_folder_path = BatConfig::get_auditor_code_overhaul_started_path(None);
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

        create_git_commit(GitCommit::StartCOMiro, Some(vec![to_start_file_name]));

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_co_file_path.as_str());
    } else {
        let started_path =
            BatConfig::get_auditor_code_overhaul_started_path(Some(to_start_file_name.clone()));
        Command::new("mv")
            .args([to_review_file_path, started_path.clone()])
            .output()
            .unwrap();
        println!("{to_start_file_name} file moved to started");

        create_git_commit(GitCommit::StartCO, Some(vec![to_start_file_name]));

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_path.as_str());
    }
}

pub fn finish_code_overhaul_file() {
    check_correct_branch();
    // get to-review files
    let started_endpoints = get_started_entrypoints();

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
            if miro_enabled() {
                let finished_endpoint = started_endpoints[index].clone();
                let finished_folder_path = BatConfig::get_auditor_code_overhaul_finished_path(None);
                let started_folder_path = BatConfig::get_auditor_code_overhaul_started_path(None);
                let started_co_folder_path =
                    canonicalize_path(format!("{started_folder_path}/{finished_endpoint}"));
                let started_co_file_path = canonicalize_path(format!(
                    "{started_folder_path}/{finished_endpoint}/{finished_endpoint}.md"
                ));
                check_code_overhaul_file_completed(
                    started_co_file_path.clone(),
                    finished_endpoint.clone(),
                );
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
                create_git_commit(GitCommit::FinishCOMiro, Some(vec![finished_endpoint]));
            } else {
                let finished_file_name = started_endpoints[index].clone();
                let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(Some(
                    finished_file_name.clone(),
                ));
                let started_path = BatConfig::get_auditor_code_overhaul_started_path(Some(
                    finished_file_name.clone(),
                ));
                check_code_overhaul_file_completed(
                    started_path.clone(),
                    finished_file_name.clone(),
                );
                Command::new("mv")
                    .args([started_path, finished_path])
                    .output()
                    .unwrap();
                println!("{finished_file_name} file moved to finished");
                create_git_commit(GitCommit::FinishCO, Some(vec![finished_file_name]));
            }
        }
        None => println!("User did not select anything"),
    }
}

pub fn update_code_overhaul_file() {
    println!("Select the code-overhaul file to finish:");
    let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(None);
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
            check_correct_branch();
            create_git_commit(GitCommit::UpdateCO, Some(vec![finished_file_name]));
        }
        None => println!("User did not select anything"),
    }
}

pub fn count_co_files() {
    let (to_review_count, started_count, finished_count) = co_counter();
    println!("to-review co files: {}", format!("{to_review_count}").red());
    println!("started co files: {}", format!("{started_count}").yellow());
    println!("finished co files: {}", format!("{finished_count}").green());
    println!(
        "total co files: {}",
        format!("{}", to_review_count + started_count + finished_count).purple()
    );
}

pub fn co_counter() -> (usize, usize, usize) {
    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None);
    let to_review_folder = fs::read_dir(to_review_path).unwrap();
    let to_review_count = count_filtered(to_review_folder);
    let started_path = BatConfig::get_auditor_code_overhaul_started_path(None);
    let started_folder = fs::read_dir(started_path).unwrap();
    let started_count = count_filtered(started_folder);
    let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(None);
    let finished_folder = fs::read_dir(finished_path).unwrap();
    let finished_count = count_filtered(finished_folder);
    (to_review_count, started_count, finished_count)
}

pub async fn deploy_miro() {
    assert!(miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    // check empty images
    // get files and folders from started, filter .md files
    let started_folders: Vec<String> = get_started_entrypoints()
        .iter()
        .filter(|file| !file.contains(".md"))
        .map(|file| file.to_string())
        .collect();
    if started_folders.is_empty() {
        panic!("No folders found in started folder for the auditor")
    }
    let prompt_text = "select the folder to deploy to Miro".to_string();
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .items(&started_folders)
        .default(0)
        .interact_on_opt(&Term::stderr())
        .unwrap()
        .unwrap();

    let selected_folder = &started_folders[selection];
    let selected_co_started_path = BatConfig::get_auditor_code_overhaul_started_path(None);
    let screenshot_paths = CO_FIGURES
        .iter()
        .map(|figure| format!("{selected_co_started_path}/{selected_folder}/{figure}"));

    // check if some of the screenshots is empty
    for path in screenshot_paths.clone() {
        let screenshot_file = fs::read(&path).unwrap();
        let screenshot_name = path.split('/').clone().last().unwrap();
        if screenshot_file.is_empty() {
            panic!("{screenshot_name} screenshot file is empty, please complete it");
        }
    }

    // create the Miro frame
    // Replace placeholder with Miro url
    let started_co_file_path =
        format!("{selected_co_started_path}/{selected_folder}/{selected_folder}.md");
    let to_start_file_content = fs::read_to_string(&started_co_file_path).unwrap();

    // only create the frame if it was not created yet
    if to_start_file_content.contains(CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER) {
        let miro_frame = create_frame(selected_folder).await;
        fs::write(
            &started_co_file_path,
            to_start_file_content
                .replace(CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER, &miro_frame.url),
        )
        .unwrap();
    }

    // Upload images
    let prompt_text = format!("select the images to upload for {selected_folder}");

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .items(CO_FIGURES)
        .interact_on_opt(&Term::stderr())
        .unwrap()
        .unwrap();
    if !selections.is_empty() {
        for selection in selections.iter() {
            let screenshot_path_vec = &screenshot_paths.clone().collect::<Vec<_>>();
            let screenshot_path = &screenshot_path_vec.as_slice()[*selection];
            let file_name = screenshot_path.split('/').last().unwrap();
            println!("Uploading: {file_name}");
            create_image_from_device(screenshot_path.to_string()).await;
        }
    } else {
        println!("No files selected");
    }
}

fn count_filtered(dir_to_count: ReadDir) -> usize {
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

fn parse_context_accounts_into_co(co_file_path: PathBuf, context_lines: Vec<String>) {
    let filtered_context_account_lines: Vec<_> = context_lines
        .iter()
        .map(|line| {
            // if has validation in a single line, then delete the validation, so the filters don't erase them
            if line.contains("#[account(")
                && line.contains(")]")
                && (line.contains("constraint") || line.contains("has_one"))
            {
                let new_line = line
                    .split(',')
                    .filter(|element| {
                        !(element.contains("has_one") || element.contains("constraint"))
                    })
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()
                    .join(",");
                new_line + ")]"
            } else {
                line.to_string()
            }
        })
        .filter(|line| !line.contains("constraint "))
        .filter(|line| !line.contains("has_one "))
        .collect();

    let mut formatted_lines: Vec<String> = vec!["- ```rust".to_string()];
    for (idx, line) in filtered_context_account_lines.iter().enumerate() {
        // if the current line opens an account, and next does not closes it
        if line.replace(' ', "") == "#[account("
            && filtered_context_account_lines[idx + 1].replace(' ', "") != ")]"
        {
            let mut counter = 1;
            let mut lines_to_add: Vec<String> = vec![];
            // iterate next lines until reaching )]
            while filtered_context_account_lines[idx + counter].replace(' ', "") != ")]" {
                let next_line = filtered_context_account_lines[idx + counter].clone();
                lines_to_add.push(next_line);
                counter += 1;
            }

            // single attribute, join to single line
            if counter == 2 {
                formatted_lines.push(
                    line.to_string() + lines_to_add[0].replace([' ', ','], "").as_str() + ")]",
                )
            // multiple attributes, join to multiple lines
            } else {
                // multiline attributes, join line, the lines_to_add and the closure )] line
                formatted_lines.push(
                    [
                        &[line.to_string()],
                        &lines_to_add[..],
                        &[filtered_context_account_lines[idx + counter].clone()],
                    ]
                    .concat()
                    .join("\n  "),
                );
            }
        // if the line defines an account, is a comment, an empty line or closure of context accounts
        } else if line.contains("pub")
            || line.contains("///")
            || line.replace(' ', "") == "}"
            || line.is_empty()
        {
            formatted_lines.push(line.to_string())
        // if is an already single line account
        } else if line.contains("#[account(") && line.contains(")]") {
            formatted_lines.push(line.to_string())
        }
    }
    formatted_lines.push("```".to_string());

    // replace formatted lines in co file
    let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
        CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER,
        formatted_lines.join("\n  ").as_str(),
    );
    fs::write(co_file_path, data).unwrap();
}

fn parse_validations_into_co(co_file_path: String, context_lines: Vec<String>) {
    let filtered_lines: Vec<_> = context_lines
        .iter()
        .filter(|line| !line.contains("///"))
        .map(|line| line.replace('\t', ""))
        .collect();
    let mut validations: Vec<String> = Vec::new();

    for (line_number, line) in filtered_lines.iter().enumerate() {
        if line.contains("#[account(") {
            let mut idx = 1;
            // set the first line as a rust snippet on md
            let mut account_string = vec![line.to_string()];
            // if next line is pub
            while !filtered_lines[line_number + idx].contains("pub ") {
                if filtered_lines[line_number + idx].contains("constraint =")
                    || filtered_lines[line_number + idx].contains("has_one")
                    || filtered_lines[line_number + idx].contains(")]")
                    || filtered_lines[line_number + idx].contains("pub ")
                {
                    account_string.push(filtered_lines[line_number + idx].to_string());
                }
                idx += 1;
            }
            // end of md section
            account_string.push(filtered_lines[line_number + idx].clone());
            // filter empty lines, like accounts without nothing or account mut
            if !(account_string[1].contains("#[account(") && account_string[2].contains(")]"))
                && !account_string[1].contains("#[account(mut)]")
            {
                validations.push(account_string.join("\n"));
            }
        }
    }
    // filter only validations
    validations = validations
        .iter()
        .filter(|validation| validation.contains("has_one") || validation.contains("constraint"))
        .map(|validation| validation.to_string())
        .collect();

    // replace in co file
    if validations.is_empty() {
        let data = fs::read_to_string(co_file_path.clone())
            .unwrap()
            .replace(
                CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER,
                CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER,
            )
            .replace(
                CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER,
                CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER,
            );
        fs::write(co_file_path.clone(), data).unwrap()
    }

    let mut account_validations: Vec<String> = vec![];
    let mut prerequisites: Vec<String> = vec![];

    for validation in validations.iter() {
        let options = vec!["account validation", "prerequisite"];
        let prompt_text =
            format!("is this validation an account validation or a prerequisite?: \n {validation}");
        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt_text)
            .items(&options)
            .default(0)
            .interact_on_opt(&Term::stderr())
            .unwrap()
            .unwrap();

        if options[selection] == options[0] {
            account_validations.push("- ```rust".to_string());
            account_validations.push(validation.to_string());
            account_validations.push("   ```".to_string());
        } else {
            account_validations.push("- ```rust".to_string());
            prerequisites.push(validation.to_string());
            account_validations.push("   ```".to_string());
        }
    }

    let co_file_content = fs::read_to_string(co_file_path.clone()).unwrap();

    let accounts_validations_string = if account_validations.is_empty() {
        "- NONE".to_string()
    } else {
        account_validations.join("\n")
    };
    let prerequisites_string = if prerequisites.is_empty() {
        "- NONE".to_string()
    } else {
        prerequisites.join("\n")
    };
    fs::write(
        co_file_path,
        co_file_content
            .replace(
                CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER,
                accounts_validations_string.as_str(),
            )
            .replace(
                CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER,
                prerequisites_string.as_str(),
            ),
    )
    .unwrap();
}

fn parse_signers_into_co(co_file_path: String, context_lines: Vec<String>) {
    // signer names is only the name of the signer
    let signers_names: Vec<_> = context_lines
        .iter()
        .filter(|line| line.contains("Signer"))
        .map(|line| {
            line.replace("pub ", "")
                .replace("  ", "")
                .split(':')
                .collect::<Vec<&str>>()[0]
                .to_string()
        })
        .collect();
    // array of signers description: - signer_name: SIGNER_DESCRIPTION
    let mut signers_text: Vec<String> = vec![];
    for signer in signers_names.clone() {
        let signer_index = context_lines
            .iter()
            .position(|line| line.contains(&signer) && line.contains("pub"))
            .unwrap();
        let mut index = 1;
        let mut candidate_lines: Vec<String> = vec![];
        // move up through the lines until getting a pub
        while !context_lines[signer_index - index].clone().contains("pub") {
            // push only if is a comment
            if context_lines[signer_index - index].contains("//") {
                candidate_lines.push(context_lines[signer_index - index].clone());
            }
            index += 1;
        }
        // no comments detected, replace with placeholder
        if candidate_lines.is_empty() {
            signers_text
                .push("- ".to_string() + &signer + ": " + CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER);
        // only 1 comment
        } else if candidate_lines.len() == 1 {
            // prompt the user to state if the comment is correct
            let signer_description = candidate_lines[0].split("// ").last().unwrap();
            let prompt_text = format!(
                "is this a proper description of the signer '{signer}'?: '{signer_description}'"
            );
            let options = vec!["yes", "no"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .items(&options)
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();

            if options[selection] == options[0] {
                signers_text.push("- ".to_string() + &signer + ": " + signer_description);
            } else {
                signers_text.push(
                    "- ".to_string() + &signer + ": " + CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
                );
            }
        // multiple line description
        } else {
            // prompt the user to select the lines that contains the description and join them
            let prompt_text = format!(
                "Use the spacebar to select the lines that describes the signer '{signer}'. \n Hit enter if is not a proper description:"
            );
            candidate_lines.reverse();
            let formatted_candidate_lines: Vec<&str> = candidate_lines
                .iter()
                .map(|line| line.split("// ").last().unwrap())
                .collect();
            let selections = MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .items(&formatted_candidate_lines)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            if selections.is_empty() {
                signers_text.push(
                    "- ".to_string() + &signer + ": " + CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
                );
            } else {
                // take the selections and create the array
                let mut signer_description_lines: Vec<String> = vec![];
                for selection in selections.iter() {
                    signer_description_lines
                        .push(formatted_candidate_lines.as_slice()[*selection].to_string());
                }
                signers_text.push(
                    "- ".to_string() + &signer + ": " + signer_description_lines.join(" ").as_str(),
                );
            }
        }
    }

    // replace in co file
    let signers_text_to_replace = if signers_names.is_empty() {
        "- No signers found".to_string()
    } else {
        signers_text.join("\n")
    };

    let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
        CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER,
        signers_text_to_replace.as_str(),
    );
    fs::write(co_file_path, data).unwrap();
}

fn parse_function_parameters_into_co(co_file_path: String, co_file_name: String) {
    let BatConfig { required, .. } = BatConfig::get_validated_config();
    let RequiredConfig {
        program_lib_path, ..
    } = required;

    let lib_file = File::open(program_lib_path).unwrap();
    let mut lib_files_lines = io::BufReader::new(lib_file).lines().map(|l| l.unwrap());
    lib_files_lines
        .borrow_mut()
        .enumerate()
        .find(|(_, line)| *line == String::from("#[program]"))
        .unwrap();

    let mut program_lines = vec![String::from(""); 0];
    for (_, line) in lib_files_lines.borrow_mut().enumerate() {
        if line == "}" {
            break;
        }
        program_lines.push(line)
    }
    let entrypoint_text = "pub fn ".to_string() + co_file_name.replace(".md", "").as_str();
    let entrypoint_index = program_lines
        .iter()
        .position(|line| line.contains(entrypoint_text.clone().as_str()))
        .unwrap();
    let mut canditate_lines = vec![program_lines[entrypoint_index].clone()];
    let mut idx = 0;
    // collect lines until closing parenthesis
    while !program_lines[entrypoint_index + idx].contains(')') {
        canditate_lines.push(program_lines[entrypoint_index + idx].clone());
        idx += 1;
    }
    // same line parameters
    if idx == 0 {
        // split by "->"
        // take only the first element
        let mut function_line = canditate_lines[0].split("->").collect::<Vec<_>>()[0]
            .to_string()
            // replace ) by ""
            .replace(')', "")
            // split by ","
            .split(", ")
            // if no : then is a lifetime
            .filter(|l| l.contains(':'))
            .map(|l| l.to_string())
            .collect::<Vec<_>>();
        // if the split produces 1 element, then there's no parameters
        if function_line.len() == 1 {
            let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
                CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
                ("- ".to_string() + CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER)
                    .as_str(),
            );
            fs::write(co_file_path, data).unwrap();
        } else {
            // delete first element
            function_line.remove(0);
            // join
            let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
                CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
                ("- ```rust\n  ".to_string() + function_line.join("\n  ").as_str() + "\n  ```")
                    .as_str(),
            );
            fs::write(co_file_path, data).unwrap();
        }
    } else {
        let parameters_lines = canditate_lines
            .iter()
            .filter(|line| !line.contains("fn") && !line.contains("Context"))
            .map(|l| {
                l.to_string()
                    .replace(' ', "")
                    .replace(':', ": ")
                    .replace(';', "; ")
            })
            .collect::<Vec<_>>();
        let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
            CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
            ("- ```rust\n  ".to_string() + parameters_lines.join("\n  ").as_str() + "\n  ```")
                .as_str(),
        );
        fs::write(co_file_path, data).unwrap();
    }
}

fn get_context_name(co_file_name: String) -> String {
    let BatConfig { required, .. } = BatConfig::get_validated_config();
    let RequiredConfig {
        program_lib_path, ..
    } = required;

    let lib_file = fs::read_to_string(program_lib_path).unwrap();
    let lib_file_lines: Vec<&str> = lib_file.lines().collect();

    let entrypoint_index = lib_file
        .lines()
        .position(|line| {
            if line.contains("pub fn") {
                let function_name = line.split('(').collect::<Vec<&str>>()[0]
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
    parsed_context_name
}

fn get_context_lines(instruction_file_path: PathBuf, co_file_name: String) -> Vec<String> {
    let instruction_file = File::open(instruction_file_path.clone()).unwrap();
    let instruction_file_lines = io::BufReader::new(instruction_file)
        .lines()
        .map(|l| l.unwrap())
        .collect::<Vec<String>>();

    let context_name = get_context_name(co_file_name.clone());
    // get context lines
    let first_line_index_opt = instruction_file_lines.iter().position(|line| {
        line.contains(("pub struct ".to_string() + &context_name.clone() + "<").as_str())
    });
    match first_line_index_opt {
        Some(first_line_index) => {
            // the closing curly brace "}", starting on first_line_index
            let last_line_index = instruction_file_lines[first_line_index..]
                .iter()
                .position(|line| line == &"}")
                .unwrap()
                + first_line_index;
            let context_lines: Vec<_> =
                instruction_file_lines[first_line_index..=last_line_index].to_vec();
            context_lines
        }
        // if the Context Accouns were not found in the file
        None => {
            // tell the user that the context was not found in the instruction file
            let co_name = co_file_name.replace(".md", "");
            let instruction_file_name = instruction_file_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            // tell the user to select the instruction file that has the context of the file
            let instruction_files = get_instruction_files();
            let instruction_files_names: Vec<&String> =
                instruction_files.iter().map(|file| &file.name).collect();
            // list the instruction files
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Context Accounts not found for {co_name} in {instruction_file_name}, please select the file that contains the context:",
                ))
                .items(&instruction_files_names)
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            let selected_instruction_file = &instruction_files[selection];
            let instruction_file = File::open(selected_instruction_file.path.clone()).unwrap();
            let instruction_file_lines = io::BufReader::new(instruction_file)
                .lines()
                .map(|l| l.unwrap())
                .collect::<Vec<String>>();
            // get context lines
            // check if the context is in the file
            let first_line_index = instruction_file_lines
                .iter()
                .position(|line| {
                    line.contains(
                        ("pub struct ".to_string() + &context_name.clone() + "<").as_str(),
                    )
                })
                // if is not in the file, panic
                .unwrap();
            let last_line_index = instruction_file_lines[first_line_index..]
                .iter()
                .position(|line| line == &"}")
                .unwrap()
                + first_line_index;
            let context_lines: Vec<_> =
                instruction_file_lines[first_line_index..=last_line_index].to_vec();
            context_lines
        }
    }
}

fn check_code_overhaul_file_completed(file_path: String, file_name: String) {
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

    if file_data.contains(CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER) {
        panic!("Please complete the \"Miro board frame\" section of the {file_name} file");
    }
}

fn get_instruction_files() -> Vec<FileInfo> {
    let instructions_path = BatConfig::get_validated_config()
        .optional
        .program_instructions_path;

    let mut instruction_files_info = WalkDir::new(instructions_path)
        .into_iter()
        .map(|entry| {
            let info = FileInfo {
                path: entry.as_ref().unwrap().path().display().to_string(),
                name: entry
                    .as_ref()
                    .unwrap()
                    .file_name()
                    .to_os_string()
                    .into_string()
                    .unwrap(),
            };
            info
        })
        .filter(|file_info| file_info.name != "mod.rs" && file_info.name.contains(".rs"))
        .collect::<Vec<FileInfo>>();
    instruction_files_info.sort_by(|a, b| a.name.cmp(&b.name));
    instruction_files_info
}

// returns a list of folder and files names
fn get_started_entrypoints() -> Vec<String> {
    let started_path = BatConfig::get_auditor_code_overhaul_started_path(None);
    let started_files = fs::read_dir(started_path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    if started_files.is_empty() {
        panic!("no started files in code-overhaul folder");
    }
    started_files
}
