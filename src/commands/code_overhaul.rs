use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::command_line::vs_code_open_file_in_current_window;
use crate::config::{BatConfig, RequiredConfig};
use crate::constants::CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER;
use crate::git::{check_correct_branch, create_git_commit, GitCommit};

use std::borrow::BorrowMut;
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
        panic!(
            "code overhaul file already exists for: {:?}",
            entrypoint_name
        );
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
    println!("code-overhaul file created: {:?}.md", entrypoint_name);
}

pub fn start_code_overhaul_file() {
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
    let review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

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
    let started_file_name = match selection {
        // move selected file to rejected
        Some(index) => review_files[index].clone(),
        None => panic!("User did not select anything"),
    };

    let to_review_path =
        BatConfig::get_auditor_code_overhaul_to_review_path(Some(started_file_name.clone()));
    let started_path =
        BatConfig::get_auditor_code_overhaul_started_path(Some(started_file_name.clone()));
    check_correct_branch();
    // move to started
    Command::new("mv")
        .args([to_review_path, started_path.clone()])
        .output()
        .unwrap();
    println!("{} file moved to started", started_file_name);
    // update started co file
            println!(
                "{} file updated with instruction information",
                started_file_name
            );
            create_git_commit(GitCommit::StartCO, Some(vec![started_file_name.clone()]));
    
            let instructions_path = BatConfig::get_validated_config()
            .optional
            .program_instructions_path;
        let instructions_folder = fs::read_dir(instructions_path.clone()).unwrap();
        let instruction_files = instructions_folder
            .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
            .filter(|file| file != "mod.rs")
            .collect::<Vec<String>>();
        let entrypoint_name = started_file_name.clone().replace(".md", "");
        let instruction_match = instruction_files
            .iter()
            .filter(|ifile| ifile.replace(".rs", "") == entrypoint_name.as_str())
            .collect::<Vec<&String>>();
    
        // if instruction exists, prompt the user if the file is correct
        let is_match = if instruction_match.len() == 1 {
            let instruction_match_path =
                Path::new(&(instructions_path.clone() + "/" + &instruction_match[0].to_string()))
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
    
        let instruction_file_name = if is_match {
            instruction_match[0].to_string()
        } else {
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Select the instruction file: ")
                .items(&instruction_files)
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            instruction_files[selection].clone()
        };
        let instruction_file_path = Path::new(&(instructions_path + "/" + &instruction_file_name))
            .canonicalize()
            .unwrap();
        
        parse_context_accounts_into_co(instruction_file_path, Path::new(&(started_path)).canonicalize().unwrap() , started_file_name);


    // open VSCode files
    let instruction_file_path = BatConfig::get_path_to_instruction(instruction_file_name);
    vs_code_open_file_in_current_window(instruction_file_path);
    vs_code_open_file_in_current_window(started_path);
}

pub fn finish_code_overhaul_file() {
    println!("Select the code-overhaul file to finish:");
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
            Command::new("mv")
                .args([started_path, finished_path])
                .output()
                .unwrap();
            println!("{} file moved to finished", finished_file_name);
            create_git_commit(GitCommit::FinishCO, Some(vec![finished_file_name]));
        }
        None => println!("User did not select anything"),
    }
}

// fn create



fn parse_context_accounts_into_co(
    instruction_file_path: PathBuf,
    co_file_path: PathBuf,
    co_file_name: String,
) {
    let co_file = File::open(co_file_path.clone()).unwrap();
    let co_file_lines = io::BufReader::new(co_file)
        .lines()
        .map(|l| l.unwrap())
        .into_iter()
        .collect::<Vec<String>>();

    let context_lines = get_context_lines(instruction_file_path, co_file_name);
    let mut filtered_context_account_lines: Vec<_> = context_lines
        .iter()
        .filter(|line| !line.contains("constraint "))
        .map(|line| line.to_string())
        .collect();
    // replace context in the co file
    let context_co_index = co_file_lines
        .iter()
        .position(|l| l.contains(CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER))
        .unwrap();
    let mut co_lines_first_half: Vec<_> = co_file_lines[0..context_co_index].to_vec();
    let mut co_lines_second_half: Vec<_> =
        co_file_lines[context_co_index + 1..co_file_lines.len()].to_vec();
    let mut co_with_context_parsed = vec![];
    co_with_context_parsed.append(&mut co_lines_first_half);
    co_with_context_parsed.append(&mut filtered_context_account_lines);
    co_with_context_parsed.append(&mut co_lines_second_half);
    let mut co_with_context_parsed_string = co_with_context_parsed.join("\n");
    co_with_context_parsed_string =
        co_with_context_parsed_string.replace("    #[account(\n    )]\n", "");
    co_with_context_parsed_string = co_with_context_parsed_string.replace(
        "    #[account(\n        mut,\n    )]\n",
        "    #[account(mut)]\n",
    );
    co_with_context_parsed_string = co_with_context_parsed_string.replace(
        "    #[account(\n        zero,\n    )]\n",
        "    #[account(zero)]\n",
    );
    fs::write(co_file_path, co_with_context_parsed_string).unwrap();
}

fn parse_validations_into_co(
    instruction_file_path: PathBuf,
    co_file_path: PathBuf,
    co_file_name: String,
) {
    let co_file = File::open(co_file_path.clone()).unwrap();
    let co_file_lines = io::BufReader::new(co_file)
        .lines()
        .map(|l| l.unwrap())
        .into_iter()
        .collect::<Vec<String>>();

    let context_lines = get_context_lines(instruction_file_path, co_file_name);
    let mut filtered_context_account_lines: Vec<_> = context_lines
        .iter()
        .filter(|line| !line.contains("constraint "))
        .map(|line| line.to_string())
        .collect();
    // replace context in the co file
    let context_co_index = co_file_lines
        .iter()
        .position(|l| l.contains(CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER))
        .unwrap();
    let mut co_lines_first_half: Vec<_> = co_file_lines[0..context_co_index].to_vec();
    let mut co_lines_second_half: Vec<_> =
        co_file_lines[context_co_index + 1..co_file_lines.len()].to_vec();
    let mut co_with_context_parsed = vec![];
    co_with_context_parsed.append(&mut co_lines_first_half);
    co_with_context_parsed.append(&mut filtered_context_account_lines);
    co_with_context_parsed.append(&mut co_lines_second_half);
    let mut co_with_context_parsed_string = co_with_context_parsed.join("\n");
    co_with_context_parsed_string =
        co_with_context_parsed_string.replace("    #[account(\n    )]\n", "");
    co_with_context_parsed_string = co_with_context_parsed_string.replace(
        "    #[account(\n        mut,\n    )]\n",
        "    #[account(mut)]\n",
    );
    co_with_context_parsed_string = co_with_context_parsed_string.replace(
        "    #[account(\n        zero,\n    )]\n",
        "    #[account(zero)]\n",
    );

    fs::write(co_file_path, co_with_context_parsed_string).unwrap();
}

fn get_context_name(co_file_name: String) -> String {
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

    // if is not in the same line as the entrypoint name, is in the next line
    let entrypoint_index = program_lines
        .iter()
        .position(|line| line.contains((co_file_name.replace(".md", "") + "(").as_str()))
        .unwrap();
    let canditate_lines = vec![
        &program_lines[entrypoint_index],
        &program_lines[entrypoint_index + 1],
    ];

    let context_line = if canditate_lines[0].contains("Context<") {
        canditate_lines[0]
    } else {
        canditate_lines[1]
    };
    let parsed_context_name = context_line
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
    let instruction_file = File::open(instruction_file_path).unwrap();
    let instruction_file_lines = io::BufReader::new(instruction_file)
        .lines()
        .map(|l| l.unwrap())
        .into_iter()
        .collect::<Vec<String>>();

    let context_name = get_context_name(co_file_name);
    // get context lines
    let first_line_index = instruction_file_lines
        .iter()
        .position(|line| {
            line.contains(("pub struct ".to_string() + &context_name.clone()).as_str())
        })
        .unwrap();
    // the closing curly brace "}", starting on first_line_index
    let last_line_index = instruction_file_lines[first_line_index..]
        .iter()
        .position(|line| line == &"}")
        .unwrap()
        + first_line_index;
    let context_lines: Vec<_> = instruction_file_lines[first_line_index..=last_line_index].to_vec();
    context_lines
}
