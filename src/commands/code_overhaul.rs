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

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::batbelt::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results,
    get_table_of_contents_for_results,
};
use crate::batbelt::path::{FilePathType, FolderPathType};

use std::fs;

use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::sonar::{get_function_parameters, BatSonar, SonarResultType};
use clap::builder::Str;
use std::path::Path;
use std::process::Command;
use std::string::String;

pub fn create_overhaul_file(entrypoint_name: String) -> Result<(), String> {
    let code_overhaul_auditor_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulToReview {
            file_name: entrypoint_name.clone(),
        },
        false,
    );
    if Path::new(&code_overhaul_auditor_file_path).is_file() {
        panic!("code overhaul file already exists for: {entrypoint_name:?}");
    }
    let mut co_template = batbelt::templates::markdown::MarkdownTemplate::CodeOverhaul
        .new(&code_overhaul_auditor_file_path);
    co_template.save()?;
    println!("code-overhaul file created: {entrypoint_name}.md");
    Ok(())
}

pub async fn start_code_overhaul_file() -> Result<(), String> {
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
            println!("params {:#?}", function_parameters);
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

    println!("ca accounts\n{:#?}", ca_accounts);
    unimplemented!();
    let to_review_file_string = fs::read_to_string(to_review_file_path.clone()).unwrap();
    // fs::write(
    //     to_review_file_path.clone(),
    //     to_review_file_string
    //         .replace(
    //             CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER,
    //             &instruction_file_path.replace("../", ""),
    //         )
    //         .as_str(),
    // )
    // .unwrap();
    let context_lines: Vec<String> = context_source_code
        .get_source_code_content()
        .lines()
        .map(|line| line.to_string())
        .collect();

    // open instruction file in VSCode
    vs_code_open_file_in_current_window(&instruction_file_path)?;

    // parse text into co file
    batbelt::helpers::parse::parse_validations_into_co(
        to_review_file_path.clone(),
        instruction_file_path.clone(),
    );
    batbelt::helpers::parse::parse_context_accounts_into_co(
        Path::new(&(to_review_file_path.clone()))
            .canonicalize()
            .unwrap(),
        context_lines.clone(),
    );

    batbelt::helpers::parse::parse_signers_into_co(to_review_file_path.clone(), context_lines);
    batbelt::helpers::parse::parse_function_parameters_into_co(
        to_review_file_path.clone(),
        to_start_file_name.clone(),
    )?;

    println!("{to_start_file_name} file updated with instruction information");

    // create  co subfolder if user provided miro_oauth_access_token
    let miro_enabled = MiroConfig::new().miro_enabled();
    if miro_enabled {
        // if miro enabled, then create a subfolder
        // let started_folder_path = utils::path::get_auditor_code_overhaul_started_file_path(None)?;
        let started_folder_path =
            batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, false);
        let started_co_folder_path =
            format!("{}/{}", started_folder_path, entrypoint_name.as_str());
        let started_co_file_path = batbelt::path::get_file_path(
            FilePathType::CodeOverhaulStarted {
                file_name: entrypoint_name.clone(),
            },
            false,
        );
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
            Some(vec![to_start_file_name.to_string()]),
        )?;

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_co_file_path.as_str())?;
    } else {
        // let started_path = utils::path::get_auditor_code_overhaul_started_file_path(Some(
        //     to_start_file_name.clone(),
        // ))?;
        let started_path = batbelt::path::get_file_path(
            FilePathType::CodeOverhaulStarted {
                file_name: to_start_file_name.clone(),
            },
            false,
        );
        Command::new("mv")
            .args([to_review_file_path, started_path.clone()])
            .output()
            .unwrap();
        println!("{to_start_file_name} file moved to started");

        create_git_commit(
            GitCommit::StartCO,
            Some(vec![to_start_file_name.to_string()]),
        )?;

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_path.as_str())?;
    }
    Ok(())
}

pub async fn finish_code_overhaul_file() -> Result<(), String> {
    check_correct_branch()?;
    // get to-review files
    let started_endpoints = batbelt::helpers::get::get_started_entrypoints()?;
    let prompt_text = "Select the code-overhaul to finish:";
    let selection = batbelt::cli_inputs::select(prompt_text, started_endpoints.clone(), None)?;

    let finished_endpoint = &started_endpoints[selection].clone();
    let finished_co_folder_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true);
    let started_co_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        },
        true,
    );
    batbelt::helpers::check::code_overhaul_file_completed(
        started_co_file_path.clone(),
        finished_endpoint.clone(),
    );

    Command::new("mv")
        .args([started_co_file_path, finished_co_folder_path])
        .output()
        .unwrap();

    if MiroConfig::new().miro_enabled() {
        let (_, _, finished_co) = batbelt::helpers::count::co_counter()?;
        let frame_id =
            batbelt::miro::helpers::get_frame_id_from_co_file(finished_endpoint.as_str())?;
        let mut frame = MiroFrame::new_from_item_id(&frame_id).await;
        let x_modifier = finished_co as i64 % MIRO_BOARD_COLUMNS;
        let y_modifier = finished_co as i64 / MIRO_BOARD_COLUMNS;
        let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 100) * x_modifier;
        let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 100) * y_modifier;
        frame.update_position(x_position, y_position).await?;
        let started_co_folder_path =
            batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, true);
        let started_co_subfolder_path = format!("{}/{}", started_co_folder_path, finished_endpoint);

        // remove co subfolder
        Command::new("rm")
            .args(["-rf", &started_co_subfolder_path])
            .output()
            .unwrap();

        create_git_commit(
            GitCommit::FinishCOMiro,
            Some(vec![finished_endpoint.to_string()]),
        )?;
    } else {
        create_git_commit(
            GitCommit::FinishCO,
            Some(vec![finished_endpoint.to_string()]),
        )?;
    }

    println!("{} moved to finished", finished_endpoint.green());
    Ok(())
}

pub fn update_code_overhaul_file() -> Result<(), String> {
    println!("Select the code-overhaul file to finish:");
    // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
    let finished_path = batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true);
    // get to-review files
    let finished_files = fs::read_dir(finished_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if finished_files.is_empty() {
        panic!("{}", "no finished files in code-overhaul folder".red());
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

pub async fn open_co() -> Result<(), String> {
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
