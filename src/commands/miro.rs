use crate::batbelt;
use crate::batbelt::constants::*;
use crate::batbelt::git::*;
use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionLevel};

use std::fs;
use std::result::Result;

use colored::Colorize;

use crate::batbelt::markdown::MarkdownFile;

use crate::batbelt::structs::{SignerInfo, SignerType};
use crate::{
    batbelt::path::FilePathType, commands::entrypoints::entrypoints::get_entrypoints_names,
};

use crate::batbelt::metadata::source_code::SourceCodeMetadata;
use crate::batbelt::metadata::source_code::SourceCodeScreenshotOptions;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::miro::connector::ConnectorOptions;
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::image::{MiroImage, MiroImageType};
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::shape::{MiroShape, MiroShapeStyle};
use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::{helpers, MiroConfig, MiroItemType};

pub async fn deploy_co() -> Result<(), String> {
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

    // // check if some of the snapshots is empty
    // for path in snapshot_paths.clone() {
    //     let snapshot_file = fs::read(&path).unwrap();
    //     let snapshot_name = path.split('/').clone().last().unwrap();
    //     if snapshot_file.is_empty() {
    //         panic!("{snapshot_name} snapshot file is empty, please complete it");
    //     }
    // }

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
                let selection = batbelt::cli_inputs::select(
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
                let selection = batbelt::cli_inputs::select_yes_or_no(&prompt_text)?;
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

        // println!("Creating signers figures in Miro for {selected_folder}");
        //
        // for (signer_index, signer) in signers_info.iter_mut().enumerate() {
        //     let x_position = 550;
        //     let y_position = (150 + signer_index * 270) as i64;
        //     let width = 374;
        //     let mut signer_sticky_note = MiroStickyNote::new(
        //         &signer.signer_text,
        //         signer.signer_type.get_sticky_note_color(),
        //         &miro_frame.item_id,
        //         x_position,
        //         y_position,
        //         width,
        //     );
        //     signer_sticky_note.deploy().await;
        //
        //     let user_figure_url = "https://mirostatic.com/app/static/12079327f83ff492.svg";
        //     let y_position = (150 + signer_index * 270) as i64;
        //     let mut user_figure =
        //         MiroImage::new_from_url(user_figure_url, &miro_frame.item_id, 150, y_position, 200);
        //     user_figure.deploy().await;
        //
        //     *signer = SignerInfo {
        //         signer_text: signer.signer_text.clone(),
        //         sticky_note_id: signer_sticky_note.item_id,
        //         user_figure_id: user_figure.item_id,
        //         signer_type: SignerType::NotSigner,
        //     }
        // }
        //
        // for snapshot in CO_FIGURES {
        //     // read the content after every placeholder replacement is essential
        //     let to_start_file_content = fs::read_to_string(&selected_co_started_path).unwrap();
        //     let placeholder = match snapshot.to_string().as_str() {
        //         ENTRYPOINT_PNG_NAME => CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER,
        //         CONTEXT_ACCOUNTS_PNG_NAME => CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER,
        //         VALIDATIONS_PNG_NAME => CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER,
        //         HANDLER_PNG_NAME => CODE_OVERHAUL_HANDLER_PLACEHOLDER,
        //         _ => todo!(),
        //     };
        //     let snapshot_path = format!(
        //         "{}",
        //         selected_co_started_path
        //             .clone()
        //             .replace(format!("{}.md", selected_folder).as_str(), snapshot)
        //     );
        //     println!("Creating image in Miro for {snapshot}");
        //     let file_name = snapshot_path.clone().split('/').last().unwrap().to_string();
        //     // let id = api::custom_image::create_image_from_device_and_update_position(
        //     //     snapshot_path.to_string(),
        //     //     &selected_folder,
        //     // )
        //     // .await?;
        //     let mut snapshot = MiroImage::new_from_file_path(&snapshot_path);
        //     snapshot.deploy();
        //     let (x_position, y_position) = match file_name.to_string().as_str() {
        //         ENTRYPOINT_PNG_NAME => (1300, 250),
        //         CONTEXT_ACCOUNTS_PNG_NAME => (2200, 350),
        //         VALIDATIONS_PNG_NAME => (3000, 500),
        //         HANDLER_PNG_NAME => (2900, 1400),
        //         _ => todo!(),
        //     };
        //     snapshot.update_position(x_position, y_position);
        //     let _frame_id =
        //         batbelt::miro::helpers::get_frame_id_from_co_file(selected_folder.as_str())?;
        //     fs::write(
        //         &selected_co_started_path,
        //         &to_start_file_content.replace(placeholder, &snapshot.item_id),
        //     )
        //     .unwrap();
        // }
        // // connect snapshots
        // let entrypoint_id = batbelt::helpers::get::get_screenshot_id(
        //     &ENTRYPOINT_PNG_NAME,
        //     &selected_co_started_path,
        // );
        // let context_accounts_id = batbelt::helpers::get::get_screenshot_id(
        //     &CONTEXT_ACCOUNTS_PNG_NAME,
        //     &selected_co_started_path,
        // );
        // let validations_id = batbelt::helpers::get::get_screenshot_id(
        //     &VALIDATIONS_PNG_NAME,
        //     &selected_co_started_path,
        // );
        // let handler_id =
        //     batbelt::helpers::get::get_screenshot_id(&HANDLER_PNG_NAME, &selected_co_started_path);
        // println!("Connecting signers to entrypoint");
        // for signer_miro_ids in signers_info {
        //     batbelt::miro::connector::create_connector(
        //         &signer_miro_ids.user_figure_id,
        //         &signer_miro_ids.sticky_note_id,
        //         None,
        //     )
        //     .await;
        //     batbelt::miro::connector::create_connector(
        //         &signer_miro_ids.sticky_note_id,
        //         &entrypoint_id,
        //         Some(ConnectorOptions {
        //             start_x_position: "100%".to_string(),
        //             start_y_position: "50%".to_string(),
        //             end_x_position: "0%".to_string(),
        //             end_y_position: "50%".to_string(),
        //         }),
        //     )
        //     .await;
        // }
        // println!("Connecting snapshots in Miro");
        // batbelt::miro::connector::create_connector(&entrypoint_id, &context_accounts_id, None)
        //     .await;
        // batbelt::miro::connector::create_connector(&context_accounts_id, &validations_id, None)
        //     .await;
        // batbelt::miro::connector::create_connector(&validations_id, &handler_id, None).await;
        // create_git_commit(
        //     GitCommit::DeployMiro,
        //     Some(vec![selected_folder.to_string()]),
        // )
        Ok(())
    } else {
        // update images
        let prompt_text = format!("select the images to update for {selected_folder}");
        let selections = batbelt::cli_inputs::multiselect(
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
                    batbelt::helpers::get::get_screenshot_id(file_name, &selected_co_started_path);
                let mut screenshot_image =
                    MiroImage::new_from_item_id(&item_id, MiroImageType::FromPath).await;
                screenshot_image.update_from_path(&snapshot_path).await;
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
    let started_folders: Vec<String> = batbelt::helpers::get::get_started_entrypoints()?
        .iter()
        .filter(|file| !file.contains(".md"))
        .map(|file| file.to_string())
        .collect();
    if started_folders.is_empty() {
        panic!("No folders found in started folder for the auditor")
    }
    let prompt_text = "select the folder:".to_string();
    let selection = batbelt::cli_inputs::select(&prompt_text, started_folders.clone(), None)?;
    let selected_folder = &started_folders[selection];
    // let selected_co_started_file_path =
    //     utils::path::get_auditor_code_overhaul_started_file_path(Some(
    //         selected_folder.clone(),
    //     ))?;
    let selected_co_started_file_path = batbelt::path::get_file_path(
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
    let entrypoints_frame_url = batbelt::cli_inputs::input(&prompt_text)?;
    let miro_frame_id = batbelt::miro::helpers::get_item_id_from_miro_url(&entrypoints_frame_url);

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
    println!(
        "\n\nGetting the {} from the {} ...\n\n",
        "frames".yellow(),
        "Miro board".yellow()
    );
    let miro_frames: Vec<MiroFrame> = MiroFrame::get_frames_from_miro().await;
    let miro_frame_titles: Vec<&str> = miro_frames
        .iter()
        .map(|frame| frame.title.as_str())
        .map(|frame| frame.clone())
        .collect();
    let prompt_text = format!("Please select the destination {}", "Miro Frame".green());
    let selection = batbelt::cli_inputs::select(&prompt_text, miro_frame_titles, None).unwrap();
    let selected_miro_frame: &MiroFrame = &miro_frames[selection];
    let miro_frame_id = selected_miro_frame.item_id.clone();
    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, true);
    let metadata_markdown = MarkdownFile::new(&metadata_path);
    let mut continue_selection = true;
    while continue_selection {
        // Choose metadata section selection
        let metadata_sections_names: Vec<String> = metadata_markdown
            .sections
            .iter()
            .filter(|section| section.section_header.section_level == MarkdownSectionLevel::H1)
            .map(|section| section.section_header.title.clone())
            .collect();
        let prompt_text = format!("Please enter the {}", "content type".green());
        let selection =
            batbelt::cli_inputs::select(&prompt_text, metadata_sections_names.clone(), None)
                .unwrap();
        let section_selected = &metadata_sections_names[selection];
        let section: MarkdownSection = metadata_markdown.get_section(&section_selected).unwrap();
        // let structs_metadata_section_title = MetadataSection::Structs.to_string().as_str();
        match section.section_header.title.clone().as_str() {
            "Structs" => {
                // Choose metadata subsection selection
                let prompt_text = format!("Please enter the {}", "struct type to deploy".green());
                let struct_types_colorized = StructMetadataType::get_colorized_structs_type_vec();
                let selection =
                    batbelt::cli_inputs::select(&prompt_text, struct_types_colorized.clone(), None)
                        .unwrap();
                let selected_struct_type = StructMetadataType::get_structs_type_vec()[selection];
                let subsections = metadata_markdown.get_section_subsections(section.clone());
                let struct_metadata_vec = subsections
                    .iter()
                    .map(|section| StructMetadata::from_markdown_section(section.clone()))
                    .filter(|struct_metadata| struct_metadata.struct_type == selected_struct_type)
                    .collect::<Vec<_>>();
                let struct_metadata_names = struct_metadata_vec
                    .iter()
                    .map(|struct_metadata| struct_metadata.name.clone())
                    .collect::<Vec<_>>();
                let _selected_subsection = &subsections[selection];
                // Choose metadata final selection
                let prompt_text = format!("Please enter the {}", "struct to deploy".green());
                let selection =
                    batbelt::cli_inputs::select(&prompt_text, struct_metadata_names.clone(), None)
                        .unwrap();
                let selected_struct_metadata = &struct_metadata_vec[selection];
                let source_code_metadata = SourceCodeMetadata::new(
                    selected_struct_metadata.name.clone(),
                    selected_struct_metadata.path.clone(),
                    selected_struct_metadata.start_line_index,
                    selected_struct_metadata.end_line_index,
                );
                let include_path = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}",
                    "include the path?".yellow()
                ))
                .unwrap();
                let filter_comments = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}",
                    "filter the comments?".yellow()
                ))
                .unwrap();
                let show_line_number = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}",
                    "include the line numbers?".yellow()
                ))
                .unwrap();
                let offset_to_start_line = if show_line_number {
                    batbelt::cli_inputs::select_yes_or_no(&format!(
                        "Do you want to {}",
                        "offset to the starting line?".yellow()
                    ))
                    .unwrap()
                } else {
                    false
                };
                let include_filters = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}",
                    "add customized filters?".red()
                ))
                .unwrap();
                // utils::cli_inputs::select_yes_or_no("Do you want to include filters?").unwrap();
                let filters = if include_filters {
                    let filters_to_include = batbelt::cli_inputs::input(
                        "Please enter the filters, comma separated: #[account,CHECK ",
                    )
                    .unwrap();
                    if !filters_to_include.is_empty() {
                        let filters: Vec<String> = filters_to_include
                            .split(",")
                            .map(|filter| filter.trim().to_string())
                            .collect();
                        Some(filters)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let screenshot_options = SourceCodeScreenshotOptions {
                    include_path,
                    offset_to_start_line,
                    filter_comments,
                    show_line_number,
                    filters,
                    font_size: Some(20),
                };
                let png_path = source_code_metadata.create_screenshot(screenshot_options);
                println!(
                    "\nCreating {}{} in {} frame",
                    selected_struct_metadata.name.green(),
                    ".png".green(),
                    selected_miro_frame.title.green()
                );
                // let screenshot_image = image::api::create_image_from_device(&png_path)
                //     .await
                //     .unwrap();
                let mut screenshot_image = MiroImage::new_from_file_path(&png_path);
                screenshot_image.deploy().await;
                // let (x_position, y_position) = frame::api::get_frame_positon(&miro_frame_id).await;
                let miro_item = MiroItem::new(
                    &screenshot_image.item_id,
                    &miro_frame_id,
                    300,
                    selected_miro_frame.height as i64 - 300,
                    MiroItemType::Image,
                );

                println!(
                    "Updating the position of {}{}\n",
                    selected_struct_metadata.name.green(),
                    ".png".green()
                );
                miro_item.update_item_parent_and_position().await;

                // promp if continue

                let prompt_text = format!(
                    "Do you want to {} in the {} frame?",
                    "continue creating screenshots".yellow(),
                    selected_miro_frame.title.yellow()
                );
                continue_selection = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
            }
            _ => unimplemented!(),
        };
    }
    batbelt::git::create_git_commit(GitCommit::Figures, None).unwrap();
    Ok(())
}

#[test]

fn test_get_miro_item_id_from_url() {
    let miro_url =
        "https://miro.com/app/board/uXjVPvhKFIg=/?moveToWidget=3458764544363318703&cot=14";
    let item_id = batbelt::miro::helpers::get_item_id_from_miro_url(miro_url);
    println!("item id: {}", item_id);
    assert_eq!(item_id, "3458764541840480526".to_string())
}
