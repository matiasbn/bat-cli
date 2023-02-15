use crate::batbelt;
use crate::batbelt::constants::*;
use crate::batbelt::git::*;
use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionLevel};

use std::fs;
use std::result::Result;

use colored::Colorize;
use toml::map::Entry;

use crate::batbelt::markdown::MarkdownFile;

use crate::batbelt::entrypoint::Entrypoint;
use crate::batbelt::helpers::get::{get_context_name, get_only_files_from_folder};
use crate::batbelt::metadata::entrypoint::EntrypointMetadata;
use crate::batbelt::metadata::functions::{FunctionMetadata, FunctionMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::structs::{SignerInfo, SignerType};
use crate::{
    batbelt::path::FilePathType, commands::entrypoints::entrypoints::get_entrypoints_names,
};

use crate::batbelt::metadata::source_code::SourceCodeMetadata;
use crate::batbelt::metadata::source_code::SourceCodeScreenshotOptions;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::miro::connector::{create_connector, ConnectorOptions};
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::image::MiroImage;

use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::shape::{MiroShape, MiroShapeStyle};
use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::{helpers, MiroConfig};
use crate::batbelt::path::FolderPathType;
use crate::batbelt::templates::code_overhaul::{
    CodeOverhaulSection, CoderOverhaulTemplatePlaceholders,
};

pub async fn deploy_co() -> Result<(), String> {
    assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    // check empty images
    // get files and folders from started, filter .md files
    // let (selected_folder, selected_co_started_path) = prompt_select_started_co_folder()?;
    let started_path = batbelt::path::get_folder_path(FolderPathType::CodeOverhaulStarted, false);
    let started_files_file_info = get_only_files_from_folder(started_path).unwrap();
    let file_names = started_files_file_info
        .iter()
        .map(|file_info| file_info.name.clone())
        .collect::<Vec<_>>();
    let prompt_text = "Select the co file to deploy to Miro";
    let selection = batbelt::cli_inputs::select(&prompt_text, file_names, None).unwrap();
    let selected_file_info = &started_files_file_info[selection];
    let entrypoint_name = selected_file_info.name.trim_end_matches(".md");
    let selected_co_started_path = selected_file_info.path.clone();
    let miro_frames = MiroFrame::get_frames_from_miro().await;
    let entrypoint_frame = miro_frames
        .iter()
        .find(|frame| frame.title == entrypoint_name);
    let entrypoint_frame = if let Some(ep_frame) = entrypoint_frame {
        ep_frame
    } else {
        unimplemented!()
    };
    let entrypoint_frame_objects = entrypoint_frame.get_items_within_frame().await;

    let is_deploying = entrypoint_frame_objects.is_empty();
    if is_deploying {
        // check that the signers are finished
        let current_content = fs::read_to_string(selected_co_started_path.clone()).unwrap();
        if current_content.contains(
            &CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription.to_placeholder(),
        ) {
            panic!("Please complete the signers description before deploying to Miro");
        }
        let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, false);
        let metadata_markdown = MarkdownFile::new(&metadata_path);
        let entrypoints_section = metadata_markdown
            .get_section(&MetadataSection::Entrypoints.to_sentence_case())
            .unwrap();
        let started_entrypoint_section =
            metadata_markdown.get_subsection(&entrypoint_name, entrypoints_section.section_header);
        let EntrypointMetadata {
            name: _,
            signers,
            instruction_file_path,
            handler_function,
            context_name: _,
            mut_accounts: _,
            function_parameters: _,
        } = EntrypointMetadata::from_markdown_section(started_entrypoint_section);
        // get the signers name and description

        let mut signers_info: Vec<SignerInfo> = vec![];
        if !signers.is_empty() {
            for signer_name in signers.iter() {
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
                    format!("Validated signer:\n\n {}", signer_name)
                } else {
                    format!("Not validated signer:\n\n {}", signer_name)
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

        println!(
            "Creating signers figures in Miro for {}",
            entrypoint_name.green()
        );

        for (signer_index, signer) in signers_info.iter_mut().enumerate() {
            let x_position = 550;
            let y_position = (150 + signer_index * 270) as i64;
            let width = 374;
            let mut signer_sticky_note = MiroStickyNote::new(
                &signer.signer_text,
                signer.signer_type.get_sticky_note_color(),
                &entrypoint_frame.item_id,
                x_position,
                y_position,
                width,
            );
            signer_sticky_note.deploy().await;

            let user_figure_url = "https://mirostatic.com/app/static/12079327f83ff492.svg";
            let y_position = (150 + signer_index * 270) as i64;
            let mut user_figure = MiroImage::new_from_url(
                user_figure_url,
                &entrypoint_frame.item_id,
                150,
                y_position,
                200,
            );
            user_figure.deploy().await;

            *signer = SignerInfo {
                signer_text: signer.signer_text.clone(),
                sticky_note_id: signer_sticky_note.item_id,
                user_figure_id: user_figure.item_id,
                signer_type: SignerType::NotSigner,
            }
        }
        // Handler figure
        let functions_section = metadata_markdown
            .get_section(&MetadataSection::Functions.to_sentence_case())
            .unwrap();
        let functions_subsections =
            metadata_markdown.get_section_subsections(functions_section.clone());
        let handler_subsection = functions_subsections
            .iter()
            .find(|subsection| {
                subsection.section_header.title == handler_function
                    && subsection.content.contains(&instruction_file_path)
            })
            .unwrap();
        let handler_function_metadata =
            FunctionMetadata::from_markdown_section(handler_subsection.clone());
        let handler_source_code = SourceCodeMetadata::new(
            handler_function,
            handler_function_metadata.path,
            handler_function_metadata.start_line_index,
            handler_function_metadata.end_line_index,
        );
        let entrypoint_metadata = functions_subsections
            .iter()
            .find_map(|function| {
                let function_metadata =
                    FunctionMetadata::from_markdown_section(function.clone().clone());
                if function_metadata.function_type == FunctionMetadataType::EntryPoint
                    && function_metadata.name == entrypoint_name
                {
                    Some(function_metadata)
                } else {
                    None
                }
            })
            .unwrap();
        let entrypoint_source_code = SourceCodeMetadata::new(
            entrypoint_metadata.name,
            entrypoint_metadata.path,
            entrypoint_metadata.start_line_index,
            entrypoint_metadata.end_line_index,
        );
        // Context accounts figure
        let co_file_markdown = MarkdownFile::new(&selected_co_started_path);
        let context_accounts_section = co_file_markdown
            .get_section(&CodeOverhaulSection::ContextAccounts.to_title())
            .unwrap();
        let context_accounts_source_code = SourceCodeMetadata::new(
            CodeOverhaulSection::ContextAccounts.to_title(),
            selected_co_started_path.clone(),
            context_accounts_section.start_line_index,
            context_accounts_section.end_line_index,
        );
        // Validations figure
        let validations_section = co_file_markdown
            .get_section(&CodeOverhaulSection::Validations.to_title())
            .unwrap();

        let validations_accounts_source_code = SourceCodeMetadata::new(
            CodeOverhaulSection::Validations.to_title(),
            selected_co_started_path.clone(),
            validations_section.start_line_index,
            validations_section.end_line_index,
        );
        let options = SourceCodeScreenshotOptions {
            include_path: true,
            offset_to_start_line: true,
            filter_comments: true,
            font_size: Some(20),
            filters: None,
            show_line_number: true,
        };
        let co_options = SourceCodeScreenshotOptions {
            include_path: false,
            offset_to_start_line: false,
            filter_comments: false,
            font_size: Some(20),
            filters: None,
            show_line_number: false,
        };
        let handler_screenshot_path = handler_source_code.create_screenshot(options.clone());
        let entrypoint_screenshot_path = entrypoint_source_code.create_screenshot(options.clone());
        let co_screenshot_path = context_accounts_source_code.create_screenshot(co_options.clone());
        let validations_screenshot_path =
            validations_accounts_source_code.create_screenshot(co_options.clone());

        // Miro Images&
        let mut handler_miro_image =
            MiroImage::new_from_file_path(&handler_screenshot_path, &entrypoint_frame.item_id);
        let mut entrypoint_miro_image =
            MiroImage::new_from_file_path(&entrypoint_screenshot_path, &entrypoint_frame.item_id);
        let mut co_miro_image =
            MiroImage::new_from_file_path(&co_screenshot_path, &entrypoint_frame.item_id);
        let mut validations_miro_image =
            MiroImage::new_from_file_path(&validations_screenshot_path, &entrypoint_frame.item_id);

        handler_miro_image.deploy().await;
        entrypoint_miro_image.deploy().await;
        co_miro_image.deploy().await;
        validations_miro_image.deploy().await;

        entrypoint_miro_image.update_position(1300, 250).await;
        co_miro_image.update_position(2200, 350).await;
        validations_miro_image.update_position(3000, 500).await;
        handler_miro_image.update_position(2900, 1400).await;

        println!("Connecting signers to entrypoint");
        for signer_miro_ids in signers_info {
            batbelt::miro::connector::create_connector(
                &signer_miro_ids.user_figure_id,
                &signer_miro_ids.sticky_note_id,
                None,
            )
            .await;
            batbelt::miro::connector::create_connector(
                &signer_miro_ids.sticky_note_id,
                &entrypoint_miro_image.item_id,
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
        batbelt::miro::connector::create_connector(
            &entrypoint_miro_image.item_id,
            &co_miro_image.item_id,
            None,
        )
        .await;
        batbelt::miro::connector::create_connector(
            &co_miro_image.item_id,
            &validations_miro_image.item_id,
            None,
        )
        .await;
        batbelt::miro::connector::create_connector(
            &validations_miro_image.item_id,
            &handler_miro_image.item_id,
            None,
        )
        .await;

        // // Deploy mut_accounts
        // if mut_accounts.len() > 0 {
        //     let structs_section = metadata_markdown
        //         .get_section(&MetadataSection::Structs.to_sentence_case())
        //         .unwrap();
        //     let structs_subsection = metadata_markdown.get_section_subsections(structs_section);
        //     for mut_account in mut_accounts {
        //         let mut_account_section = structs_subsection.iter().find_map(|subsection| {
        //             let struct_md_section =
        //                 StructMetadata::from_markdown_section(subsection.clone());
        //             if struct_md_section.struct_type == StructMetadataType::SolanaAccount
        //                 && struct_md_section.name == mut_account[1]
        //             {
        //                 Some(struct_md_section)
        //             } else {
        //                 None
        //             }
        //         });
        //         if let Some(mut_section) = mut_account_section {
        //             let mut_acc_source_code = SourceCodeMetadata::new(
        //                 CodeOverhaulSection::Validations.to_title(),
        //                 mut_section.path.clone(),
        //                 mut_section.start_line_index,
        //                 mut_section.end_line_index,
        //             );
        //             let mut_acc_screenshot_path =
        //                 mut_acc_source_code.create_screenshot(options.clone());
        //             let mut mut_acc_miro_image = MiroImage::new_from_file_path(
        //                 &mut_acc_screenshot_path,
        //                 &entrypoint_frame.item_id,
        //             );
        //             mut_acc_miro_image.deploy().await;
        //             mut_acc_miro_image.update_position(400, 400).await;
        //             // fs::remove_file(mut_acc_screenshot_path).unwrap();
        //         }
        //     }
        // }
        // Remove screenshots
        fs::remove_file(handler_screenshot_path).unwrap();
        fs::remove_file(co_screenshot_path).unwrap();
        fs::remove_file(validations_screenshot_path).unwrap();
        fs::remove_file(entrypoint_screenshot_path).unwrap();

        //
        // create_git_commit(
        //     GitCommit::DeployMiro,
        //     Some(vec![selected_co_started_path.to_string()]),
        // )
        // .unwrap();
        Ok(())
    } else {
        // update images
        // let prompt_text = format!("select the images to update for {selected_folder}");
        // let selections = batbelt::cli_inputs::multiselect(
        //     &prompt_text,
        //     CO_FIGURES.to_vec(),
        //     Some(&vec![true, true, true, true]),
        // )?;
        // if !selections.is_empty() {
        //     for selection in selections.iter() {
        //         let snapshot_path_vec = &snapshot_paths.clone().collect::<Vec<_>>();
        //         let snapshot_path = &snapshot_path_vec.as_slice()[*selection];
        //         let file_name = snapshot_path.split('/').last().unwrap();
        //         println!("Updating: {file_name}");
        //         let item_id =
        //             batbelt::helpers::get::get_screenshot_id(file_name, &selected_co_started_path);
        //         let mut screenshot_image =
        //             MiroImage::new_from_item_id(&item_id, MiroImageType::FromPath).await;
        //         screenshot_image.update_from_path(&snapshot_path).await;
        //     }
        //     create_git_commit(
        //         GitCommit::UpdateMiro,
        //         Some(vec![selected_folder.to_string()]),
        //     )?;
        // } else {
        //     println!("No files selected");
        // }
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

pub async fn deploy_entrypoint_screenshots_to_frame(
    new_frame: bool,
    select_all: bool,
    sorted: bool,
) -> Result<(), String> {
    // get entrypoints name
    let entrypoints_names = Entrypoint::get_entrypoints_names(sorted)?;

    // prompt the user to select an entrypoint
    let prompt_text = "Please select the entrypoints to deploy";
    let selected_entrypoints_index = batbelt::cli_inputs::multiselect(
        prompt_text,
        entrypoints_names.clone(),
        Some(&vec![select_all; entrypoints_names.clone().len()]),
    )
    .unwrap();
    if new_frame {
        unimplemented!()
    }
    // prompt to select the frame
    let mut miro_frames = MiroFrame::get_frames_from_miro().await;
    miro_frames.sort_by(|frame_a, frame_b| frame_a.title.cmp(&frame_b.title));
    let mut miro_frames_names = miro_frames
        .iter()
        .map(|frame| frame.title.to_string())
        .collect::<Vec<_>>();
    let prompt_text = "Please select the Miro frame to deploy";
    let selected_frame_index =
        batbelt::cli_inputs::select(prompt_text, miro_frames_names, None).unwrap();
    let selected_frame = &miro_frames[selected_frame_index];
    // let structs_subsections = StructMetadata::get_structs_from_metadata_file();
    let entrypoint_sc_options = SourceCodeScreenshotOptions {
        include_path: false,
        offset_to_start_line: true,
        filter_comments: true,
        font_size: None,
        filters: None,
        show_line_number: true,
    };
    let context_accounts_sc_options = SourceCodeScreenshotOptions {
        include_path: false,
        offset_to_start_line: false,
        filter_comments: true,
        font_size: None,
        filters: None,
        show_line_number: false,
    };

    let handler_sc_options = SourceCodeScreenshotOptions {
        include_path: true,
        offset_to_start_line: true,
        filter_comments: true,
        font_size: None,
        filters: None,
        show_line_number: true,
    };
    for (index, selected_ep_index) in selected_entrypoints_index.iter().enumerate() {
        let selected_entrypoint = &entrypoints_names[selected_ep_index.clone()];
        // get context_accounts name
        let entrypoint = Entrypoint::new_from_name(selected_entrypoint.as_str());
        let ep_source_code = entrypoint.entrypoint_function.get_source_code();
        let ca_source_code = entrypoint.context_accounts.get_source_code();
        let handler_source_code = entrypoint.handler.get_source_code();
        let ep_id = ep_source_code
            .deploy_screenshot_to_miro_frame(
                selected_frame.clone(),
                (selected_frame.clone().width / 2) as i64,
                (selected_frame.clone().height * 2 / 6) as i64,
                // 0,
                entrypoint_sc_options.clone(),
            )
            .await;
        let ca_id = ca_source_code
            .deploy_screenshot_to_miro_frame(
                selected_frame.clone(),
                (selected_frame.clone().width / 2) as i64,
                (selected_frame.clone().height * 3 / 6) as i64,
                // 0,
                context_accounts_sc_options.clone(),
            )
            .await;
        let handler_id = handler_source_code
            .deploy_screenshot_to_miro_frame(
                selected_frame.clone(),
                (selected_frame.clone().width / 2) as i64,
                (selected_frame.clone().height * 4 / 6) as i64,
                // 0,
                handler_sc_options.clone(),
            )
            .await;
        create_connector(&ep_id, &ca_id, None).await;
        create_connector(&ca_id, &handler_id, None).await;
        if index < selected_entrypoints_index.len() - 1 {
            let user_decided_to_continue =
                batbelt::cli_inputs::select_yes_or_no("Do you want to continue deploying?")
                    .unwrap();
            if !user_decided_to_continue {
                break;
            }
        }
    }
    // // get entrypoint miro frame url
    // let prompt_text = format!("Please enter the {}", "entrypoints frame url".green());
    // let entrypoints_frame_url = batbelt::cli_inputs::input(&prompt_text)?;
    // let miro_frame_id = batbelt::miro::helpers::get_item_id_from_miro_url(&entrypoints_frame_url);

    // for (entrypoint_name_index, entrypoint_name) in entrypoints_names.iter().enumerate() {
    //     // example
    //     let columns = 5;
    //     let initial_x_position = 372;
    //     let initial_y_position = 243;
    //     let entrypoint_width = 374;
    //     let entrypoint_height = 164;
    //     let x_offset = 40;
    //     let y_offset = 40;
    //     let x_position = initial_x_position
    //         + (x_offset + initial_x_position) * (entrypoint_name_index as i32 % columns);
    //     let y_position = initial_y_position
    //         + (y_offset + initial_y_position) * (entrypoint_name_index as i32 / columns);
    //     let miro_shape = MiroShape::new(
    //         x_position,
    //         y_position,
    //         entrypoint_width,
    //         entrypoint_height,
    //         entrypoint_name.to_string(),
    //     );
    //     let miro_shape_style = MiroShapeStyle::new_from_hex_border_color("#2d9bf0");
    //     miro_shape
    //         .create_shape_in_frame(miro_shape_style, &miro_frame_id)
    //         .await?;
    // }
    Ok(())
}

pub async fn deploy_metadata_screenshot_to_frame(
    _default: bool,
    select_all: bool,
) -> Result<(), String> {
    MiroConfig::check_miro_enabled();

    println!(
        "\n\nGetting the {} from the {} ...\n\n",
        "frames".yellow(),
        "Miro board".yellow()
    );
    let mut miro_frames: Vec<MiroFrame> = MiroFrame::get_frames_from_miro().await;
    miro_frames.sort_by(|a, b| a.title.cmp(&b.title));
    let miro_frame_titles: Vec<&str> = miro_frames
        .iter()
        .map(|frame| frame.title.as_str())
        .map(|frame| frame.clone())
        .collect();

    let prompt_text = format!("Please select the destination {}", "Miro Frame".green());
    let selection = batbelt::cli_inputs::select(&prompt_text, miro_frame_titles, None).unwrap();
    let selected_miro_frame: &MiroFrame = &miro_frames[selection];
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
        let selected_section_title = section.section_header.title.clone();
        let structs_title = MetadataSection::Structs.to_string();
        let functions_title = MetadataSection::Functions.to_string();
        match true {
            _ if selected_section_title == structs_title => {
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
                    .map(|struct_metadata| {
                        format!(
                            "{}: {}:{}",
                            struct_metadata.name.clone(),
                            struct_metadata.path.clone(),
                            struct_metadata.start_line_index.clone()
                        )
                    })
                    .collect::<Vec<_>>();
                let prompt_text = format!("Please enter the {}", "struct to deploy".green());
                let selections = batbelt::cli_inputs::multiselect(
                    &prompt_text,
                    struct_metadata_names.clone(),
                    Some(&vec![select_all; struct_metadata_names.len()]),
                )
                .unwrap();
                let default_config = SourceCodeScreenshotOptions::get_default_metadata_options(
                    MetadataSection::Structs,
                );

                let use_default = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}\n{:#?}",
                    "use the default screenshot config?".yellow(),
                    default_config
                ))
                .unwrap();

                let screenshot_options = if use_default {
                    default_config
                } else {
                    SourceCodeMetadata::prompt_screenshot_options()
                };

                for selection in selections {
                    let selected_struct_metadata = &struct_metadata_vec[selection];
                    let source_code_metadata = SourceCodeMetadata::new(
                        selected_struct_metadata.name.clone(),
                        selected_struct_metadata.path.clone(),
                        selected_struct_metadata.start_line_index,
                        selected_struct_metadata.end_line_index,
                    );
                    source_code_metadata
                        .deploy_screenshot_to_miro_frame(
                            selected_miro_frame.clone(),
                            300,
                            selected_miro_frame.height as i64 - 300,
                            screenshot_options.clone(),
                        )
                        .await;
                }
            }
            _ if selected_section_title == functions_title => {
                // Choose metadata subsection selection
                let prompt_text = format!("Please enter the {}", "function type to deploy".green());
                let function_types_colorized =
                    FunctionMetadataType::get_colorized_functions_type_vec();
                let selection = batbelt::cli_inputs::select(
                    &prompt_text,
                    function_types_colorized.clone(),
                    None,
                )
                .unwrap();
                let selected_function_type =
                    FunctionMetadataType::get_functions_type_vec()[selection];
                let subsections = metadata_markdown.get_section_subsections(section.clone());
                let function_metadata_vec = subsections
                    .iter()
                    .filter_map(|section| {
                        let function_metadata =
                            FunctionMetadata::from_markdown_section(section.clone());
                        if function_metadata.function_type == selected_function_type {
                            Some(function_metadata)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                let function_metadata_names = function_metadata_vec
                    .iter()
                    .map(|function_metadata| {
                        format!(
                            "{}: {}:{}",
                            function_metadata.name.clone(),
                            function_metadata.path.clone(),
                            function_metadata.start_line_index.clone()
                        )
                    })
                    .collect::<Vec<_>>();
                let prompt_text = format!("Please enter the {}", "function to deploy".green());
                let selections = batbelt::cli_inputs::multiselect(
                    &prompt_text,
                    function_metadata_names.clone(),
                    Some(&vec![select_all; function_metadata_names.len()]),
                )
                .unwrap();

                let default_config = SourceCodeScreenshotOptions::get_default_metadata_options(
                    MetadataSection::Functions,
                );

                let use_default = batbelt::cli_inputs::select_yes_or_no(&format!(
                    "Do you want to {}\n{:#?}",
                    "use the default screenshot config?".yellow(),
                    default_config
                ))
                .unwrap();

                let screenshot_options = if use_default {
                    default_config
                } else {
                    SourceCodeMetadata::prompt_screenshot_options()
                };

                for selection in selections {
                    let selected_function_metadata = &function_metadata_vec[selection];
                    let source_code_metadata = SourceCodeMetadata::new(
                        selected_function_metadata.name.clone(),
                        selected_function_metadata.path.clone(),
                        selected_function_metadata.start_line_index,
                        selected_function_metadata.end_line_index,
                    );
                    source_code_metadata
                        .deploy_screenshot_to_miro_frame(
                            selected_miro_frame.clone(),
                            300,
                            selected_miro_frame.height as i64 - 300,
                            screenshot_options.clone(),
                        )
                        .await;
                }
            }
            _ => unimplemented!(),
        };
        // promp if continue

        let prompt_text = format!(
            "Do you want to {} in the {} frame?",
            "continue creating screenshots".yellow(),
            selected_miro_frame.title.yellow()
        );
        continue_selection = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
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
