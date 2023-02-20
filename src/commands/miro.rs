use crate::batbelt;

use std::fs;

use colored::Colorize;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSection};

use crate::batbelt::helpers::get::get_only_files_from_folder;
use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::functions_metadata::{FunctionMetadata, FunctionMetadataType};
use crate::batbelt::metadata::{BatMetadataType, MetadataError};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::BatFile;
use crate::batbelt::structs::{SignerInfo, SignerType};

use crate::batbelt::metadata::source_code_metadata::SourceCodeMetadata;
use crate::batbelt::metadata::source_code_metadata::SourceCodeScreenshotOptions;
use crate::batbelt::metadata::structs_metadata::{StructMetadata, StructMetadataType};
use crate::batbelt::miro::connector::{create_connector, ConnectorOptions};
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::image::MiroImage;

use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::path::BatFolder;
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CoderOverhaulTemplatePlaceholders,
};

use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;

use super::CommandError;

pub async fn deploy_code_overhaul_screenshots_to_frame() -> Result<(), CommandError> {
    unimplemented!();
    // MiroConfig::check_miro_enabled();
    // let started_path = batbelt::path::get_folder_path(BatFolder::CodeOverhaulStarted, false)
    //     .change_context(CommandError)?;
    // let started_files_file_info =
    //     get_only_files_from_folder(started_path).change_context(CommandError)?;
    // let file_names = started_files_file_info
    //     .iter()
    //     .map(|file_info| file_info.name.clone())
    //     .collect::<Vec<_>>();
    // let prompt_text = "Select the co file to deploy to Miro";
    // let selection =
    //     batbelt::cli_inputs::select(&prompt_text, file_names, None).change_context(CommandError)?;
    // let selected_file_info = &started_files_file_info[selection];
    // let entrypoint_name = selected_file_info.name.trim_end_matches(".md");
    // let selected_co_started_path = selected_file_info.path.clone();
    // let miro_frames = MiroFrame::get_frames_from_miro()
    //     .await
    //     .change_context(CommandError)?;
    // let entrypoint_frame = miro_frames
    //     .iter()
    //     .find(|frame| frame.title == entrypoint_name);
    // let entrypoint_frame = if let Some(ep_frame) = entrypoint_frame {
    //     ep_frame
    // } else {
    //     unimplemented!()
    // };
    // let entrypoint_frame_objects = entrypoint_frame.get_items_within_frame().await;
    //
    // let is_deploying = entrypoint_frame_objects
    //     .change_context(CommandError)?
    //     .is_empty();
    // if is_deploying {
    //     // check that the signers are finished
    //     let current_content = fs::read_to_string(selected_co_started_path.clone()).unwrap();
    //     if current_content.contains(
    //         &CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription.to_placeholder(),
    //     ) {
    //         return Err(Report::new(CommandError).attach_printable(format!(
    //             "Please complete the signers description before deploying to Miro"
    //         )));
    //     }
    //     let entrypoints_metadata_path = BatFile::EntrypointsMetadata
    //         .get_path(false)
    //         .change_context(CommandError)?;
    //     let metadata_markdown = MarkdownFile::new(&entrypoints_metadata_path);
    //     let entrypoints_section = metadata_markdown
    //         .get_section(&BatMetadataType::Entrypoints.to_sentence_case())
    //         .unwrap();
    //     let started_entrypoint_section =
    //         metadata_markdown.get_subsection(&entrypoint_name, entrypoints_section.section_header);
    //     let EntrypointMetadata {
    //         name: _,
    //         signers,
    //         instruction_file_path,
    //         handler_function,
    //         context_name: _,
    //         mut_accounts: _,
    //         function_parameters: _,
    //     } = EntrypointMetadata::from_markdown_section(started_entrypoint_section);
    //     // get the signers name and description
    //
    //     let mut signers_info: Vec<SignerInfo> = vec![];
    //     if !signers.is_empty() {
    //         for signer_name in signers.iter() {
    //             let prompt_text = format!(
    //                 "is the signer {} a validated signer?",
    //                 format!("{signer_name}").red()
    //             );
    //             let selection = batbelt::cli_inputs::select_yes_or_no(&prompt_text)
    //                 .change_context(CommandError)?;
    //             let signer_type = if selection {
    //                 SignerType::Validated
    //             } else {
    //                 SignerType::NotValidated
    //             };
    //
    //             let signer_title = if selection {
    //                 format!("Validated signer:\n\n {}", signer_name)
    //             } else {
    //                 format!("Not validated signer:\n\n {}", signer_name)
    //             };
    //
    //             signers_info.push(SignerInfo {
    //                 signer_text: signer_title,
    //                 sticky_note_id: "".to_string(),
    //                 user_figure_id: "".to_string(),
    //                 signer_type,
    //             })
    //         }
    //     } else {
    //         // no signers, push template signer
    //         signers_info.push(SignerInfo {
    //             signer_text: "Permissionless".to_string(),
    //             sticky_note_id: "".to_string(),
    //             user_figure_id: "".to_string(),
    //             signer_type: SignerType::NotSigner,
    //         })
    //     }
    //
    //     println!(
    //         "Creating signers figures in Miro for {}",
    //         entrypoint_name.green()
    //     );
    //
    //     for (signer_index, signer) in signers_info.iter_mut().enumerate() {
    //         let x_position = 550;
    //         let y_position = (150 + signer_index * 270) as i64;
    //         let width = 374;
    //         let mut signer_sticky_note = MiroStickyNote::new(
    //             &signer.signer_text,
    //             signer.signer_type.get_sticky_note_color(),
    //             &entrypoint_frame.item_id,
    //             x_position,
    //             y_position,
    //             width,
    //             0,
    //         );
    //         signer_sticky_note
    //             .deploy()
    //             .await
    //             .change_context(CommandError)?;
    //
    //         let user_figure_url = "https://mirostatic.com/app/static/12079327f83ff492.svg";
    //         let y_position = (150 + signer_index * 270) as i64;
    //         let mut user_figure = MiroImage::new_from_url(
    //             user_figure_url,
    //             &entrypoint_frame.item_id,
    //             150,
    //             y_position,
    //             200,
    //         );
    //         user_figure.deploy().await.change_context(CommandError)?;
    //
    //         *signer = SignerInfo {
    //             signer_text: signer.signer_text.clone(),
    //             sticky_note_id: signer_sticky_note.item_id,
    //             user_figure_id: user_figure.item_id,
    //             signer_type: SignerType::NotSigner,
    //         }
    //     }
    //     // Handler figure
    //     let functions_section = metadata_markdown
    //         .get_section(&BatMetadataType::Functions.to_sentence_case())
    //         .unwrap();
    //     let functions_subsections =
    //         metadata_markdown.get_section_subsections(functions_section.clone());
    //     let handler_subsection = functions_subsections
    //         .iter()
    //         .find(|subsection| {
    //             subsection.section_header.title == handler_function
    //                 && subsection.content.contains(&instruction_file_path)
    //         })
    //         .unwrap();
    //     let handler_function_metadata =
    //         FunctionMetadata::from_markdown_section(handler_subsection.clone())
    //             .change_context(CommandError)?;
    //     let handler_source_code = SourceCodeMetadata::new(
    //         handler_function,
    //         handler_function_metadata.path,
    //         handler_function_metadata.start_line_index,
    //         handler_function_metadata.end_line_index,
    //     );
    //     let entrypoint_metadata = functions_subsections
    //         .iter()
    //         .map(|function| {
    //             FunctionMetadata::from_markdown_section(function.clone())
    //                 .change_context(CommandError)
    //         })
    //         .collect::<Result<Vec<FunctionMetadata>, _>>()?
    //         .into_iter()
    //         .find(|function_metadata| {
    //             function_metadata.function_type == FunctionMetadataType::EntryPoint
    //                 && function_metadata.name == entrypoint_name
    //         })
    //         .ok_or(CommandError)
    //         .into_report()
    //         .attach_printable(format!(
    //             "Error finding FunctionMetadata for {}",
    //             entrypoint_name
    //         ))?;
    //
    //     let entrypoint_source_code = SourceCodeMetadata::new(
    //         entrypoint_metadata.name,
    //         entrypoint_metadata.path,
    //         entrypoint_metadata.start_line_index,
    //         entrypoint_metadata.end_line_index,
    //     );
    //     // Context accounts figure
    //     let co_file_markdown = MarkdownFile::new(&selected_co_started_path);
    //     let context_accounts_section = co_file_markdown
    //         .get_section(&CodeOverhaulSection::ContextAccounts.to_title())
    //         .unwrap();
    //     let context_accounts_source_code = SourceCodeMetadata::new(
    //         CodeOverhaulSection::ContextAccounts.to_title(),
    //         selected_co_started_path.clone(),
    //         context_accounts_section.start_line_index,
    //         context_accounts_section.end_line_index,
    //     );
    //     // Validations figure
    //     let validations_section = co_file_markdown
    //         .get_section(&CodeOverhaulSection::Validations.to_title())
    //         .unwrap();
    //
    //     let validations_accounts_source_code = SourceCodeMetadata::new(
    //         CodeOverhaulSection::Validations.to_title(),
    //         selected_co_started_path.clone(),
    //         validations_section.start_line_index,
    //         validations_section.end_line_index,
    //     );
    //     let options = SourceCodeScreenshotOptions {
    //         include_path: true,
    //         offset_to_start_line: true,
    //         filter_comments: true,
    //         font_size: Some(20),
    //         filters: None,
    //         show_line_number: true,
    //     };
    //     let co_options = SourceCodeScreenshotOptions {
    //         include_path: false,
    //         offset_to_start_line: false,
    //         filter_comments: false,
    //         font_size: Some(20),
    //         filters: None,
    //         show_line_number: false,
    //     };
    //     let handler_screenshot_path = handler_source_code
    //         .create_screenshot(options.clone())
    //         .change_context(CommandError)?;
    //     let entrypoint_screenshot_path = entrypoint_source_code
    //         .create_screenshot(options.clone())
    //         .change_context(CommandError)?;
    //     let co_screenshot_path = context_accounts_source_code
    //         .create_screenshot(co_options.clone())
    //         .change_context(CommandError)?;
    //     let validations_screenshot_path = validations_accounts_source_code
    //         .create_screenshot(co_options.clone())
    //         .change_context(CommandError)?;
    //
    //     // Miro Images&
    //     let mut handler_miro_image =
    //         MiroImage::new_from_file_path(&handler_screenshot_path, &entrypoint_frame.item_id);
    //     let mut entrypoint_miro_image =
    //         MiroImage::new_from_file_path(&entrypoint_screenshot_path, &entrypoint_frame.item_id);
    //     let mut co_miro_image =
    //         MiroImage::new_from_file_path(&co_screenshot_path, &entrypoint_frame.item_id);
    //     let mut validations_miro_image =
    //         MiroImage::new_from_file_path(&validations_screenshot_path, &entrypoint_frame.item_id);
    //
    //     handler_miro_image
    //         .deploy()
    //         .await
    //         .change_context(CommandError)?;
    //     entrypoint_miro_image
    //         .deploy()
    //         .await
    //         .change_context(CommandError)?;
    //     co_miro_image.deploy().await.change_context(CommandError)?;
    //     validations_miro_image
    //         .deploy()
    //         .await
    //         .change_context(CommandError)?;
    //
    //     entrypoint_miro_image.update_position(1300, 250).await;
    //     co_miro_image.update_position(2200, 350).await;
    //     validations_miro_image.update_position(3000, 500).await;
    //     handler_miro_image.update_position(2900, 1400).await;
    //
    //     println!("Connecting signers to entrypoint");
    //     for signer_miro_ids in signers_info {
    //         batbelt::miro::connector::create_connector(
    //             &signer_miro_ids.user_figure_id,
    //             &signer_miro_ids.sticky_note_id,
    //             None,
    //         )
    //         .await
    //         .change_context(CommandError)?;
    //         batbelt::miro::connector::create_connector(
    //             &signer_miro_ids.sticky_note_id,
    //             &entrypoint_miro_image.item_id,
    //             Some(ConnectorOptions {
    //                 start_x_position: "100%".to_string(),
    //                 start_y_position: "50%".to_string(),
    //                 end_x_position: "0%".to_string(),
    //                 end_y_position: "50%".to_string(),
    //             }),
    //         )
    //         .await
    //         .change_context(CommandError)?;
    //     }
    //
    //     println!("Connecting snapshots in Miro");
    //     batbelt::miro::connector::create_connector(
    //         &entrypoint_miro_image.item_id,
    //         &co_miro_image.item_id,
    //         None,
    //     )
    //     .await
    //     .change_context(CommandError)?;
    //     batbelt::miro::connector::create_connector(
    //         &co_miro_image.item_id,
    //         &validations_miro_image.item_id,
    //         None,
    //     )
    //     .await
    //     .change_context(CommandError)?;
    //     batbelt::miro::connector::create_connector(
    //         &validations_miro_image.item_id,
    //         &handler_miro_image.item_id,
    //         None,
    //     )
    //     .await
    //     .change_context(CommandError)?;
    //
    //     // // Deploy mut_accounts
    //     // if mut_accounts.len() > 0 {
    //     //     let structs_section = metadata_markdown
    //     //         .get_section(&MetadataSection::Structs.to_sentence_case())
    //     //         .unwrap();
    //     //     let structs_subsection = metadata_markdown.get_section_subsections(structs_section);
    //     //     for mut_account in mut_accounts {
    //     //         let mut_account_section = structs_subsection.iter().find_map(|subsection| {
    //     //             let struct_md_section =
    //     //                 StructMetadata::from_markdown_section(subsection.clone());
    //     //             if struct_md_section.struct_type == StructMetadataType::SolanaAccount
    //     //                 && struct_md_section.name == mut_account[1]
    //     //             {
    //     //                 Some(struct_md_section)
    //     //             } else {
    //     //                 None
    //     //             }
    //     //         });
    //     //         if let Some(mut_section) = mut_account_section {
    //     //             let mut_acc_source_code = SourceCodeMetadata::new(
    //     //                 CodeOverhaulSection::Validations.to_title(),
    //     //                 mut_section.path.clone(),
    //     //                 mut_section.start_line_index,
    //     //                 mut_section.end_line_index,
    //     //             );
    //     //             let mut_acc_screenshot_path =
    //     //                 mut_acc_source_code.create_screenshot(options.clone());
    //     //             let mut mut_acc_miro_image = MiroImage::new_from_file_path(
    //     //                 &mut_acc_screenshot_path,
    //     //                 &entrypoint_frame.item_id,
    //     //             );
    //     //             mut_acc_miro_image.deploy().await;
    //     //             mut_acc_miro_image.update_position(400, 400).await;
    //     //             // fs::remove_file(mut_acc_screenshot_path).unwrap();
    //     //         }
    //     //     }
    //     // }
    //     // Remove screenshots
    //     fs::remove_file(handler_screenshot_path).unwrap();
    //     fs::remove_file(co_screenshot_path).unwrap();
    //     fs::remove_file(validations_screenshot_path).unwrap();
    //     fs::remove_file(entrypoint_screenshot_path).unwrap();
    //
    //     //
    //     // create_git_commit(
    //     //     GitCommit::DeployMiro,
    //     //     Some(vec![selected_co_started_path.to_string()]),
    //     // )
    //     // .unwrap();
    //     Ok(())
    // } else {
    //     // update images
    //     // let prompt_text = format!("select the images to update for {selected_folder}");
    //     // let selections = batbelt::cli_inputs::multiselect(
    //     //     &prompt_text,
    //     //     CO_FIGURES.to_vec(),
    //     //     Some(&vec![true, true, true, true]),
    //     // )?;
    //     // if !selections.is_empty() {
    //     //     for selection in selections.iter() {
    //     //         let snapshot_path_vec = &snapshot_paths.clone().collect::<Vec<_>>();
    //     //         let snapshot_path = &snapshot_path_vec.as_slice()[*selection];
    //     //         let file_name = snapshot_path.split('/').last().unwrap();
    //     //         println!("Updating: {file_name}");
    //     //         let item_id =
    //     //             batbelt::helpers::get::get_screenshot_id(file_name, &selected_co_started_path);
    //     //         let mut screenshot_image =
    //     //             MiroImage::new_from_item_id(&item_id, MiroImageType::FromPath).await;
    //     //         screenshot_image.update_from_path(&snapshot_path).await;
    //     //     }
    //     //     create_git_commit(
    //     //         GitCommit::UpdateMiro,
    //     //         Some(vec![selected_folder.to_string()]),
    //     //     )?;
    //     // } else {
    //     //     println!("No files selected");
    //     // }
    //     Ok(())
    // }
}

pub async fn deploy_entrypoint_screenshots_to_frame(
    select_all: bool,
    sorted: bool,
) -> Result<(), CommandError> {
    // get entrypoints name
    let entrypoints_names =
        EntrypointParser::get_entrypoints_names(sorted).change_context(CommandError)?;

    // prompt the user to select an entrypoint
    let prompt_text = "Please select the entrypoints to deploy";
    let selected_entrypoints_index = batbelt::cli_inputs::multiselect(
        prompt_text,
        entrypoints_names.clone(),
        Some(&vec![select_all; entrypoints_names.clone().len()]),
    )
    .unwrap();
    // prompt to select the frame
    let mut miro_frames = MiroFrame::get_frames_from_miro()
        .await
        .change_context(CommandError)?;
    miro_frames.sort_by(|frame_a, frame_b| frame_a.title.cmp(&frame_b.title));
    let miro_frames_names = miro_frames
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
    let selected_entrypoints_amount = if selected_entrypoints_index.len() % 2 == 0 {
        selected_entrypoints_index.len()
    } else {
        selected_entrypoints_index.len() + 1
    };
    let grid_amount = 24;
    let height_grid = selected_frame.height as i64 / grid_amount;
    // this number indicates the distance between screenshot relate to the grid amount
    let (ep_multiplier, ca_multiplier, handler_multiplier) = (1, 2, 4);
    for (index, selected_ep_index) in selected_entrypoints_index.iter().enumerate() {
        // this number indicates the distance between screenshot relate to the grid amount
        let (x_position, ep_y_position, ca_y_position, handler_y_position) =
            if index < selected_entrypoints_amount / 2 {
                let x_position = (selected_frame.width as i64 / selected_entrypoints_amount as i64)
                    * (2 * index as i64 + 1);
                (
                    x_position,
                    ep_multiplier * height_grid,
                    ca_multiplier * height_grid,
                    handler_multiplier * height_grid,
                )
            } else {
                let x_position = (selected_frame.width as i64 / selected_entrypoints_amount as i64)
                    * (2 * (index as i64 - (selected_entrypoints_amount as i64 / 2)) + 1);
                (
                    x_position,
                    (grid_amount - ep_multiplier) * height_grid,
                    (grid_amount - ca_multiplier) * height_grid,
                    (grid_amount - handler_multiplier) * height_grid,
                )
            };
        let selected_entrypoint = &entrypoints_names[selected_ep_index.clone()];
        // get context_accounts name
        let entrypoint = EntrypointParser::new_from_name(selected_entrypoint.as_str())
            .change_context(CommandError)?;
        let ep_source_code =
            entrypoint
                .entrypoint_function
                .to_source_code(Some(parse_entrypoint_screenshot_name(
                    &entrypoint.entrypoint_function.name,
                    &selected_frame.title,
                )));
        let ca_source_code =
            entrypoint
                .context_accounts
                .to_source_code(Some(parse_entrypoint_screenshot_name(
                    &entrypoint.context_accounts.name,
                    &selected_frame.title,
                )));
        let ep_image = ep_source_code
            .deploy_screenshot_to_miro_frame(
                selected_frame.clone(),
                x_position,
                ep_y_position,
                entrypoint_sc_options.clone(),
            )
            .await
            .change_context(CommandError)?;
        let ca_image = ca_source_code
            .deploy_screenshot_to_miro_frame(
                selected_frame.clone(),
                x_position,
                ca_y_position,
                context_accounts_sc_options.clone(),
            )
            .await
            .change_context(CommandError)?;
        create_connector(&ep_image.item_id, &ca_image.item_id, None)
            .await
            .change_context(CommandError)?;
        if let Some(entrypoint_handler) = entrypoint.handler {
            let handler_source_code = entrypoint_handler.to_source_code(Some(
                parse_entrypoint_screenshot_name(&entrypoint_handler.name, &selected_frame.title),
            ));
            let handler_image = handler_source_code
                .deploy_screenshot_to_miro_frame(
                    selected_frame.clone(),
                    x_position,
                    handler_y_position,
                    handler_sc_options.clone(),
                )
                .await
                .change_context(CommandError)?;
            create_connector(&ca_image.item_id, &handler_image.item_id, None)
                .await
                .change_context(CommandError)?;
        }
    }
    Ok(())
}

fn parse_entrypoint_screenshot_name(function_name: &str, frame_title: &str) -> String {
    format!(
        "{}-frame:{}",
        function_name,
        frame_title
            .replace(" ", "_")
            .replace("-", "_")
            .to_screaming_snake_case()
    )
}

pub async fn deploy_metadata_screenshot_to_frame(
    _default: bool,
    select_all: bool,
) -> Result<(), CommandError> {
    MiroConfig::check_miro_enabled();

    println!(
        "\n\nGetting the {} from the {} ...\n\n",
        "frames".yellow(),
        "Miro board".yellow()
    );
    let mut miro_frames: Vec<MiroFrame> = MiroFrame::get_frames_from_miro()
        .await
        .change_context(CommandError)?;

    log::info!("miro_frames:\n{:#?}", miro_frames);

    miro_frames.sort_by(|a, b| a.title.cmp(&b.title));
    let miro_frame_titles: Vec<String> = miro_frames
        .iter()
        .map(|frame| frame.title.clone())
        .collect();

    let prompt_text = format!("Please select the destination {}", "Miro Frame".green());
    let selection = batbelt::cli_inputs::select(&prompt_text, miro_frame_titles, None).unwrap();
    let selected_miro_frame: MiroFrame = miro_frames[selection].clone();
    let metadata_types_vec = BatMetadataType::get_metadata_type_vec();
    let metadata_types_colorized_vec = BatMetadataType::get_colorized_metadata_type_vec();
    let mut continue_selection = true;
    while continue_selection {
        // Choose metadata section selection
        let prompt_text = format!("Please enter the {}", "metadata type".green());
        let selection =
            batbelt::cli_inputs::select(&prompt_text, metadata_types_colorized_vec.clone(), None)
                .unwrap();
        let metadata_type_selected = &metadata_types_vec[selection];
        let (sourcecode_metadata_vec, screenshot_options): (
            Vec<SourceCodeMetadata>,
            SourceCodeScreenshotOptions,
        ) = match metadata_type_selected {
            BatMetadataType::Structs => {
                // Choose metadata subsection selection
                let prompt_text = format!("Please enter the {}", "struct type to deploy".green());
                let struct_types_colorized = StructMetadataType::get_colorized_structs_type_vec();
                let selection =
                    batbelt::cli_inputs::select(&prompt_text, struct_types_colorized.clone(), None)
                        .unwrap();
                let selected_struct_type = StructMetadataType::get_structs_type_vec()[selection];
                let struct_metadata_vec =
                    StructMetadata::get_filtered_metadata(None, Some(selected_struct_type))
                        .change_context(CommandError)?;
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
                    BatMetadataType::Structs,
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
                let sc_vec = struct_metadata_vec
                    .into_iter()
                    .enumerate()
                    .filter_map(|(sc_index, sc_metadata)| {
                        if selections.iter().any(|selection| &sc_index == selection) {
                            Some(sc_metadata.to_source_code(None))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                (sc_vec, screenshot_options)
            }
            BatMetadataType::Functions => {
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
                let function_metadata_vec =
                    FunctionMetadata::get_filtered_metadata(None, Some(selected_function_type))
                        .change_context(CommandError)?;
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
                    BatMetadataType::Functions,
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

                let sc_vec = function_metadata_vec
                    .into_iter()
                    .enumerate()
                    .filter_map(|(sc_index, sc_metadata)| {
                        if selections.iter().any(|selection| &sc_index == selection) {
                            Some(sc_metadata.to_source_code(None))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                (sc_vec, screenshot_options)
            }
            _ => unimplemented!(),
        };
        // promp if continue
        for sc_metadata in sourcecode_metadata_vec {
            sc_metadata
                .deploy_screenshot_to_miro_frame(
                    selected_miro_frame.clone(),
                    0,
                    selected_miro_frame.height as i64,
                    screenshot_options.clone(),
                )
                .await
                .change_context(CommandError)?;
        }
        let prompt_text = format!(
            "Do you want to {} in the {} frame?",
            "continue creating screenshots".yellow(),
            selected_miro_frame.title.yellow()
        );
        continue_selection = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
    }
    Ok(())
}

#[test]
fn test_screaming_snake_case() {
    let function_name = "handle_thing";
    let frame_name = "points-store actors";
    let expected_output = "handle_thing-frame:POINTS_STORE_ACTORS";
    println!(
        "{}",
        parse_entrypoint_screenshot_name(function_name, frame_name)
    );
    assert_eq!(
        parse_entrypoint_screenshot_name(function_name, frame_name),
        expected_output,
        "incorrect output"
    )
}
