#[derive(Debug)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
}
use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{MultiSelect, Select};
use std::io::Result;

use walkdir::WalkDir;

use crate::command_line::{canonicalize_path, vs_code_open_file_in_current_window};
use crate::commands::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::commands::miro::api::connector::ConnectorOptions;
use crate::commands::miro::{self, MiroConfig};
use crate::config::{BatConfig, RequiredConfig};
use crate::constants::{
    CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER, CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER,
    CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER, CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
    CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER, CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
    CODE_OVERHAUL_HANDLER_PLACEHOLDER, CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER,
    CODE_OVERHAUL_NOTES_PLACEHOLDER, CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER,
    CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER, CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER,
    CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER, CONTEXT_ACCOUNTS_PNG_NAME, CO_FIGURES,
    ENTRYPOINT_PNG_NAME, HANDLER_PNG_NAME, VALIDATIONS_PNG_NAME,
};

use std::borrow::{Borrow, BorrowMut};

use std::fs::{File, ReadDir};
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::String;
use std::{fs, io};

pub mod parse {
    use super::*;

    pub fn context_accounts_into_co(co_file_path: PathBuf, context_lines: Vec<String>) {
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

    pub fn validations_into_co(
        co_file_path: String,
        context_lines: Vec<String>,
        instruction_file_path: String,
    ) {
        let filtered_lines: Vec<_> = context_lines
            .iter()
            .filter(|line| !line.contains("///"))
            .map(|line| line.replace('\t', ""))
            .collect();
        let validations_strings = vec!["require", "valid", "assert"];
        let mut validations: Vec<String> =
            super::get::possible_validations(instruction_file_path, validations_strings.clone());
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
            .filter(|validation| {
                validation.contains("has_one")
                    || validation.contains("constraint")
                    || validations_strings
                        .iter()
                        .any(|validation_str| validation.contains(validation_str))
            })
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
            let options = vec![
                format!("account validation").red(),
                format!("prerequisite").yellow(),
            ];
            let prompt_text = format!(
                "is this validation an {} or a {}?: \n {}",
                options[0],
                options[1],
                format!("{validation}").green(),
            );
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
                prerequisites.push("- ```rust".to_string());
                prerequisites.push(validation.to_string());
                prerequisites.push("   ```".to_string());
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

    pub fn signers_into_co(co_file_path: String, context_lines: Vec<String>) {
        // signer names is only the name of the signer
        let signers_names = super::get::signers_description_from_co_file(&context_lines);
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
                signers_text.push(
                    "- ".to_string() + &signer + ": " + CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
                );
            // only 1 comment
            } else if candidate_lines.len() == 1 {
                // prompt the user to state if the comment is correct
                let signer_description = candidate_lines[0].split("// ").last().unwrap();
                let prompt_text = format!(
                    "is this a proper description of the signer {}?: '{signer_description}'",
                    format!("{signer}").red()
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
            "Use the spacebar to select the lines that describes the signer {}. \n Hit enter if is not a proper description:", format!("{signer}").red()
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
                        "- ".to_string()
                            + &signer
                            + ": "
                            + signer_description_lines.join(". ").as_str(),
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

    pub fn function_parameters_into_co(co_file_path: String, co_file_name: String) -> Result<()> {
        let BatConfig { required, .. } = BatConfig::get_validated_config()?;
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
        let entrypoint_text = "pub pub fn ".to_string() + co_file_name.replace(".md", "").as_str();
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
                .filter(|line| !line.contains("pub fn") && !line.contains("Context"))
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
        Ok(())
    }
}
pub mod get {
    use super::*;
    pub fn signers_description_from_co_file(context_lines: &Vec<String>) -> Vec<String> {
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
        signers_names
    }

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

    pub fn context_name(co_file_name: String) -> Result<String> {
        let BatConfig { required, .. } = BatConfig::get_validated_config()?;
        let RequiredConfig {
            program_lib_path, ..
        } = required;

        let lib_file = fs::read_to_string(program_lib_path).unwrap();
        let lib_file_lines: Vec<&str> = lib_file.lines().collect();

        let entrypoint_index = lib_file
            .lines()
            .position(|line| {
                if line.contains("pub pub fn") {
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
        Ok(parsed_context_name)
    }

    pub fn context_lines(
        instruction_file_path: PathBuf,
        co_file_name: String,
    ) -> Result<Vec<String>> {
        let instruction_file = File::open(instruction_file_path.clone()).unwrap();
        let instruction_file_lines = io::BufReader::new(instruction_file)
            .lines()
            .map(|l| l.unwrap())
            .collect::<Vec<String>>();

        let context_name = context_name(co_file_name.clone())?;
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
                Ok(context_lines)
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
                let instruction_files = super::get::instruction_files()?;
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
                Ok(context_lines)
            }
        }
    }

    pub fn possible_validations(
        instruction_file_path: String,
        validations_strings: Vec<&str>,
    ) -> Vec<String> {
        let instruction_file = fs::read_to_string(&instruction_file_path).unwrap();
        let mut possible_validations: Vec<String> = Vec::new();
        let lines_enumerate = instruction_file.lines().enumerate();
        for (line_index, line) in lines_enumerate {
            // if the line contains any of the validations and has a parenthesis
            if validations_strings
                .iter()
                .any(|validation| line.contains(validation))
                && line.contains('(')
            {
                // single line validation
                if line.contains(");") || line.contains(")?;") {
                    let validation = format!("\t{}", line.trim());
                    let prompt = format!(
                        "is the next line a {}? \n {}",
                        format!("validation").red(),
                        format!("{validation}").bright_green()
                    );
                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt(prompt)
                        .item("yes")
                        .item("no")
                        .default(0)
                        .interact_on_opt(&Term::stderr())
                        .unwrap()
                        .unwrap();
                    if selection == 0 {
                        possible_validations.push(validation);
                    }
                } else {
                    // multi line validation
                    let mut validation_candidate: Vec<String> = vec![line.to_string()];
                    let mut idx = 1;
                    let mut validation_line = instruction_file.clone().lines().collect::<Vec<_>>()
                        [line_index + idx]
                        .to_string();
                    while !(validation_line.contains(");") || validation_line.contains(")?;")) {
                        validation_candidate.push(validation_line);
                        idx += 1;
                        validation_line = instruction_file.clone().lines().collect::<Vec<_>>()
                            [line_index + idx]
                            .to_string();
                    }
                    validation_candidate.push(validation_line);
                    let validation_string = validation_candidate.join("\n");
                    let prompt = format!(
                        "is the next function a {}? \n {}",
                        format!("validation").red(),
                        format!("{validation_string}").bright_green(),
                    );
                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt(prompt)
                        .item("yes")
                        .item("no")
                        .default(0)
                        .interact_on_opt(&Term::stderr())
                        .unwrap()
                        .unwrap();
                    if selection == 0 {
                        possible_validations.push(validation_string);
                    }
                }
            }
        }
        possible_validations
    }

    pub fn instruction_files() -> Result<Vec<FileInfo>> {
        let instructions_path = BatConfig::get_validated_config()?
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
        Ok(instruction_files_info)
    }

    // returns a list of folder and files names
    pub fn started_entrypoints() -> Result<Vec<String>> {
        let started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
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
    ) -> Result<(String, String)> {
        let instruction_files_info = instruction_files()?;

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
        Ok((entrypoint_name.clone(), instruction_file_path.clone()))
    }
}

pub mod check {
    use super::*;
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
    pub fn co_counter() -> Result<(usize, usize, usize)> {
        let to_review_path = BatConfig::get_auditor_code_overhaul_to_review_path(None)?;
        let to_review_folder = fs::read_dir(to_review_path).unwrap();
        let to_review_count = count_filtering_gitkeep(to_review_folder);
        let started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
        let started_folder = fs::read_dir(started_path).unwrap();
        let started_count = count_filtering_gitkeep(started_folder);
        let finished_path = BatConfig::get_auditor_code_overhaul_finished_path(None)?;
        let finished_folder = fs::read_dir(finished_path).unwrap();
        let finished_count = count_filtering_gitkeep(finished_folder);
        Ok((to_review_count, started_count, finished_count))
    }
}
