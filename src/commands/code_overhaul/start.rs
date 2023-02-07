use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{MultiSelect, Select};

use crate::batbelt::command_line::vs_code_open_file_in_current_window;

use crate::batbelt::constants::{
    CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER, CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER,
    CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER, CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
    CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER,
    CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER,
    CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER, CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER,
    CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER, CO_FIGURES, MIRO_BOARD_COLUMNS,
    MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;
use crate::config::{BatConfig, RequiredConfig};

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::batbelt::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results, get_multiple_line_validation,
    get_signers_description_from_co_file, get_single_line_validation,
    get_string_between_two_index_from_string, get_table_of_contents_for_results,
    prompt_check_validation,
};
use crate::batbelt::path::{FilePathType, FolderPathType};

use std::{env, fs};

use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::helpers::format::format_to_rust_comment;
use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::sonar::{get_function_parameters, BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::markdown::code_overhaul_template::CodeOverhaulSection;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::string::String;

pub fn start_co_file() -> Result<(), String> {
    check_correct_branch()?;
    let bat_config = BatConfig::get_validated_config().unwrap();
    let to_review_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, false);

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
    let prompt_text = "Select the code-overhaul file to start:";
    let selection = batbelt::cli_inputs::select(prompt_text, review_files.clone(), None)?;

    // user select file
    let to_start_file_name = &review_files[selection].clone();
    let entrypoint_name = to_start_file_name.replace(".md", "");
    let to_review_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulToReview {
            file_name: to_start_file_name.clone(),
        },
        false,
    );

    let instruction_file_path =
        batbelt::helpers::get::get_instruction_file_with_prompts(&to_start_file_name)?;

    let program_lib_path = bat_config.required.program_lib_path;

    let entrypoint_functions = BatSonar::new_from_path(
        &program_lib_path,
        Some("#[program"),
        SonarResultType::Function,
    );
    let entrypoint_function = entrypoint_functions
        .results
        .iter()
        .find(|function| function.name == entrypoint_name)
        .unwrap();

    let parameters = get_function_parameters(entrypoint_function.content.clone());
    let context_name = parameters
        .iter()
        .find(|parameter| parameter.contains("Context<"))
        .unwrap()
        .split("Context<")
        .last()
        .unwrap()
        .split(">")
        .next()
        .unwrap();

    let instruction_file_content = fs::read_to_string(&instruction_file_path).unwrap();
    let instruction_file_functions =
        BatSonar::new_scanned(&instruction_file_content, SonarResultType::Function);
    let handler_function = instruction_file_functions
        .results
        .iter()
        .find(|function| {
            let function_parameters = get_function_parameters(function.content.clone());
            function_parameters
                .iter()
                .any(|parameter| parameter.contains(&context_name))
        })
        .unwrap();

    let handler_if_statements = BatSonar::new_from_path(
        &instruction_file_path,
        Some(&handler_function.name),
        SonarResultType::If,
    );

    let if_validations = handler_if_statements
        .results
        .iter()
        .map(|if_validation| {
            let if_in_validations =
                BatSonar::new_scanned(&if_validation.content, SonarResultType::Validation);
            if !if_in_validations.results.is_empty() {
                if_in_validations.results
            } else {
                vec![]
            }
        })
        .fold(vec![], |mut result, current| {
            for res in current {
                result.push(res);
            }
            result
        });
    let handler_validations =
        BatSonar::new_from_path(&instruction_file_path, None, SonarResultType::Validation);

    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, true);
    let metadata_markdown = MarkdownFile::new(&metadata_path);
    let structs_section = metadata_markdown
        .get_section(&MetadataSection::Structs.to_sentence_case())
        .unwrap();
    let structs_subsections = metadata_markdown.get_section_subsections(structs_section);
    let context_source_code = structs_subsections
        .iter()
        .filter(|subsection| subsection.section_header.title == context_name)
        .map(|section| StructMetadata::from_markdown_section(section.clone()))
        .find(|struct_metadata| struct_metadata.struct_type == StructMetadataType::ContextAccounts)
        .unwrap()
        .get_source_code();

    let ca_content = context_source_code.get_source_code_content();
    let ca_accounts = BatSonar::new_scanned(&ca_content, SonarResultType::ContextAccounts);
    println!("ca \n{:#?}", ca_accounts.results);
    let started_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: to_start_file_name.clone(),
        },
        false,
    );

    let mut started_markdown_file = MarkdownFile::new(&to_review_file_path);

    let signers_section = started_markdown_file
        .get_section(&CodeOverhaulSection::Signers.to_title())
        .unwrap();

    let function_parameter_section = started_markdown_file
        .get_section(&CodeOverhaulSection::FunctionParameters.to_title())
        .unwrap();

    // Context accounts section
    let context_accounts_section = started_markdown_file
        .get_section(&CodeOverhaulSection::ContextAccounts.to_title())
        .unwrap();

    let context_accounts_content =
        get_context_account(&context_source_code.get_source_code_content());
    let mut new_accounts_section = context_accounts_section.clone();
    new_accounts_section.content = context_accounts_content;
    started_markdown_file
        .replace_section(new_accounts_section, context_accounts_section, vec![])
        .unwrap();

    let validations_section = started_markdown_file
        .get_section(&CodeOverhaulSection::Validations.to_title())
        .unwrap();

    started_markdown_file.save().unwrap();
    Command::new("mv")
        .args([to_review_file_path, started_path.clone()])
        .output()
        .unwrap();

    println!("{to_start_file_name} file moved to started");

    // create_git_commit(
    //     GitCommit::StartCO,
    //     Some(vec![to_start_file_name.to_string()]),
    // )?;

    // open co file in VSCode
    vs_code_open_file_in_current_window(started_path.as_str())?;
    // open instruction file in VSCode
    vs_code_open_file_in_current_window(&instruction_file_path)?;

    // println!("if statement\n{:#?}", handler_if_statements.results);
    // println!("handler valdiations\n{:#?}", handler_validations.results);
    // println!("ca accounts\n{:#?}", ca_accounts.results);

    Ok(())
}

pub fn get_context_account(context_accounts_content: &str) -> String {
    let context_lines = context_accounts_content.lines().collect::<Vec<_>>();
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

    let mut formatted_lines: Vec<String> = vec![];
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

    let ca_content = formatted_lines
        .iter()
        .map(|line| format!("  {}", line))
        .collect::<Vec<_>>()
        .join("\n");
    format!("{}\n{}\n{}", "- ```rust", ca_content, "  ```")
}

pub fn parse_validations_into_co(co_file_path: String, instruction_file_path: String) {
    let validations_strings = vec![
        "require".to_string(),
        "valid".to_string(),
        "assert".to_string(),
        "verify".to_string(),
    ];
    let mut validations: Vec<String> = vec![];
    let instruction_file_string = fs::read_to_string(&instruction_file_path).unwrap();

    let mut last_reviewed_line = 0;
    for (line_index, line) in instruction_file_string
        .lines()
        .into_iter()
        .enumerate()
        .map(|f| (f.0, f.1.to_string()))
    {
        if line_index < last_reviewed_line || line.is_empty() {
            continue;
        }
        // check the if statements
        let is_if = line.clone().trim().split(" ").next().unwrap() == "if";
        if is_if {
            // check that is not in comment
            if line.contains("//") {
                let tokenized_line = line.split_whitespace();
                let comment_position = tokenized_line.clone().position(|word| word.contains("//"));
                let if_position = tokenized_line.clone().position(|word| word.contains("if"));
                // if the if is after the comment, continue
                if if_position >= comment_position {
                    continue;
                }
            }
            let instruction_clone = instruction_file_string.clone();
            let mut instruction_lines = instruction_clone.lines().enumerate();
            let find_brace = instruction_lines
                .find(|(l_index, line)| line.contains("{") && l_index >= &line_index);
            // check that the if is correct by looking up {
            if let Some(found) = find_brace {
                let (opening_brace_index, _) = found;
                let (mut closing_brace_index, mut closing_brace_line) = instruction_lines
                    .find(|(l_index, line)| line.contains("}") && l_index >= &line_index)
                    .unwrap();
                // if closing line contains an else (or else if)
                while !(closing_brace_line.contains("}") && !closing_brace_line.contains("else")) {
                    (closing_brace_index, closing_brace_line) = instruction_lines.next().unwrap();
                }
                // check if exists a validation inside the if
                let if_lines = &instruction_file_string.lines().collect::<Vec<_>>()
                    [opening_brace_index..=closing_brace_index];
                // check if there are validations inside the if
                if if_lines.clone().to_vec().iter().any(|if_line| {
                    validations_strings
                        .clone()
                        .iter()
                        .any(|validation| if_line.contains(validation))
                }) {
                    last_reviewed_line = closing_brace_index;
                    validations.push(if_lines.to_vec().join("\n"))
                }
            };

            // if the line contains any of the validations and has a an opening parenthesis
        } else if validations_strings
            .iter()
            .any(|validation| line.contains(validation))
            && line.contains('(')
        {
            // single line validation
            if line.contains(");") || line.contains(")?;") {
                let is_validation = prompt_check_validation(line.clone());
                if is_validation {
                    validations.push(line);
                }
            } else {
                let instruction_file_lines = instruction_file_string.lines();
                let validation_closing_index = instruction_file_lines
                    .clone()
                    .into_iter()
                    .enumerate()
                    .position(|(l_index, l)| {
                        (l.trim() == ");"
                            || l.trim() == ")?;"
                            || l.trim() == ")"
                            || l.trim() == ")?")
                            && l_index > line_index
                    });
                if let Some(closing_index) = validation_closing_index {
                    let validation_string = get_string_between_two_index_from_string(
                        instruction_file_string.to_string(),
                        line_index,
                        closing_index,
                    )
                    .unwrap();
                    let is_validation = prompt_check_validation(validation_string.clone());
                    if is_validation {
                        validations.push(validation_string);
                    }
                };
            }
            // multi line account only has #[account(
        } else if line.trim() == "#[account(" {
            let closing_account_index = instruction_file_string
                .clone()
                .lines()
                .into_iter()
                .enumerate()
                .position(|(l_index, l)| l.trim() == ")]" && l_index > line_index)
                .unwrap();
            let account_lines = get_string_between_two_index_from_string(
                instruction_file_string.clone(),
                line_index,
                closing_account_index,
            )
            .unwrap();
            // accounts without validations inside are length = 2
            if account_lines
                .split("\n")
                .filter(|l| l.contains("has_one") || l.contains("constraint"))
                .collect::<Vec<_>>()
                .len()
                > 2
            {
                let is_validation = prompt_check_validation(account_lines.clone());
                if is_validation {
                    validations.push(account_lines);
                }
            }
            // single line "#[account(", push the next lines which is the account name
        } else if line.contains("#[account(") {
            let possible_validation = line.to_string().replace("mut,", "")
                + "\n"
                + &instruction_file_string.split("\n").collect::<Vec<_>>()[line_index + 1];
            if possible_validation.contains("has_one") || possible_validation.contains("constraint")
            {
                let is_validation = prompt_check_validation(possible_validation.clone());
                if is_validation {
                    validations.push(possible_validation);
                }
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

    // replace in co file if no validations where found
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

    // from now on, check if is an acc validation or a prerequisite
    let mut account_validations: Vec<String> = vec![];
    let mut prerequisites: Vec<String> = vec![];
    for validation in validations.iter().map(|val| val.to_string()) {
        let validation_first_word = validation.split_whitespace().next().unwrap();
        // parse if validations differently
        if validation_first_word == "if" {
            // save the else lines
            let filtered_else = validation
                .lines()
                .filter(|line| line.contains("} else"))
                .collect::<Vec<_>>();
            let if_validation_formatted = validation.replace("else if", "else");
            let if_validation_tokenized = if_validation_formatted.split("else").collect::<Vec<_>>();
            let mut acc_validations: Vec<Vec<String>> = vec![vec![]; if_validation_tokenized.len()];
            let mut prereq_validations: Vec<Vec<String>> =
                vec![vec![]; if_validation_tokenized.len()];
            let closing_brace = validation.clone().lines().last().unwrap().to_string();
            for (tokenized_index, if_tokenized) in if_validation_tokenized.iter().enumerate() {
                for (val_index, val_line) in if_tokenized.clone().lines().enumerate() {
                    // if is the first line
                    if val_index == 0 {
                        // if is an if statement copy val_line
                        if tokenized_index == 0 {
                            acc_validations[tokenized_index].push(val_line.to_string().clone());
                            prereq_validations[tokenized_index].push(val_line.to_string().clone());
                            // else, is an else statement
                        } else {
                            acc_validations[tokenized_index]
                                .push(filtered_else[tokenized_index - 1].to_string());
                            prereq_validations[tokenized_index]
                                .push(filtered_else[tokenized_index - 1].to_string());
                        }
                    }
                    // if the line contains any of the validations and has a parenthesis
                    if validations_strings
                        .iter()
                        .any(|validation| val_line.contains(validation))
                        && val_line.contains('(')
                    {
                        // single line validation
                        let validation_string: String;
                        if val_line.contains(");") || val_line.contains(")?;") {
                            validation_string = get_single_line_validation(val_line.clone());
                        } else {
                            // multi line validation
                            validation_string = get_multiple_line_validation(
                                &if_tokenized.to_string().clone(),
                                val_index,
                            );
                        }
                        if !validation_string.is_empty() {
                            let selection = prompt_acc_val_or_prereq(validation_string.clone());
                            // 0 is acc validation
                            if selection == 0 {
                                // tokenized index == 0 means inside and if
                                acc_validations[tokenized_index]
                                    .push(validation_string.to_string());
                                // not 0 is prereq
                            } else {
                                prereq_validations[tokenized_index]
                                    .push(validation_string.to_string());
                                // tokenized index == 0 means inside and if
                            }
                        }
                    }
                }
            }
            // acc validations
            let mut acc_validations_vec: Vec<String> = vec![];
            if acc_validations.iter().any(|vec| vec.len() > 1) {
                for (acc_val_index, acc_val) in acc_validations.iter().enumerate() {
                    acc_validations_vec.append(&mut acc_val.clone());
                    // empty validation
                    // if acc_val.len() == 1 {
                    //     acc_validations_vec.push(format!(
                    //         "\n{}\n",
                    //         format!("{}", closing_brace.replace("}", "NO_ACCOUNT_VALIDATION")),
                    //     ));
                    // }
                    if acc_val_index == acc_validations.len() - 1 {
                        acc_validations_vec.push(closing_brace.clone());
                    }
                }
                account_validations.push("- ```rust".to_string());
                account_validations.push(acc_validations_vec.join("\n"));
                account_validations.push("   ```".to_string());
            }
            // prereq validations
            let mut prereq_validations_vec: Vec<String> = vec![];
            if prereq_validations.iter().any(|vec| vec.len() > 1) {
                for (prereq_val_index, prereq_val) in prereq_validations.iter().enumerate() {
                    prereq_validations_vec.append(&mut prereq_val.clone());
                    // empty validation
                    // if prereq_val.len() == 1 {
                    //     prereq_validations_vec.push(format!(
                    //         "\n{}\n",
                    //         format!("{}", closing_brace.replace("}", "NO_PREREQUISITE")),
                    //     ));
                    // }
                    if prereq_val_index == prereq_validations.len() - 1 {
                        prereq_validations_vec.push(closing_brace.clone());
                    }
                }
                prerequisites.push("- ```rust".to_string());
                prerequisites.push(prereq_validations_vec.join("\n"));
                prerequisites.push("   ```".to_string());
            }
        } else if validation.contains("#[account")
            && validation.lines().collect::<Vec<_>>().len() > 2
        {
            // check "#[account" type validations
            let validation_lines_amount = validation.lines().collect::<Vec<_>>().len();
            if validation_lines_amount > 1 {
                let mut acc_multline: Vec<String> = vec![];
                let mut prereq_multline: Vec<String> = vec![];
                for line in validation.lines() {
                    if line.contains("#[account") || line.contains("pub") || line.contains(")]") {
                        acc_multline.push(line.to_string());
                        prereq_multline.push(line.to_string());
                    } else {
                        let is_acc = prompt_acc_val_or_prereq(line.to_string()) == 0;
                        if is_acc {
                            acc_multline.push(line.to_string());
                        } else {
                            prereq_multline.push(line.to_string());
                        }
                    }
                }
                // if theres's more than 1 acc val
                if acc_multline.len() > 3 {
                    let acc_val = acc_multline.join("\n");
                    account_validations.push("- ```rust".to_string());
                    account_validations.push(acc_val);
                    account_validations.push("   ```".to_string());
                }
                // if theres's more than 1 prereq val
                if prereq_multline.len() > 3 {
                    let prereq_val = prereq_multline.join("\n");
                    prerequisites.push("- ```rust".to_string());
                    prerequisites.push(prereq_val);
                    prerequisites.push("   ```".to_string());
                }
            }
        } else {
            // single line validation
            let selection = prompt_acc_val_or_prereq(validation.clone());
            // 0 is acc validation
            if selection == 0 {
                account_validations.push("- ```rust".to_string());
                account_validations.push(validation.to_string());
                account_validations.push("   ```".to_string());
            } else {
                prerequisites.push("- ```rust".to_string());
                prerequisites.push(validation.to_string());
                prerequisites.push("   ```".to_string());
            }
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

fn prompt_acc_val_or_prereq(validation: String) -> usize {
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
    selection
}

pub fn parse_signers_into_co(co_file_path: String, context_lines: Vec<String>) {
    // signer names is only the name of the signer
    let signers_names = get_signers_description_from_co_file(&context_lines);
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

pub fn parse_function_parameters_into_co(
    co_file_path: String,
    co_file_name: String,
) -> Result<(), String> {
    let BatConfig { required, .. } = BatConfig::get_validated_config()?;
    let RequiredConfig {
        program_lib_path, ..
    } = required;

    let mut entrypoints_sonar = BatSonar::new_from_path(
        &program_lib_path,
        Some("#[program"),
        SonarResultType::Function,
    );
    let mut entrypoint = entrypoints_sonar
        .results
        .into_iter()
        .find(|function| function.name == co_file_name.replace(".md", ""))
        .unwrap();
    let parameters = get_function_parameters(entrypoint.content);
    // Filter context accounts
    let filtered_parameters: Vec<String> = parameters
        .into_iter()
        .filter(|parameter| !parameter.contains("Context<"))
        .collect();
    if filtered_parameters.is_empty() {
        let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
            CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
            ("- ".to_string() + CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER).as_str(),
        );
        fs::write(co_file_path, data).unwrap();
    } else {
        // join
        let data = fs::read_to_string(co_file_path.clone()).unwrap().replace(
            CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER,
            ("- ```rust\n  ".to_string() + filtered_parameters.join("\n  ").as_str() + "\n  ```")
                .as_str(),
        );
        fs::write(co_file_path, data).unwrap();
    }
    Ok(())
}

// unimplemented!();
// let to_review_file_string = fs::read_to_string(to_review_file_path.clone()).unwrap();
// // fs::write(
// //     to_review_file_path.clone(),
// //     to_review_file_string
// //         .replace(
// //             CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER,
// //             &instruction_file_path.replace("../", ""),
// //         )
// //         .as_str(),
// // )
// // .unwrap();
// let context_lines: Vec<String> = context_source_code
//     .get_source_code_content()
//     .lines()
//     .map(|line| line.to_string())
//     .collect();
//
// // parse text into co file
// batbelt::helpers::parse::parse_validations_into_co(
//     to_review_file_path.clone(),
//     instruction_file_path.clone(),
// );
// batbelt::helpers::parse::parse_context_accounts_into_co(
//     Path::new(&(to_review_file_path.clone()))
//         .canonicalize()
//         .unwrap(),
//     context_lines.clone(),
// );
//
// batbelt::helpers::parse::parse_signers_into_co(to_review_file_path.clone(), context_lines);
// batbelt::helpers::parse::parse_function_parameters_into_co(
//     to_review_file_path.clone(),
//     to_start_file_name.clone(),
// )?;
//
// println!("{to_start_file_name} file updated with instruction information");
//
// // create  co subfolder if user provided miro_oauth_access_token
// let miro_enabled = MiroConfig::new().miro_enabled();
// if miro_enabled {
//     // if miro enabled, then create a subfolder
//     // let started_folder_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
//     let started_folder_path =
//         batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, false);
//     let started_co_folder_path =
//         format!("{}/{}", started_folder_path, entrypoint_name.as_str());
//     let started_co_file_path = batbelt::path::get_file_path(
//         FilePathType::CodeOverhaulStarted {
//             file_name: entrypoint_name.clone(),
//         },
//         false,
//     );
//     // create the co subfolder
//     Command::new("mkdir")
//         .args([&started_co_folder_path])
//         .output()
//         .unwrap();
//     // move the co file inside the folder: mv
//     Command::new("mv")
//         .args([&to_review_file_path, &started_co_folder_path])
//         .output()
//         .unwrap();
//     println!("{to_start_file_name} file moved to started");
//     // create the screenshots empty images: entrypoint, handler, context accounts and validations
//     Command::new("touch")
//         .current_dir(&started_co_folder_path)
//         .args(CO_FIGURES)
//         .output()
//         .unwrap();
//     println!("Empty screenshots created, remember to complete them");
//
//     create_git_commit(
//         GitCommit::StartCOMiro,
//         Some(vec![to_start_file_name.to_string()]),
//     )?;
//
//     // open co file in VSCode
//     vs_code_open_file_in_current_window(started_co_file_path.as_str())?;
// } else {
//     // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(Some(
//     //     to_start_file_name.clone(),
//     // ))?;
//     let started_path = batbelt::path::get_file_path(
//         FilePathType::CodeOverhaulStarted {
//             file_name: to_start_file_name.clone(),
//         },
//         false,
//     );
//     Command::new("mv")
//         .args([to_review_file_path, started_path.clone()])
//         .output()
//         .unwrap();
//     println!("{to_start_file_name} file moved to started");
//
//     create_git_commit(
//         GitCommit::StartCO,
//         Some(vec![to_start_file_name.to_string()]),
//     )?;
//
//     // open co file in VSCode
//     vs_code_open_file_in_current_window(started_path.as_str())?;
// }

// #[test]
// fn test_get_ca_accounts() {
//     // let ca_accounts = vec![
//     //     SonarResult {
//     //         name: "NO_NAME".to_string(),
//     //         content: "    #[account(\n        mut,\n        seeds = [\n            CRAFTING_PROCESS.as_bytes(),\n            crafting_process.load()?.crafting_facility.as_ref(),\n            crafting_process.load()?.recipe.as_ref(),\n            &crafting_process.load()?.crafting_id.to_le_bytes(),\n        ],\n        bump = crafting_process.load()?.bump,\n        has_one = recipe @Errors::IncorrectRecipe,\n        has_one = authority @Errors::IncorrectAuthority,\n    )]\n    pub crafting_process: AccountLoader<'info, CraftingProcess>,".to_string(),
//     //         trailing_whitespaces: 4,
//     //         result_type: SonarResultType::ContextAccounts,
//     //         start_line_index: 6,
//     //         end_line_index: 18,
//     //         is_public: true,
//     //     },
//     //     SonarResult {
//     //         name: "NO_NAME".to_string(),
//     //         content: "    #[account(\n        mut,\n        constraint = token_from.mint == mint.key() @Errors::IncorrectMintAddress,\n        constraint = token_from.owner == crafting_process.key() @Errors::IncorrectAuthority,\n        constraint = token_from.delegated_amount > 0 @Errors::InsufficientAmount,\n    )]\n    pub token_from: Account<'info, TokenAccount>,".to_string(),
//     //         trailing_whitespaces: 4,
//     //         result_type: SonarResultType::ContextAccounts,
//     //         start_line_index: 24,
//     //         end_line_index: 30,
//     //         is_public: true,
//     //     },
//     //     SonarResult {
//     //         name: "NO_NAME".to_string(),
//     //         content: "    #[account(\n        mut,\n        constraint = token_to.mint == mint.key() @Errors::IncorrectMintAddress,\n    )]\n    pub token_to: Account<'info, TokenAccount>,".to_string(),
//     //         trailing_whitespaces: 4,
//     //         result_type: SonarResultType::ContextAccounts,
//     //         start_line_index: 33,
//     //         end_line_index: 37,
//     //         is_public: true,
//     //     },
//     //     SonarResult {
//     //         name: "NO_NAME".to_string(),
//     //         content: "    #[account(\n        mut,\n        constraint = token_from.mint == *mint.key @Errors::IncorrectMintAddress,\n    )]\n    pub mint: UncheckedAccount<'info>,".to_string(),
//     //         trailing_whitespaces: 4,
//     //         result_type: SonarResultType::ContextAccounts,
//     //         start_line_index: 41,
//     //         end_line_index: 45,
//     //         is_public: true,
//     //     },
//     // ];
//
//     let ca_content = "pub struct ClaimNonConsumableIngredient<'info> {
//     /// The owner/authority of crafting_process account
//     /// CHECK: Checked in constraints.
//     pub authority: UncheckedAccount<'info>,
//
//     /// The [`CraftingProcess`] account
//     #[account(
//         mut,
//         seeds = [
//             CRAFTING_PROCESS.as_bytes(),
//             crafting_process.load()?.crafting_facility.as_ref(),
//             crafting_process.load()?.recipe.as_ref(),
//             &crafting_process.load()?.crafting_id.to_le_bytes(),
//         ],
//         bump = crafting_process.load()?.bump,
//         has_one = recipe @Errors::IncorrectRecipe,
//         has_one = authority @Errors::IncorrectAuthority,
//     )]
//     pub crafting_process: AccountLoader<'info, CraftingProcess>,
//
//     /// The [`Recipe`] account
//     pub recipe: AccountLoader<'info, Recipe>,
//
//     /// The token account owned by the `crafting_process` which holds the ingredient in escrow
//     #[account(
//         mut,
//         constraint = token_from.mint == mint.key() @Errors::IncorrectMintAddress,
//         constraint = token_from.owner == crafting_process.key() @Errors::IncorrectAuthority,
//         constraint = token_from.delegated_amount > 0 @Errors::InsufficientAmount,
//     )]
//     pub token_from: Account<'info, TokenAccount>,
//
//     /// The token account to receive the non-consumable ingredient.
//     #[account(mut,constraint = token_to.mint == mint.key() @Errors::IncorrectMintAddress,)]
//     pub token_to: Account<'info, TokenAccount>,
//
//     /// The mint of the recipe ingredient
//     /// CHECK: checked in cargo program and constraints
//     #[account(
//         mut,
//         constraint = token_from.mint == *mint.key @Errors::IncorrectMintAddress,
//     )]
//     pub mint: UncheckedAccount<'info>,
//
//     /// The [Token] program
//     pub token_program: Program<'info, Token>,
// }";
//
//     let result = format_ca_accounts(ca_content);
// }
