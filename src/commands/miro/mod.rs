use crate::commands::metadata::FunctionsSubSection;
use crate::commands::metadata::MetadataSection;
use crate::config::*;
use crate::utils;
use crate::utils::git::*;
use normalize_url::normalizer;
use reqwest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::multipart::{self};
use serde_json::*;
use std::fs;
use std::result::Result;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};
pub mod api;
pub mod frame;
pub mod image;
pub mod shape;
use crate::constants::*;

pub struct MiroConfig {
    access_token: String,
    board_id: String,
    board_url: String,
}

impl MiroConfig {
    pub fn new() -> Self {
        let BatConfig {
            required, auditor, ..
        } = BatConfig::get_validated_config().unwrap();
        let access_token = auditor.miro_oauth_access_token;
        let board_id = required.miro_board_id;
        let board_url = required.miro_board_url;
        MiroConfig {
            access_token,
            board_id,
            board_url,
        }
    }

    pub fn miro_enabled(&self) -> bool {
        !self.access_token.is_empty()
    }

    pub fn get_frame_url(&self, frame_id: &str) -> String {
        let url = normalizer::UrlNormalizer::new(
            format!("{}/?moveToWidget={frame_id}", self.board_url).as_str(),
        )
        .unwrap()
        .normalize(None)
        .unwrap();
        url
    }
}

#[derive(Debug)]
pub enum MiroItemType {
    AppCard,
    Card,
    Document,
    Embed,
    Frame,
    Image,
    Shape,
    StickyNote,
    Text,
}

impl MiroItemType {
    fn str_item_type(&self) -> Result<String, String> {
        match self {
            MiroItemType::AppCard => Ok("app_card".to_string()),
            MiroItemType::Card => Ok("card".to_string()),
            MiroItemType::Document => Ok("document".to_string()),
            MiroItemType::Embed => Ok("embed".to_string()),
            MiroItemType::Frame => Ok("frame".to_string()),
            MiroItemType::Image => Ok("image".to_string()),
            MiroItemType::Shape => Ok("shape".to_string()),
            MiroItemType::StickyNote => Ok("sticky_note".to_string()),
            MiroItemType::Text => Ok("text".to_string()),
        }
    }
}

pub enum MiroColors {
    // Gray,
    // LightYellow,
    // Yellow,
    // Orange,
    // LightGreen,
    Green,
    DarkGreen,
    Cyan,
    LightPink,
    Pink,
    Violet,
    Red,
    LightBlue,
    Blue,
    DarkBlue,
    Black,
}

impl MiroColors {
    pub fn to_string(&self) -> &str {
        match self {
            // MiroStickyNoteColors::Gray => "gray",
            // MiroStickyNoteColors::LightYellow => "light_yellow",
            // MiroStickyNoteColors::Yellow => "yellow",
            // MiroStickyNoteColors::Orange => "orange",
            // MiroStickyNoteColors::LightGreen => "light_green",
            MiroColors::Green => "green",
            MiroColors::DarkGreen => "dark_green",
            MiroColors::Cyan => "cyan",
            MiroColors::LightPink => "light_pink",
            MiroColors::Pink => "pink",
            MiroColors::Violet => "violet",
            MiroColors::Red => "red",
            MiroColors::LightBlue => "light_blue",
            MiroColors::Blue => "blue",
            MiroColors::DarkBlue => "dark_blue",
            MiroColors::Black => "black",
        }
    }

    pub fn get_colors_vec() -> Vec<String> {
        vec![
            // "gray".to_string(),
            // "light_yellow".to_string(),
            // "yellow".to_string(),
            // "orange".to_string(),
            // "light_green".to_string(),
            "green".to_string(),
            "dark_green".to_string(),
            "cyan".to_string(),
            "light_pink".to_string(),
            "pink".to_string(),
            "violet".to_string(),
            "red".to_string(),
            "light_blue".to_string(),
            "blue".to_string(),
            "dark_blue".to_string(),
            "black".to_string(),
        ]
    }
}

use colored::Colorize;
use shape::{MiroShape, MiroShapeStyle};

use crate::{
    commands::{
        entrypoints::entrypoints::get_entrypoints_names, miro::api::connector::ConnectorOptions,
    },
    markdown::MarkdownFile,
    structs::{SignerInfo, SignerType},
    utils::{
        helpers::get::{
            get_string_between_two_index_from_string, get_string_between_two_str_from_string,
        },
        path::FilePathType,
    },
};

use super::metadata::source_code::SourceCodeMetadata;
use super::metadata::source_code::SourceCodeScreenshotOptions;

pub async fn deploy_miro() -> Result<(), String> {
    assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    // check empty images
    // get files and folders from started, filter .md files
    let (selected_folder, selected_co_started_path) = prompt_select_started_co_folder()?;
    let snapshot_paths = CO_FIGURES.iter().map(|figure| {
        format!(
            "{}",
            selected_co_started_path
                .clone()
                .replace(format!("{}.md", selected_folder).as_str(), figure)
        )
    });

    // check if some of the snapshots is empty
    for path in snapshot_paths.clone() {
        let snapshot_file = fs::read(&path).unwrap();
        let snapshot_name = path.split('/').clone().last().unwrap();
        if snapshot_file.is_empty() {
            panic!("{snapshot_name} snapshot file is empty, please complete it");
        }
    }

    // create the Miro frame
    // Replace placeholder with Miro url

    // only create the frame if it was not created yet
    let to_start_file_content = fs::read_to_string(&selected_co_started_path).unwrap();
    let is_deploying = to_start_file_content.contains(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER);
    if is_deploying {
        // check that the signers are finished
        let current_content = fs::read_to_string(selected_co_started_path.clone()).unwrap();
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
            // filter empty lines and No signers found
            if !current_content_lines[idx].is_empty()
                && !current_content_lines[idx].contains("No signers found")
            {
                signers_description.push(current_content_lines[idx].clone());
            }
        }

        let mut signers_info: Vec<SignerInfo> = vec![];
        if !signers_description.is_empty() {
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
                let selection = utils::cli_inputs::select(
                    &prompt_text,
                    vec![
                        format!("Signer name: {}", signer_name.clone()),
                        format!("Signer description: {}", signer_description.clone()),
                    ],
                    None,
                )?;
                let signer_text = if selection == 0 {
                    signer_name.clone()
                } else {
                    signer_description.clone()
                };
                let prompt_text = format!(
                    "is the signer {} a validated signer?",
                    format!("{signer_name}").red()
                );
                let selection = utils::cli_inputs::select_yes_or_no(&prompt_text)?;
                let signer_type = if selection {
                    SignerType::Validated
                } else {
                    SignerType::NotValidated
                };

                let signer_title = if selection {
                    format!("Validated signer:\n {}", signer_text)
                } else {
                    format!("Not validated signer:\n {}", signer_text)
                };

                signers_info.push(SignerInfo {
                    signer_text: signer_title,
                    sticky_note_id: "".to_string(),
                    user_figure_id: "".to_string(),
                    signer_type,
                })
            }
        } else {
            // no signers, push template signer
            signers_info.push(SignerInfo {
                signer_text: "Permissionless".to_string(),
                sticky_note_id: "".to_string(),
                user_figure_id: "".to_string(),
                signer_type: SignerType::NotSigner,
            })
        }

        println!("Creating frame in Miro for {selected_folder}");
        let miro_frame = api::frame::create_frame_for_entrypoint(&selected_folder)
            .await
            .unwrap();
        fs::write(
            &selected_co_started_path,
            &to_start_file_content
                .replace(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER, &miro_frame.url),
        )
        .unwrap();

        println!("Creating signers figures in Miro for {selected_folder}");

        for (signer_index, signer) in signers_info.iter_mut().enumerate() {
            // create the sticky note for every signer
            let sticky_note_id = api::sticky_note::create_signer_sticky_note(
                signer.signer_text.clone(),
                signer_index,
                miro_frame.id.clone(),
                signer.signer_type,
            )
            .await;
            let user_figure_id = api::custom_image::create_user_figure_for_signer(
                signer_index,
                miro_frame.id.clone(),
            )
            .await;
            *signer = SignerInfo {
                signer_text: signer.signer_text.clone(),
                sticky_note_id: sticky_note_id,
                user_figure_id: user_figure_id,
                signer_type: SignerType::NotSigner,
            }
        }

        for snapshot in CO_FIGURES {
            // read the content after every placeholder replacement is essential
            let to_start_file_content = fs::read_to_string(&selected_co_started_path).unwrap();
            let placeholder = match snapshot.to_string().as_str() {
                ENTRYPOINT_PNG_NAME => CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER,
                CONTEXT_ACCOUNTS_PNG_NAME => CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER,
                VALIDATIONS_PNG_NAME => CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER,
                HANDLER_PNG_NAME => CODE_OVERHAUL_HANDLER_PLACEHOLDER,
                _ => todo!(),
            };
            let snapshot_path = format!(
                "{}",
                selected_co_started_path
                    .clone()
                    .replace(format!("{}.md", selected_folder).as_str(), snapshot)
            );
            println!("Creating image in Miro for {snapshot}");
            let id = api::custom_image::create_image_from_device_and_update_position(
                snapshot_path.to_string(),
                &selected_folder,
            )
            .await?;
            fs::write(
                &selected_co_started_path,
                &to_start_file_content.replace(placeholder, &id),
            )
            .unwrap();
        }
        // connect snapshots
        let entrypoint_id =
            utils::helpers::get::get_screenshot_id(&ENTRYPOINT_PNG_NAME, &selected_co_started_path);
        let context_accounts_id = utils::helpers::get::get_screenshot_id(
            &CONTEXT_ACCOUNTS_PNG_NAME,
            &selected_co_started_path,
        );
        let validations_id = utils::helpers::get::get_screenshot_id(
            &VALIDATIONS_PNG_NAME,
            &selected_co_started_path,
        );
        let handler_id =
            utils::helpers::get::get_screenshot_id(&HANDLER_PNG_NAME, &selected_co_started_path);
        println!("Connecting signers to entrypoint");
        for signer_miro_ids in signers_info {
            api::connector::create_connector(
                &signer_miro_ids.user_figure_id,
                &signer_miro_ids.sticky_note_id,
                None,
            )
            .await;
            api::connector::create_connector(
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
        println!("Connecting snapshots in Miro");
        api::connector::create_connector(&entrypoint_id, &context_accounts_id, None).await;
        api::connector::create_connector(&context_accounts_id, &validations_id, None).await;
        api::connector::create_connector(&validations_id, &handler_id, None).await;
        create_git_commit(
            GitCommit::DeployMiro,
            Some(vec![selected_folder.to_string()]),
        )
    } else {
        // update images
        let prompt_text = format!("select the images to update for {selected_folder}");
        let selections = utils::cli_inputs::multiselect(
            &prompt_text,
            CO_FIGURES.to_vec(),
            Some(&vec![true, true, true, true]),
        )?;
        if !selections.is_empty() {
            for selection in selections.iter() {
                let snapshot_path_vec = &snapshot_paths.clone().collect::<Vec<_>>();
                let snapshot_path = &snapshot_path_vec.as_slice()[*selection];
                let file_name = snapshot_path.split('/').last().unwrap();
                println!("Updating: {file_name}");
                let item_id =
                    utils::helpers::get::get_screenshot_id(file_name, &selected_co_started_path);
                api::custom_image::update_image_from_device(snapshot_path.to_string(), &item_id)
                    .await
            }
            create_git_commit(
                GitCommit::UpdateMiro,
                Some(vec![selected_folder.to_string()]),
            )?;
        } else {
            println!("No files selected");
        }
        Ok(())
    }
}

fn prompt_select_started_co_folder() -> Result<(String, String), String> {
    let started_folders: Vec<String> = utils::helpers::get::get_started_entrypoints()?
        .iter()
        .filter(|file| !file.contains(".md"))
        .map(|file| file.to_string())
        .collect();
    if started_folders.is_empty() {
        panic!("No folders found in started folder for the auditor")
    }
    let prompt_text = "select the folder:".to_string();
    let selection = utils::cli_inputs::select(&prompt_text, started_folders.clone(), None)?;
    let selected_folder = &started_folders[selection];
    // let selected_co_started_file_path =
    //     utils::path::get_auditor_code_overhaul_started_file_path(Some(
    //         selected_folder.clone(),
    //     ))?;
    let selected_co_started_file_path = utils::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: selected_folder.clone(),
        },
        true,
    );
    Ok((
        selected_folder.clone(),
        selected_co_started_file_path.clone(),
    ))
}
pub fn create_co_snapshots() -> Result<(), String> {
    assert!(self::helpers::check_silicon_installed());
    let (selected_folder, selected_co_started_path) = prompt_select_started_co_folder()?;
    let co_file_string = fs::read_to_string(selected_co_started_path.clone()).expect(
        format!(
            "Error opening code-overhaul file at: {}",
            selected_co_started_path.clone()
        )
        .as_str(),
    );
    for figure in CO_FIGURES {
        println!("creating {} image for {}", figure, selected_folder);
        let (file_lines, snapshot_image_path, snapshot_markdown_path, index) =
            self::helpers::get_data_for_snapshots(
                co_file_string.clone(),
                selected_co_started_path.clone(),
                selected_folder.clone(),
                figure.to_string(),
            )?;
        self::helpers::create_co_figure(
            file_lines,
            snapshot_image_path,
            snapshot_markdown_path,
            index,
        );
    }
    //
    Ok(())
}

pub async fn deploy_accounts() -> Result<(), String> {
    let accounts_frame_id = self::helpers::get_accounts_frame_id().await?;
    println!("{}", accounts_frame_id);
    Ok(())
}

pub async fn deploy_entrypoints() -> Result<(), String> {
    // get entrypoints name
    let entrypoints_names = get_entrypoints_names()?;
    // get entrypoint miro frame url
    let prompt_text = format!("Please enter the {}", "entrypoints frame url".green());
    let entrypoints_frame_url = utils::cli_inputs::input(&prompt_text)?;
    let miro_frame_id = helpers::get_item_id_from_miro_url(&entrypoints_frame_url);

    for (entrypoint_name_index, entrypoint_name) in entrypoints_names.iter().enumerate() {
        // example
        let columns = 5;
        let initial_x_position = 372;
        let initial_y_position = 243;
        let entrypoint_width = 374;
        let entrypoint_height = 164;
        let x_offset = 40;
        let y_offset = 40;
        let x_position = initial_x_position
            + (x_offset + initial_x_position) * (entrypoint_name_index as i32 % columns);
        let y_position = initial_y_position
            + (y_offset + initial_y_position) * (entrypoint_name_index as i32 / columns);
        let miro_shape = MiroShape::new(
            x_position,
            y_position,
            entrypoint_width,
            entrypoint_height,
            entrypoint_name.to_string(),
        );
        let miro_shape_style = MiroShapeStyle::new_from_hex_border_color("#2d9bf0");
        miro_shape
            .create_shape_in_frame(miro_shape_style, &miro_frame_id)
            .await?;
    }
    Ok(())
}

pub async fn deploy_screenshot_to_frame() -> Result<(), String> {
    let prompt_text = format!("Please enter the {}", "destination frame url".green());
    let frame_url = utils::cli_inputs::input(&prompt_text)?;
    let frame_url =
        "https://miro.com/app/board/uXjVPvhKFIg=/?moveToWidget=3458764544674879470&cot=14";
    let miro_frame_id = helpers::get_item_id_from_miro_url(&frame_url);
    let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
    let metadata_markdown = MarkdownFile::new(&metadata_path);
    // Choose metadata section selection
    let metadata_sections_names: Vec<String> = metadata_markdown
        .clone()
        .sections
        .into_iter()
        .map(|section| section.title)
        .collect();
    let prompt_text = format!("Please enter the {}", "content type".green());
    let selection =
        utils::cli_inputs::select(&prompt_text, metadata_sections_names.clone(), None).unwrap();
    let section_selected = &metadata_sections_names[selection];
    let section = metadata_markdown
        .clone()
        .get_section_by_title(section_selected);
    // Choose metadata subsection selection
    let prompt_text = format!("Please enter the {}", "content sub type".green());
    let metadata_subsections_names: Vec<String> = section
        .subsections
        .clone()
        .into_iter()
        .map(|section| section.title)
        .collect();
    let selection =
        utils::cli_inputs::select(&prompt_text, metadata_subsections_names.clone(), None).unwrap();
    let subsection_selected = section.subsections[selection].clone();
    // Choose metadata final selection
    let prompt_text = format!("Please enter the {}", "content to deploy".green());
    let metadata_subsubsections_names: Vec<String> = subsection_selected
        .subsections
        .clone()
        .into_iter()
        .map(|section| section.title)
        .collect();
    let selection =
        utils::cli_inputs::select(&prompt_text, metadata_subsubsections_names.clone(), None)
            .unwrap();
    let subsubsection_selected = subsection_selected.subsections[selection].clone();
    let source_code_metadata = SourceCodeMetadata::new_from_metadata_data(
        &subsubsection_selected.title,
        &section.title,
        &subsection_selected.title,
    );
    let screnshot_options = SourceCodeScreenshotOptions {
        include_path: true,
        offset_to_start_line: false,
        filter_comments: true,
        filters: Some(vec!["#[account"]),
        font_size: Some(20),
        show_line_number: false,
    };
    let png_path = source_code_metadata.create_screenshot(screnshot_options);
    api::custom_image::create_image_from_device_and_update_position(
        png_path,
        &source_code_metadata.name,
    )
    .await
    .unwrap();
    Ok(())
}

pub mod helpers {
    use std::collections::HashMap;

    use reqwest::Url;

    use super::*;
    pub async fn get_accounts_frame_id() -> Result<String, String> {
        let response = api::item::get_items_on_board(Some(MiroItemType::Frame)).await?;
        let value: serde_json::Value =
            serde_json::from_str(&response.to_string()).expect("JSON was not well-formatted");
        let frames = value["data"].as_array().unwrap();
        let accounts_frame_id = frames
            .into_iter()
            .find(|f| f["data"]["title"] == "Accounts")
            .unwrap()["id"]
            .to_string();
        Ok(accounts_frame_id.clone().replace("\"", ""))
    }

    pub fn get_data_for_snapshots(
        co_file_string: String,
        selected_co_started_path: String,
        selected_folder_name: String,
        snapshot_name: String,
    ) -> Result<(String, String, String, Option<usize>), String> {
        if snapshot_name == CONTEXT_ACCOUNTS_PNG_NAME {
            let context_account_lines = get_string_between_two_str_from_string(
                co_file_string,
                "# Context Accounts:",
                "# Validations:",
            )?;
            let snapshot_image_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "context_accounts.png",
            );
            let snapshot_markdown_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "context_accounts.md",
            );
            Ok((
                context_account_lines
                    .replace("\n- ```rust", "")
                    .replace("\n  ```", ""),
                snapshot_image_path,
                snapshot_markdown_path,
                None,
            ))
        } else if snapshot_name == VALIDATIONS_PNG_NAME {
            let validation_lines = get_string_between_two_str_from_string(
                co_file_string,
                "# Validations:",
                "# Miro board frame:",
            )?;
            let snapshot_image_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "validations.png",
            );
            let snapshot_markdown_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "validations.md",
            );
            Ok((
                validation_lines,
                snapshot_image_path,
                snapshot_markdown_path,
                None,
            ))
        } else if snapshot_name == ENTRYPOINT_PNG_NAME {
            let RequiredConfig {
                program_lib_path, ..
            } = BatConfig::get_validated_config()?.required;
            let lib_file_string = fs::read_to_string(program_lib_path.clone()).unwrap();
            let start_entrypoint_index = lib_file_string
                .lines()
                .into_iter()
                .position(|f| f.contains("pub fn") && f.contains(&selected_folder_name))
                .unwrap();
            let end_entrypoint_index = lib_file_string
                .lines()
                .into_iter()
                .enumerate()
                .position(|(f_index, f)| f.trim() == "}" && f_index > start_entrypoint_index)
                .unwrap();
            let entrypoint_lines = get_string_between_two_index_from_string(
                lib_file_string,
                start_entrypoint_index,
                end_entrypoint_index,
            )?;
            let snapshot_image_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "entrypoint.png",
            );
            let snapshot_markdown_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "entrypoint.md",
            );
            Ok((
                format!(
                    "///{}\n\n{}",
                    program_lib_path.replace("../", ""),
                    entrypoint_lines,
                ),
                snapshot_image_path,
                snapshot_markdown_path,
                Some(start_entrypoint_index - 1),
            ))
        } else {
            //
            let (handler_string, instruction_file_path, start_index, _) =
                utils::helpers::get::get_instruction_handler_of_entrypoint(
                    selected_folder_name.clone(),
                )?;
            let snapshot_image_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name.clone()).as_str(),
                "handler.png",
            );
            let snapshot_markdown_path = selected_co_started_path.replace(
                format!("{}.md", selected_folder_name).as_str(),
                "handler.md",
            );
            // Handler
            Ok((
                format!("///{}\n\n{}", instruction_file_path, handler_string),
                snapshot_image_path,
                snapshot_markdown_path,
                Some(start_index - 1),
            ))
        }
    }

    pub fn create_co_figure(
        contents: String,
        image_path: String,
        temporary_markdown_path: String,
        index: Option<usize>,
    ) {
        // write the temporary markdown file
        fs::write(temporary_markdown_path.clone(), contents).unwrap();
        // take the snapshot
        if let Some(offset) = index {
            take_silicon_snapshot(image_path.clone(), temporary_markdown_path.clone(), offset);
        } else {
            take_silicon_snapshot(image_path.clone(), temporary_markdown_path.clone(), 1);
        }

        // delete the markdown
        delete_file(temporary_markdown_path);
    }

    pub fn take_silicon_snapshot<'a>(
        image_path: String,
        temporary_markdown_path: String,
        index: usize,
    ) {
        let offset = format!("{}", index);
        let image_file_name = image_path.split("/").last().unwrap();
        let mut args = vec![
            "--no-window-controls",
            "--language",
            "Rust",
            "--line-offset",
            &offset,
            "--theme",
            "Visual Studio Dark+",
            "--pad-horiz",
            "40",
            "--pad-vert",
            "40",
            "--background",
            "#d3d4d5",
            "--font",
            match image_file_name {
                ENTRYPOINT_PNG_NAME => "Hack=15",
                CONTEXT_ACCOUNTS_PNG_NAME => "Hack=15",
                VALIDATIONS_PNG_NAME => "Hack=14",
                HANDLER_PNG_NAME => "Hack=11",
                _ => "Hack=13",
            },
            "--output",
            &image_path,
            &temporary_markdown_path,
        ];
        if index == 1 {
            args.insert(0, "--no-line-number");
        }
        std::process::Command::new("silicon")
            .args(args)
            .output()
            .unwrap();
        // match output {
        //     Ok(_) => println!(""),
        //     Err(_) => false,
        // }
    }

    pub fn delete_file(path: String) {
        std::process::Command::new("rm")
            .args([path])
            .output()
            .unwrap();
    }

    pub fn check_silicon_installed() -> bool {
        let output = std::process::Command::new("silicon")
            .args(["--version"])
            .output();
        match output {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    pub fn get_item_id_from_miro_url(miro_url: &str) -> String {
        // example https://miro.com/app/board/uXjVP7aqTzc=/?moveToWidget=3458764541840480526&cot=14
        let frame_id = Url::parse(miro_url).unwrap();
        let hash_query: HashMap<_, _> = frame_id.query_pairs().into_owned().collect();
        hash_query.get("moveToWidget").unwrap().to_owned()
    }
}

#[test]

fn test_get_miro_item_id_from_url() {
    let miro_url =
        "https://miro.com/app/board/uXjVPvhKFIg=/?moveToWidget=3458764544363318703&cot=14";
    let item_id = helpers::get_item_id_from_miro_url(miro_url);
    println!("item id: {}", item_id);
    assert_eq!(item_id, "3458764541840480526".to_string())
}
