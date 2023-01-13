#[derive(Debug)]
struct FileInfo {
    path: String,
    name: String,
}

use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use walkdir::WalkDir;

use crate::command_line::vs_code_open_file_in_current_window;
use crate::commands::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::commands::miro::miro_api::frame::create_frame;
use crate::config::{BatConfig, RequiredConfig};
use crate::constants::{
    CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER, CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
    CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER, CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER,
    CODE_OVERHAUL_NOTES_PLACEHOLDER, CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER,
    CODE_OVERHAUL_VALIDATION_PLACEHOLDER, CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER,
};

use std::borrow::{Borrow, BorrowMut};
use std::fs::File;
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

pub async fn start_code_overhaul_file() {
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

    let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path.clone()(Some(
        to_start_file_name.clone(),
    ));

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

    // parse text into co file
    parse_context_accounts_into_co(
        Path::new(&(to_review_path.clone())).canonicalize().unwrap(),
        context_lines.clone(),
    );
    parse_validations_into_co(to_review_path.clone(), context_lines.clone());
    parse_signers_into_co(to_review_path.clone(), context_lines);
    parse_function_parameters_into_co(to_review_path.clone(), to_start_file_name.clone());

    println!("{to_start_file_name} file updated with instruction information");

    // create frame into Miro if user provided miro_oauth_access_token
    if !BatConfig::get_validated_config()
        .auditor
        .miro_oauth_access_token
        .is_empty()
    {
        let RequiredConfig { miro_board_url, .. } = BatConfig::get_validated_config().required;
        let frame = create_frame(&entrypoint_name).await;
        let frame_id: String = frame["id"].clone().to_string().replace('\"', "");
        let frame_url: String = miro_board_url + "/?moveToWidget=" + &frame_id;
        // Replace placeholder with Miro url
        let to_start_file_content = fs::read_to_string(&to_review_path)
            .unwrap()
            .replace(CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER, &frame_url);
        fs::write(&to_review_path, to_start_file_content).unwrap()
    }

    // move to started
    let started_path =
        BatConfig::get_auditor_code_overhaul_started_path(Some(to_start_file_name.clone()));
    Command::new("mv")
        .args([to_review_path.clone(), started_path.clone()])
        .output()
        .unwrap();
    println!("{to_start_file_name} file moved to started");

    create_git_commit(GitCommit::StartCO, Some(vec![to_start_file_name]));

    // open VSCode files
    vs_code_open_file_in_current_window(instruction_file_path.to_str().unwrap());
    vs_code_open_file_in_current_window(started_path.as_str());
}

pub fn finish_code_overhaul_file() {
    let started_path = BatConfig::get_auditor_code_overhaul_started_path(None);
    // get to-review files
    let started_files = fs::read_dir(started_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if started_files.is_empty() {
        panic!("no started files in code-overhaul folder");
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&started_files)
        .default(0)
        .with_prompt("Select the code-overhaul file to finish:")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to finished
        Some(index) => {
            let finished_file_name = started_files[index].clone();
            let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(Some(
                finished_file_name.clone(),
            ));
            let started_path =
                BatConfig::get_auditor_code_overhaul_started_path(Some(finished_file_name.clone()));
            check_correct_branch();
            check_code_overhaul_file_completed(started_path.clone(), finished_file_name.clone());
            Command::new("mv")
                .args([started_path, finished_path])
                .output()
                .unwrap();
            println!("{finished_file_name} file moved to finished");
            create_git_commit(GitCommit::FinishCO, Some(vec![finished_file_name]));
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
    let mut accounts_groups: Vec<String> = Vec::new();

    for (line_number, line) in filtered_lines.iter().enumerate() {
        if line.contains("#[account(") {
            let mut idx = 1;
            // set the first line as a rust snippet on md
            let mut account_string = vec!["- ```rust".to_string(), line.to_string()];
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
            account_string.push("   ```".to_string());
            // filter empty lines, like accounts without nothing or account mut
            if !(account_string[1].contains("#[account(") && account_string[2].contains(")]"))
                && !account_string[1].contains("#[account(mut)]")
            {
                accounts_groups.push(account_string.join("\n"));
            }
        }
    }
    let accounts_string = accounts_groups.join("\n");

    // replace in co file
    if accounts_groups.is_empty() {
        let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
            CODE_OVERHAUL_VALIDATION_PLACEHOLDER,
            CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER,
        );
        fs::write(co_file_path.clone(), data).unwrap()
    }
    let co_file = File::open(co_file_path.clone()).unwrap();
    let co_file_lines = io::BufReader::new(co_file)
        .lines()
        .map(|l| l.unwrap())
        .map(|line| {
            if line == CODE_OVERHAUL_VALIDATION_PLACEHOLDER {
                accounts_string.clone()
            } else {
                line
            }
        })
        .collect::<Vec<String>>();
    fs::write(co_file_path, co_file_lines.join("\n")).unwrap();
}

fn parse_signers_into_co(co_file_path: String, context_lines: Vec<String>) {
    let signers_names: Vec<_> = context_lines
        .iter()
        .filter(|line| line.contains("Signer"))
        .map(|line| line.replace("pub ", ""))
        .map(|line| line.replace("  ", ""))
        .map(|line| {
            "- ".to_string()
                + line
                    .split(':')
                    .map(|l| l.to_string())
                    .collect::<Vec<String>>()[0]
                    .clone()
                    .as_str()
                + ": "
                + CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER
        })
        .collect();
    let signers_string = signers_names.join("\n");

    // replace in co file
    let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
        CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER,
        if signers_names.is_empty() {
            "No signers found"
        } else {
            signers_string.as_str() as _
        },
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
                let function_name = line.split("(").collect::<Vec<&str>>()[0]
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
