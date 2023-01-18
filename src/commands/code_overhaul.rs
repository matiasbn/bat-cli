use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{MultiSelect, Select};

use crate::command_line::{canonicalize_path, vs_code_open_file_in_current_window};
use crate::commands::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::commands::helpers;
use crate::commands::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results,
};
use crate::commands::miro::api::connector::ConnectorOptions;
use crate::commands::miro::{self, MiroConfig};
use crate::config::BatConfig;
use crate::constants::{
    AUDIT_RESULT_FILE_NAME,
    CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER, CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER,
    CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER, CODE_OVERHAUL_HANDLER_PLACEHOLDER,
    CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER,
    CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER, CONTEXT_ACCOUNTS_PNG_NAME, CO_FIGURES,
    ENTRYPOINT_PNG_NAME, HANDLER_PNG_NAME, VALIDATIONS_PNG_NAME,
};

use std::fs;
use std::io::{BufRead, Result};
use std::path::Path;
use std::process::Command;
use std::string::String;

pub fn create_overhaul_file(entrypoint_name: String) -> Result<()> {
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

pub async fn start_code_overhaul_file() -> Result<()> {
    check_correct_branch();

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
    let instruction_file_path = Path::new(&instruction_file_path).canonicalize().unwrap();
    let context_lines =
        helpers::get::get_context_lines(instruction_file_path.clone(), to_start_file_name.clone())?;

    // open instruction file in VSCode
    vs_code_open_file_in_current_window(instruction_file_path.to_str().unwrap());

    // parse text into co file
    helpers::parse::context_accounts_into_co(
        Path::new(&(to_review_file_path.clone()))
            .canonicalize()
            .unwrap(),
        context_lines.clone(),
    );
    helpers::parse::validations_into_co(
        to_review_file_path.clone(),
        context_lines.clone(),
        instruction_file_path.to_str().unwrap().to_string(),
    );
    helpers::parse::signers_into_co(to_review_file_path.clone(), context_lines);
    helpers::parse::function_parameters_into_co(
        to_review_file_path.clone(),
        to_start_file_name.clone(),
    )?;

    println!("{to_start_file_name} file updated with instruction information");

    // create  co subfolder if user provided miro_oauth_access_token
    let miro_enabled = MiroConfig::new().miro_enabled();
    if miro_enabled {
        // if miro enabled, then create a subfolder
        let started_folder_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
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
        vs_code_open_file_in_current_window(started_co_file_path.as_str());
    } else {
        let started_path =
            BatConfig::get_auditor_code_overhaul_started_path(Some(to_start_file_name.clone()))?;
        Command::new("mv")
            .args([to_review_file_path, started_path.clone()])
            .output()
            .unwrap();
        println!("{to_start_file_name} file moved to started");

        create_git_commit(GitCommit::StartCO, Some(vec![to_start_file_name]))?;

        // open co file in VSCode
        vs_code_open_file_in_current_window(started_path.as_str());
    }
    Ok(())
}

pub async fn finish_code_overhaul_file() -> Result<()> {
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
                let started_folder_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
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
                let started_path = BatConfig::get_auditor_code_overhaul_started_path(Some(
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

pub fn update_code_overhaul_file() -> Result<()> {
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

pub fn count_co_files() -> Result<()> {
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

pub async fn deploy_miro() -> Result<()> {
    assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    // check empty images
    // get files and folders from started, filter .md files
    let started_folders: Vec<String> = helpers::get::get_started_entrypoints()?
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
    let selected_co_started_path = BatConfig::get_auditor_code_overhaul_started_path(None)?;
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

    // only create the frame if it was not created yet
    let to_start_file_content = fs::read_to_string(&started_co_file_path).unwrap();
    let is_deploying = to_start_file_content.contains(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER);
    if is_deploying {
        // check that the signers are finished
        let current_content = fs::read_to_string(&started_co_file_path).unwrap();
        if current_content.contains(CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER) {
            panic!("Please complete the signers description before deploying to Miro");
        }
        // get the signers name and description
        let signers_section_index = current_content
            .lines()
            .position(|line| line.contains("# Signers:"))
            .unwrap();
        let function_parameters_section_index = current_content
            .lines()
            .position(|line| line.contains("# Function parameters:"))
            .unwrap();
        let mut signers_description: Vec<String> = vec![];
        let current_content_lines: Vec<String> = current_content
            .lines()
            .map(|line| line.to_string())
            .collect();
        for idx in signers_section_index + 1..function_parameters_section_index - 1 {
            if !current_content_lines[idx].is_empty() {
                signers_description.push(current_content_lines[idx].clone());
            }
        }
        struct SignerInfo {
            signer_text: String,
            sticky_note_id: String,
            user_figure_id: String,
            validated_signer: bool,
        }
        let mut signers_info: Vec<SignerInfo> = vec![];
        for signer in signers_description.iter() {
            let signer_name = signer
                .split(":")
                .next()
                .unwrap()
                .replace("-", "")
                .trim()
                .to_string();
            let signer_description = signer.split(":").last().unwrap().trim().to_string();
            // prompt the user to select signer content
            let prompt_text = format!(
                "select the content of the signer {} sticky note in Miro",
                format!("{signer_name}").red()
            );
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .item(format!("Signer name: {}", signer_name.clone()))
                .item(format!(
                    "Signer description: {}",
                    signer_description.clone()
                ))
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            let signer_text = if selection == 0 {
                signer_name.clone()
            } else {
                signer_description.clone()
            };
            let prompt_text = format!(
                "is the signer {} a validated signer?",
                format!("{signer_name}").red()
            );
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(prompt_text)
                .item("yes")
                .item("no")
                .default(0)
                .interact_on_opt(&Term::stderr())
                .unwrap()
                .unwrap();
            let validated_signer = selection == 0;

            signers_info.push(SignerInfo {
                signer_text,
                sticky_note_id: "".to_string(),
                user_figure_id: "".to_string(),
                validated_signer,
            })
        }

        println!("Creating frame in Miro for {selected_folder}");
        let miro_frame = miro::api::frame::create_frame(selected_folder)
            .await
            .unwrap();
        fs::write(
            &started_co_file_path,
            &to_start_file_content
                .replace(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER, &miro_frame.url),
        )
        .unwrap();

        println!("Creating signers figures in Miro for {selected_folder}");
        for (signer_index, signer) in signers_info.iter_mut().enumerate() {
            // create the sticky note for every signer
            let sticky_note_id = miro::api::sticky_note::create_signer_sticky_note(
                signer.signer_text.clone(),
                signer_index,
                miro_frame.id.clone(),
                signer.validated_signer,
            )
            .await;
            let user_figure_id = miro::api::image::create_user_figure_for_signer(
                signer_index,
                miro_frame.id.clone(),
            )
            .await;
            *signer = SignerInfo {
                signer_text: signer.signer_text.clone(),
                sticky_note_id: sticky_note_id,
                user_figure_id: user_figure_id,
                validated_signer: signer.validated_signer,
            }
        }

        for screenshot in CO_FIGURES {
            // read the content after every placeholder replacement is essential
            let to_start_file_content = fs::read_to_string(&started_co_file_path).unwrap();
            let placeholder = match screenshot.to_string().as_str() {
                ENTRYPOINT_PNG_NAME => CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER,
                CONTEXT_ACCOUNTS_PNG_NAME => CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER,
                VALIDATIONS_PNG_NAME => CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER,
                HANDLER_PNG_NAME => CODE_OVERHAUL_HANDLER_PLACEHOLDER,
                _ => todo!(),
            };
            let screenshot_path =
                format!("{selected_co_started_path}/{selected_folder}/{screenshot}");
            println!("Creating image in Miro for {screenshot}");
            let id = miro::api::image::create_image_from_device(
                screenshot_path.to_string(),
                &selected_folder,
            )
            .await;
            fs::write(
                &started_co_file_path,
                &to_start_file_content.replace(placeholder, &id),
            )
            .unwrap();
        }
        // connect screenshots
        let entrypoint_id =
            helpers::get::get_screenshot_id(&ENTRYPOINT_PNG_NAME, &started_co_file_path);
        let context_accounts_id =
            helpers::get::get_screenshot_id(&CONTEXT_ACCOUNTS_PNG_NAME, &started_co_file_path);
        let validations_id =
            helpers::get::get_screenshot_id(&VALIDATIONS_PNG_NAME, &started_co_file_path);
        let handler_id = helpers::get::get_screenshot_id(&HANDLER_PNG_NAME, &started_co_file_path);
        println!("Connecting signers to entrypoint");
        for signer_miro_ids in signers_info {
            miro::api::connector::create_connector(
                &signer_miro_ids.user_figure_id,
                &signer_miro_ids.sticky_note_id,
                None,
            )
            .await;
            miro::api::connector::create_connector(
                &signer_miro_ids.sticky_note_id,
                &entrypoint_id,
                Some(ConnectorOptions {
                    start_x_position: "100%".to_string(),
                    start_y_position: "50%".to_string(),
                    end_x_position: "0%".to_string(),
                    end_y_position: "50%".to_string(),
                }),
            )
            .await;
        }
        println!("Connecting screenshots in Miro");
        miro::api::connector::create_connector(&entrypoint_id, &context_accounts_id, None).await;
        miro::api::connector::create_connector(&context_accounts_id, &validations_id, None).await;
        miro::api::connector::create_connector(&validations_id, &handler_id, None).await;
        create_git_commit(
            GitCommit::DeployMiro,
            Some(vec![selected_folder.to_string()]),
        )
    } else {
        // update images
        let prompt_text = format!("select the images to update for {selected_folder}");
        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .with_prompt(prompt_text)
            .items(CO_FIGURES)
            .defaults(&[true, true, true, true])
            .interact_on_opt(&Term::stderr())
            .unwrap()
            .unwrap();
        if !selections.is_empty() {
            for selection in selections.iter() {
                let screenshot_path_vec = &screenshot_paths.clone().collect::<Vec<_>>();
                let screenshot_path = &screenshot_path_vec.as_slice()[*selection];
                let file_name = screenshot_path.split('/').last().unwrap();
                println!("Updating: {file_name}");
                let item_id = helpers::get::get_screenshot_id(file_name, &started_co_file_path);
                miro::api::image::update_image_from_device(screenshot_path.to_string(), &item_id)
                    .await
            }
            create_git_commit(
                GitCommit::UpdateMiro,
                Some(vec![selected_folder.to_string()]),
            );
        } else {
            println!("No files selected");
        }
        Ok(())
    }
}

pub async fn open_co() -> Result<()> {
    // list to start
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
            BatConfig::get_auditor_code_overhaul_started_path(Some(
                started_entrypoints[index].clone(),
            ))?,
        ),
        None => panic!("User did not select anything"),
    };
    // select to start
    // get instruction
    let (_, instruction_file_path) =
        helpers::get::get_instruction_file_with_prompts(&started_file_name)?;

    println!(
        "Opening {} in VS Code",
        started_file_path.split("/").last().unwrap()
    );
    vs_code_open_file_in_current_window(&started_file_path);
    println!(
        "Opening {} in VS Code",
        instruction_file_path.split("/").last().unwrap()
    );
    vs_code_open_file_in_current_window(&instruction_file_path);
    Ok(())
}

pub fn update_audit_results() -> Result<()> {
    let audit_file_path =
        BatConfig::get_audit_folder_path(Some(AUDIT_RESULT_FILE_NAME.to_string()))?;
    let finished_co_files = get_finished_co_files()?;
    let finished_co_audit_information = get_finished_co_files_info_for_results(finished_co_files)?;
    let mut final_result: Vec<String> = vec!["# Code overhaul\n".to_string()];
    for result in finished_co_audit_information {
        let title = format!("## {}\n\n", result.file_name);
        let what_it_does_text = format!("### What it does:\n\n{}\n\n", result.what_it_does_content);
        let notes_text = format!("### Notes:\n\n{}\n\n", result.notes_content);
        let miro_frame_text = format!("### Miro frame url:\n\n{}\n", result.miro_frame_url);
        final_result.push([title, what_it_does_text, notes_text, miro_frame_text].join(""));
    }
    fs::write(
        audit_file_path,
        final_result
            .join("\n")
            .replace(CODE_OVERHAUL_NOTES_PLACEHOLDER, "No notes")
            .as_str(),
    )
    .unwrap();
    Ok(())
}
