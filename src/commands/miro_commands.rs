use std::fs;

use clap::Subcommand;
use colored::{ColoredString, Colorize};
use error_stack::{FutureExt, IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use regex::Regex;
use strum::IntoEnumIterator;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::functions_source_code_metadata::{
    FunctionMetadataType, FunctionSourceCodeMetadata,
};
use crate::batbelt::metadata::miro_metadata::{MiroCodeOverhaulMetadata, SignerInfo, SignerType};
use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataCommit, BatMetadataParser, BatMetadataType, MetadataError,
    MiroMetadata, SourceCodeMetadata,
};
use crate::batbelt::miro::connector::{create_connector, ConnectorOptions};
use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::frame::{
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::miro::image::{MiroImage, MiroImageType};
use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::parser::code_overhaul_parser::CodeOverhaulParser;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::parser::source_code_parser::{SourceCodeParser, SourceCodeScreenshotOptions};
use crate::batbelt::path::BatFolder;
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandResult};
use crate::{batbelt, Suggestion};

use super::CommandError;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum MiroCommand {
    /// Creates the code-overhaul frames
    #[default]
    CodeOverhaulFrames,
    /// Deploys the code-overhaul screenshots
    CodeOverhaulScreenshots {
        /// If provided, skips the co file selection process
        #[arg(long)]
        entry_point_name: Option<String>,
    },
    /// Deploys the entry point function, context accounts and handler function screenshots to a Miro frame
    EntrypointScreenshots {
        /// if true, deploy screenshots for al entry points
        #[arg(short, long)]
        select_all_entry_points: bool,
        /// shows the list of entrypoints sorted by name
        #[arg(long)]
        sorted: bool,
    },
    /// Creates an screenshot in a determined frame from metadata
    Metadata {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Creates screenshot for a function and it dependencies
    FunctionDependencies {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
}

impl BatEnumerator for MiroCommand {}

impl BatCommandEnumerator for MiroCommand {
    fn execute_command(&self) -> CommandResult<()> {
        unimplemented!()
    }

    fn check_metadata_is_initialized(&self) -> bool {
        true
    }

    fn check_correct_branch(&self) -> bool {
        false
    }
}

impl MiroCommand {
    pub async fn execute_command(&self) -> Result<(), CommandError> {
        MiroConfig::check_miro_enabled().change_context(CommandError)?;
        match self {
            MiroCommand::CodeOverhaulFrames => self.deploy_co_frames().await,
            MiroCommand::CodeOverhaulScreenshots { entry_point_name } => {
                self.deploy_co_screenshots(entry_point_name.clone()).await
            }
            MiroCommand::EntrypointScreenshots {
                select_all_entry_points,
                sorted,
            } => {
                self.entrypoint_screenshots(*select_all_entry_points, *sorted)
                    .await
            }
            MiroCommand::Metadata { select_all } => self.metadata(*select_all).await,
            MiroCommand::FunctionDependencies { select_all } => {
                self.function_dependencies(*select_all).await
            }
        }
    }

    async fn entrypoint_screenshots(
        &self,
        select_all: bool,
        sorted: bool,
    ) -> Result<(), CommandError> {
        let code_overhaul_frame_title_regex = Regex::new(r"co: [A-Za-z0-9_]+")
            .into_report()
            .change_context(CommandError)?;
        let selected_miro_frame =
            MiroFrame::prompt_select_frame(Some(vec![code_overhaul_frame_title_regex]))
                .await
                .change_context(CommandError)?;
        // get entrypoints name
        let entrypoints_names =
            EntrypointParser::get_entrypoint_names(sorted).change_context(CommandError)?;

        // prompt the user to select an entrypoint
        let prompt_text = "Please select the entrypoints to deploy";
        let selected_entrypoints_index = batbelt::bat_dialoguer::multiselect(
            prompt_text,
            entrypoints_names.clone(),
            Some(&vec![select_all; entrypoints_names.clone().len()]),
        )
        .unwrap();

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
        let height_grid = selected_miro_frame.height as i64 / grid_amount;
        // this number indicates the distance between screenshot relate to the grid amount
        let (ep_multiplier, ca_multiplier, handler_multiplier) = (1, 2, 4);
        for (index, selected_ep_index) in selected_entrypoints_index.iter().enumerate() {
            // this number indicates the distance between screenshot relate to the grid amount
            let (x_position, ep_y_position, ca_y_position, handler_y_position) =
                if index < selected_entrypoints_amount / 2 {
                    let x_position = (selected_miro_frame.width as i64
                        / selected_entrypoints_amount as i64)
                        * (2 * index as i64 + 1);
                    (
                        x_position,
                        ep_multiplier * height_grid,
                        ca_multiplier * height_grid,
                        handler_multiplier * height_grid,
                    )
                } else {
                    let x_position = (selected_miro_frame.width as i64
                        / selected_entrypoints_amount as i64)
                        * (2 * (index as i64 - (selected_entrypoints_amount as i64 / 2)) + 1);
                    (
                        x_position,
                        (grid_amount - ep_multiplier) * height_grid,
                        (grid_amount - ca_multiplier) * height_grid,
                        (grid_amount - handler_multiplier) * height_grid,
                    )
                };
            let selected_entrypoint = &entrypoints_names[*selected_ep_index];
            // get context_accounts name
            let entrypoint = EntrypointParser::new_from_name(selected_entrypoint.as_str())
                .change_context(CommandError)?;
            let ep_source_code = entrypoint.entry_point_function.to_source_code_parser(Some(
                miro_command_functions::parse_screenshot_name(
                    &entrypoint.entry_point_function.name,
                    &selected_miro_frame.title,
                ),
            ));
            let ca_source_code = entrypoint.context_accounts.to_source_code_parser(Some(
                miro_command_functions::parse_screenshot_name(
                    &entrypoint.context_accounts.name,
                    &selected_miro_frame.title,
                ),
            ));
            let ep_image = ep_source_code
                .deploy_screenshot_to_miro_frame(
                    selected_miro_frame.clone(),
                    x_position,
                    ep_y_position,
                    entrypoint_sc_options.clone(),
                )
                .await
                .change_context(CommandError)?;
            let ca_image = ca_source_code
                .deploy_screenshot_to_miro_frame(
                    selected_miro_frame.clone(),
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
                let handler_source_code = entrypoint_handler.to_source_code_parser(Some(
                    miro_command_functions::parse_screenshot_name(
                        &entrypoint_handler.name,
                        &selected_miro_frame.title,
                    ),
                ));
                let handler_image = handler_source_code
                    .deploy_screenshot_to_miro_frame(
                        selected_miro_frame.clone(),
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

    async fn metadata(&self, select_all: bool) -> Result<(), CommandError> {
        let selected_miro_frame = MiroFrame::prompt_select_frame(None)
            .await
            .change_context(CommandError)?;
        let mut continue_selection = true;
        let metadata_types_vec = BatMetadataType::get_type_vec();
        let metadata_types_colorized_vec = BatMetadataType::get_colorized_type_vec(true);
        while continue_selection {
            // Choose metadata section selection
            let prompt_text = format!("Please enter the {}", "metadata type".green());
            let selection = batbelt::bat_dialoguer::select(
                &prompt_text,
                metadata_types_colorized_vec.clone(),
                None,
            )
            .unwrap();
            let metadata_type_selected = &metadata_types_vec[selection];
            let (sourcecode_metadata_vec, screenshot_options): (
                Vec<SourceCodeParser>,
                SourceCodeScreenshotOptions,
            ) = match metadata_type_selected {
                BatMetadataType::Struct => {
                    // Choose metadata subsection selection
                    let prompt_text =
                        format!("Please enter the {}", "struct type to deploy".green());
                    let struct_types_colorized = StructMetadataType::get_colorized_type_vec(true);
                    let selection = batbelt::bat_dialoguer::select(
                        &prompt_text,
                        struct_types_colorized.clone(),
                        None,
                    )
                    .unwrap();
                    let selected_struct_type = StructMetadataType::get_type_vec()[selection];
                    let struct_metadata_vec =
                        SourceCodeMetadata::get_filtered_structs(None, Some(selected_struct_type))
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
                    let selections = BatDialoguer::multiselect(
                        prompt_text,
                        struct_metadata_names.clone(),
                        Some(&vec![select_all; struct_metadata_names.len()]),
                        true,
                    )
                    .unwrap();
                    let default_config = SourceCodeScreenshotOptions::get_default_metadata_options(
                        BatMetadataType::Struct,
                    );

                    let use_default = batbelt::bat_dialoguer::select_yes_or_no(&format!(
                        "Do you want to {}\n{:#?}",
                        "use the default screenshot config?".yellow(),
                        default_config
                    ))
                    .unwrap();

                    let screenshot_options = if use_default {
                        default_config
                    } else {
                        SourceCodeParser::prompt_screenshot_options()
                    };
                    let sc_vec = struct_metadata_vec
                        .into_iter()
                        .enumerate()
                        .filter_map(|(sc_index, sc_metadata)| {
                            if selections.iter().any(|selection| &sc_index == selection) {
                                Some(sc_metadata.to_source_code_parser(Some(
                                    miro_command_functions::parse_screenshot_name(
                                        &sc_metadata.name,
                                        &selected_miro_frame.title,
                                    ),
                                )))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    (sc_vec, screenshot_options)
                }
                BatMetadataType::Function => {
                    // Choose metadata subsection selection
                    let prompt_text =
                        format!("Please enter the {}", "function type to deploy".green());
                    let function_types_colorized =
                        FunctionMetadataType::get_colorized_type_vec(true);
                    let selection = batbelt::bat_dialoguer::select(
                        &prompt_text,
                        function_types_colorized.clone(),
                        None,
                    )
                    .unwrap();
                    let selected_function_type = FunctionMetadataType::get_type_vec()[selection];
                    let function_metadata_vec = SourceCodeMetadata::get_filtered_functions(
                        None,
                        Some(selected_function_type),
                    )
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
                    let selections = BatDialoguer::multiselect(
                        prompt_text,
                        function_metadata_names.clone(),
                        Some(&vec![select_all; function_metadata_names.len()]),
                        true,
                    )
                    .unwrap();

                    let default_config = SourceCodeScreenshotOptions::get_default_metadata_options(
                        BatMetadataType::Function,
                    );

                    let use_default = batbelt::bat_dialoguer::select_yes_or_no(&format!(
                        "Do you want to {}\n{:#?}",
                        "use the default screenshot config?".yellow(),
                        default_config
                    ))
                    .unwrap();

                    let screenshot_options = if use_default {
                        default_config
                    } else {
                        SourceCodeParser::prompt_screenshot_options()
                    };

                    let sc_vec = function_metadata_vec
                        .into_iter()
                        .enumerate()
                        .filter_map(|(sc_index, sc_metadata)| {
                            if selections.iter().any(|selection| &sc_index == selection) {
                                Some(sc_metadata.to_source_code_parser(Some(
                                    miro_command_functions::parse_screenshot_name(
                                        &sc_metadata.name,
                                        &selected_miro_frame.title,
                                    ),
                                )))
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
            continue_selection = batbelt::bat_dialoguer::select_yes_or_no(&prompt_text).unwrap();
        }
        Ok(())
    }

    async fn function_dependencies(&self, _select_all: bool) -> Result<(), CommandError> {
        let selected_miro_frame = MiroFrame::prompt_select_frame(None)
            .await
            .change_context(CommandError)?;
        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        let function_metadata_vec = bat_metadata.source_code.functions_source_code.clone();
        let mut keep_deploying = true;
        let mut deployed_dependencies: Vec<(MiroImage, FunctionSourceCodeMetadata)> = vec![];
        let mut pending_to_check: Vec<FunctionSourceCodeMetadata> = vec![];
        while keep_deploying {
            let function_metadata_names_vec = function_metadata_vec
                .clone()
                .into_iter()
                .map(|f_meta| {
                    miro_command_functions::get_formatted_path(
                        f_meta.name.clone(),
                        f_meta.path.clone(),
                        f_meta.start_line_index,
                    )
                })
                .collect::<Vec<_>>();
            let prompt_text = "Select the Function to deploy";
            let seleted_function_index = batbelt::bat_dialoguer::select(
                prompt_text,
                function_metadata_names_vec.clone(),
                None,
            )?;
            let selected_function_metadata = &function_metadata_vec[seleted_function_index].clone();

            pending_to_check.push(selected_function_metadata.clone());

            while !pending_to_check.is_empty() {
                let parent_function = pending_to_check.pop().unwrap();
                let miro_image = deployed_dependencies.clone().into_iter().find_map(|image| {
                    if image.1 == parent_function {
                        Some(image.0)
                    } else {
                        None
                    }
                });
                miro_command_functions::prompt_deploy_dependencies(
                    parent_function,
                    miro_image,
                    selected_miro_frame.clone(),
                    &mut deployed_dependencies,
                    &mut pending_to_check,
                )
                .await?;
            }

            let prompt_text = format!(
                "Do you want to {} in the {} frame?",
                "continue creating screenshots".yellow(),
                selected_miro_frame.title.yellow()
            );
            keep_deploying = batbelt::bat_dialoguer::select_yes_or_no(&prompt_text).unwrap();
        }

        Ok(())
    }

    async fn deploy_co_frames(&self) -> Result<(), CommandError> {
        println!("Deploying code-overhaul frames to the Miro board");

        let entry_point_names =
            EntrypointParser::get_entrypoint_names(false).change_context(CommandError)?;

        for (entrypoint_index, entrypoint_name) in entry_point_names.iter().enumerate() {
            match MiroMetadata::get_co_metadata_by_entrypoint_name(entrypoint_name.clone())
                .change_context(CommandError)
            {
                Ok(miro_co_metadata) => {
                    match MiroFrame::new_from_item_id(&miro_co_metadata.miro_frame_id)
                        .await
                        .change_context(CommandError)
                    {
                        Ok(miro_frame) => {
                            println!(
                                "Frame already deployed for {}, \nurl: {}\n",
                                entrypoint_name.clone().green(),
                                MiroFrame::get_frame_url_by_frame_id(&miro_frame.item_id)
                                    .change_context(CommandError)?
                            );
                        }
                        // the item id is incorrect, is necessary to replace the old metadata
                        Err(_) => {
                            println!(
                                "Incorrect frame deployed for {}, \nurl: {}\nUpdating the Miro metadata\n",
                                entrypoint_name.clone().red(),
                                MiroFrame::get_frame_url_by_frame_id(&miro_co_metadata.miro_frame_id)
                                    .change_context(CommandError)?
                            );
                            let new_frame = miro_command_functions::deploy_miro_frame_for_co(
                                entrypoint_name,
                                entrypoint_index,
                            )
                            .await?;
                            let new_co_metadata = MiroCodeOverhaulMetadata {
                                metadata_id: miro_co_metadata.metadata_id.clone(),
                                entry_point_name: entrypoint_name.clone(),
                                miro_frame_id: new_frame.item_id.clone(),
                                images_deployed: false,
                                entry_point_image_id: "".to_string(),
                                context_accounts_image_id: "".to_string(),
                                validations_image_id: "".to_string(),
                                handler_image_id: "".to_string(),
                                signers: vec![],
                            };
                            new_co_metadata
                                .update_code_overhaul_metadata()
                                .change_context(CommandError)?;
                        }
                    }
                }
                Err(_) => {
                    let mut miro_co_metadata = MiroCodeOverhaulMetadata {
                        metadata_id: BatMetadata::create_metadata_id(),
                        entry_point_name: entrypoint_name.clone(),
                        miro_frame_id: "".to_string(),
                        images_deployed: false,
                        entry_point_image_id: "".to_string(),
                        context_accounts_image_id: "".to_string(),
                        validations_image_id: "".to_string(),
                        handler_image_id: "".to_string(),
                        signers: vec![],
                    };

                    let miro_frame = miro_command_functions::deploy_miro_frame_for_co(
                        entrypoint_name,
                        entrypoint_index,
                    )
                    .await?;

                    miro_co_metadata.miro_frame_id = miro_frame.item_id.clone();
                    miro_co_metadata
                        .update_code_overhaul_metadata()
                        .change_context(CommandError)?;
                }
            }
        }
        GitCommit::UpdateMetadataJson {
            bat_metadata_commit: BatMetadataCommit::MiroMetadataCommit,
        }
        .create_commit()
        .change_context(CommandError)?;
        Ok(())
    }

    async fn deploy_co_screenshots(&self, entry_point_name: Option<String>) -> CommandResult<()> {
        MiroConfig::check_miro_enabled().change_context(CommandError)?;

        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;
        if bat_metadata.miro.code_overhaul.is_empty() {
            let message = format!(
                "Miro code-overhaul's metadata is not initialized yet.\n \
            This action is {} to proceed with this function.",
                "required".red()
            );
            let suggestion_message = format!(
                "Run  {} to deploy the code-overhaul frames",
                "bat-cli miro deploy-co-frames".green()
            );
            return Err(Report::new(CommandError)
                .attach_printable(message)
                .attach(Suggestion(suggestion_message)));
        }

        let co_started_bat_folder = BatFolder::CodeOverhaulStarted;
        let co_finished_bat_folder = BatFolder::CodeOverhaulFinished;
        let mut started_files_names = co_started_bat_folder
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;
        let mut finished_files_names = co_finished_bat_folder
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;
        if started_files_names.is_empty() && finished_files_names.is_empty() {
            return Err(Report::new(CommandError)
                .attach_printable("code-overhaul's to-review and finished folders are empty"));
        }

        let mut co_files_names = vec![];
        co_files_names.append(&mut started_files_names.clone());
        co_files_names.append(&mut finished_files_names.clone());
        co_files_names.sort();

        let entrypoint_name = match entry_point_name {
            None => {
                let prompt_text = "Select the co file to deploy to Miro".to_string();
                let selection = BatDialoguer::select(prompt_text, co_files_names.clone(), None)?;
                let selected_file_name = co_files_names[selection].clone();
                let entrypoint_name = selected_file_name.trim_end_matches(".md").to_string();
                entrypoint_name
            }
            Some(ep_name) => {
                let entrypoint_name = ep_name.trim_end_matches(".md").to_string();
                let co_file_name = format!("{}.md", entrypoint_name.clone());
                if !co_files_names.contains(&co_file_name) {
                    return Err(Report::new(CommandError).attach_printable(format!(
                        "code-overhaul's file with name {} not found on {} and {} folders",
                        co_file_name.clone(),
                        "to-review".bright_red(),
                        "finished".bright_red()
                    )));
                }
                entrypoint_name
            }
        };

        let (co_miro_frame, mut miro_co_metadata) =
            match MiroMetadata::get_co_metadata_by_entrypoint_name(entrypoint_name.clone()) {
                Ok(co_meta) => {
                    let frame_id = co_meta.miro_frame_id.clone();
                    let miro_frame = MiroFrame::new_from_item_id(&frame_id)
                        .change_context(CommandError)
                        .await?;
                    println!(
                        "Deploying {} to {:#?}",
                        entrypoint_name.green(),
                        miro_frame.title
                    );
                    (miro_frame, co_meta)
                }
                Err(_) => {
                    let message = format!(
                        "Miro code-overhaul's metadata not found for {}.\n \
            This action is {} to proceed with this function.",
                        entrypoint_name,
                        "required".red()
                    );
                    let suggestion_message = format!(
                        "Run  {} to deploy the code-overhaul frames",
                        "bat-cli miro deploy-co".green()
                    );
                    return Err(Report::new(CommandError)
                        .attach_printable(message)
                        .attach(Suggestion(suggestion_message)));
                }
            };
        if !miro_co_metadata.images_deployed {
            let entrypoint_parser =
                EntrypointParser::new_from_name(&entrypoint_name).change_context(CommandError)?;
            let co_parser = CodeOverhaulParser::new_from_entry_point_name(entrypoint_name.clone())
                .change_context(CommandError)?;
            let mut signers_info: Vec<SignerInfo> = vec![];
            if !co_parser.signers.is_empty() {
                for signer in co_parser.signers.clone().into_iter() {
                    let prompt_text = format!(
                        "is the signer {} a validated signer?",
                        signer.name.to_string().red()
                    );
                    let is_validated =
                        BatDialoguer::select_yes_or_no(prompt_text).change_context(CommandError)?;
                    let signer_type = if is_validated {
                        SignerType::Validated
                    } else {
                        SignerType::NotValidated
                    };

                    let signer_title = if is_validated {
                        format!("Validated signer:<br> <strong>{}</strong>", signer.name)
                    } else {
                        format!("Not validated signer:<br> <strong>{}</strong>", signer.name)
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
                    signer_text: SignerType::Permissionless.to_string(),
                    sticky_note_id: "".to_string(),
                    user_figure_id: "".to_string(),
                    signer_type: SignerType::Permissionless,
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
                    &co_miro_frame.item_id,
                    x_position,
                    y_position,
                    width,
                    0,
                );
                signer_sticky_note
                    .deploy()
                    .await
                    .change_context(CommandError)?;

                let user_figure_url = "https://mirostatic.com/app/static/12079327f83ff492.svg";
                let y_position = (150 + signer_index * 270) as i64;
                let mut user_figure = MiroImage::new_from_url(
                    user_figure_url,
                    &co_miro_frame.item_id,
                    150,
                    y_position,
                    200,
                );
                user_figure.deploy().await.change_context(CommandError)?;

                *signer = SignerInfo {
                    signer_text: signer.signer_text.clone(),
                    sticky_note_id: signer_sticky_note.item_id,
                    user_figure_id: user_figure.item_id,
                    signer_type: signer.signer_type,
                }
            }
            miro_co_metadata.signers = signers_info.clone();

            // Deploy images

            // let (entrypoint_x_position, entrypoint_y_position) = (1300, 250);
            // let (handler_x_position, handler_y_position) = (2900, 1400);
            let (entrypoint_x_position, entrypoint_y_position) =
                MiroCodeOverhaulConfig::EntryPoint.get_positions();
            let (handler_x_position, handler_y_position) =
                MiroCodeOverhaulConfig::Handler.get_positions();

            match entrypoint_parser.handler.clone() {
                None => {}
                Some(handler_meta) => {
                    let handler_sc = handler_meta.to_source_code_parser(Some(
                        miro_command_functions::parse_screenshot_name(
                            &handler_meta.name,
                            &co_miro_frame.title,
                        ),
                    ));
                    let handler_image = handler_sc
                        .deploy_screenshot_to_miro_frame(
                            co_miro_frame.clone(),
                            handler_x_position,
                            handler_y_position,
                            SourceCodeScreenshotOptions {
                                include_path: true,
                                offset_to_start_line: true,
                                filter_comments: true,
                                font_size: None,
                                filters: None,
                                show_line_number: true,
                            },
                        )
                        .await
                        .change_context(CommandError)?;
                    miro_co_metadata.handler_image_id = handler_image.item_id;
                }
            }

            let entrypoint_function_image = entrypoint_parser
                .entry_point_function
                .to_source_code_parser(Some(miro_command_functions::parse_screenshot_name(
                    &entrypoint_parser.entry_point_function.name,
                    &co_miro_frame.title,
                )))
                .deploy_screenshot_to_miro_frame(
                    co_miro_frame.clone(),
                    entrypoint_x_position,
                    entrypoint_y_position,
                    SourceCodeScreenshotOptions {
                        include_path: false,
                        offset_to_start_line: true,
                        filter_comments: false,
                        font_size: None,
                        filters: None,
                        show_line_number: true,
                    },
                )
                .await
                .change_context(CommandError)?;

            miro_co_metadata.entry_point_image_id = entrypoint_function_image.item_id.clone();

            let validations_miro_image = co_parser
                .deploy_new_validations_image_for_miro_co_frame(co_miro_frame.clone())
                .await
                .change_context(CommandError)?;

            let ca_miro_image = co_parser
                .deploy_new_context_accounts_image_for_miro_co_frame(co_miro_frame.clone())
                .await
                .change_context(CommandError)?;

            miro_co_metadata.validations_image_id = validations_miro_image.item_id.clone();
            miro_co_metadata.context_accounts_image_id = ca_miro_image.item_id.clone();
            miro_co_metadata.images_deployed = true;

            miro_co_metadata
                .update_code_overhaul_metadata()
                .change_context(CommandError)?;

            GitCommit::UpdateMetadataJson {
                bat_metadata_commit: BatMetadataCommit::MiroMetadataCommit,
            }
            .create_commit()
            .change_context(CommandError)?;

            println!("Connecting signers to entrypoint");
            for signer_miro_ids in signers_info {
                batbelt::miro::connector::create_connector(
                    &signer_miro_ids.user_figure_id,
                    &signer_miro_ids.sticky_note_id,
                    None,
                )
                .await
                .change_context(CommandError)?;
                batbelt::miro::connector::create_connector(
                    &signer_miro_ids.sticky_note_id,
                    &miro_co_metadata.entry_point_image_id,
                    Some(ConnectorOptions {
                        start_x_position: "100%".to_string(),
                        start_y_position: "50%".to_string(),
                        end_x_position: "0%".to_string(),
                        end_y_position: "50%".to_string(),
                    }),
                )
                .await
                .change_context(CommandError)?;
            }

            println!("Connecting entrypoint screenshot to context accounts screenshot in Miro");
            batbelt::miro::connector::create_connector(
                &miro_co_metadata.entry_point_image_id,
                &miro_co_metadata.context_accounts_image_id,
                None,
            )
            .await
            .change_context(CommandError)?;

            println!("Connecting context accounts screenshot to validations screenshot in Miro");
            batbelt::miro::connector::create_connector(
                &miro_co_metadata.context_accounts_image_id,
                &miro_co_metadata.validations_image_id,
                None,
            )
            .await
            .change_context(CommandError)?;

            if !miro_co_metadata.handler_image_id.is_empty() {
                println!("validations screenshot to handler screenshot in Miro");
                batbelt::miro::connector::create_connector(
                    &miro_co_metadata.validations_image_id,
                    &miro_co_metadata.handler_image_id,
                    None,
                )
                .await
                .change_context(CommandError)?;
            }
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
            //             let mut_acc_source_code = SourceCodeParser::new(
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
            // fs::remove_file(handler_screenshot_path).unwrap();
            // fs::remove_file(co_screenshot_path).unwrap();
            // fs::remove_file(validations_screenshot_path).unwrap();
            // fs::remove_file(entrypoint_screenshot_path).unwrap();
            //
            //
            // create_git_commit(
            //     GitCommit::DeployMiro,
            //     Some(vec![selected_co_started_path.to_string()]),
            // )
            // .unwrap();
            // Ok(())
            // } else {
            //     update images
            //     let prompt_text = format!("select the images to update for {selected_folder}");
            //     let selections = batbelt::cli_inputs::multiselect(
            //         &prompt_text,
            //         CO_FIGURES.to_vec(),
            //         Some(&vec![true, true, true, true]),
            //     )?;
            //     if !selections.is_empty() {
            //         for selection in selections.iter() {
            //             let snapshot_path_vec = &snapshot_paths.clone().collect::<Vec<_>>();
            //             let snapshot_path = &snapshot_path_vec.as_slice()[*selection];
            //             let file_name = snapshot_path.split('/').last().unwrap();
            //             println!("Updating: {file_name}");
            //             let item_id =
            //                 batbelt::helpers::get::get_screenshot_id(file_name, &selected_co_started_path);
            //             let mut screenshot_image =
            //                 MiroImage::new_from_item_id(&item_id, MiroImageType::FromPath).await;
            //             screenshot_image.update_from_path(&snapshot_path).await;
            //         }
            //         create_git_commit(
            //             GitCommit::UpdateMiro,
            //             Some(vec![selected_folder.to_string()]),
            //         )?;
            //     } else {
            //         println!("No files selected");
            //     }
        } else {
            // update screenshots
            let options = vec![
                "Entrypoint function".to_string().bright_green(),
                "Context accounts".to_string().bright_yellow(),
                "Validations".to_string().bright_red(),
                "Handler function".to_string().bright_cyan(),
            ];
            let prompt_text = "Which screenshots you want to update?".to_string();
            let selections = BatDialoguer::multiselect(prompt_text, options.clone(), None, true)?;
            let co_parser = CodeOverhaulParser::new_from_entry_point_name(entrypoint_name.clone())
                .change_context(CommandError)?;
            let ep_parser =
                EntrypointParser::new_from_name(&entrypoint_name).change_context(CommandError)?;
            for selection in selections {
                match selection {
                    // Entrypoint
                    0 => {
                        let ep_sc_parser = ep_parser.entry_point_function.to_source_code_parser(
                            Some(miro_command_functions::parse_screenshot_name(
                                &ep_parser.entry_point_function.name,
                                &co_miro_frame.title,
                            )),
                        );
                        let ep_screenshot_path = ep_sc_parser
                            .create_screenshot(SourceCodeScreenshotOptions {
                                include_path: false,
                                offset_to_start_line: true,
                                filter_comments: false,
                                font_size: None,
                                filters: None,
                                show_line_number: true,
                            })
                            .change_context(CommandError)?;
                        let mut ep_image = MiroImage::new_from_item_id(
                            &miro_co_metadata.entry_point_image_id,
                            MiroImageType::FromPath,
                        )
                        .await
                        .change_context(CommandError)?;

                        println!(
                            "\nUpdating entrypoint screenshot in {} frame",
                            co_miro_frame.title.green()
                        );

                        ep_image
                            .update_from_path(&ep_screenshot_path)
                            .await
                            .change_context(CommandError)?;

                        fs::remove_file(&ep_screenshot_path)
                            .into_report()
                            .change_context(CommandError)?;
                    }
                    // Context accounts
                    1 => co_parser
                        .update_context_accounts_screenshot()
                        .await
                        .change_context(CommandError)?,
                    // Validations
                    2 => co_parser
                        .update_validations_screenshot()
                        .await
                        .change_context(CommandError)?,
                    // Handler function
                    3 => {
                        if ep_parser.handler.is_none() {
                            println!("No handler function");
                            continue;
                        }
                        let handler_sc_parser = ep_parser
                            .handler
                            .clone()
                            .unwrap()
                            .to_source_code_parser(Some(
                                miro_command_functions::parse_screenshot_name(
                                    &ep_parser.handler.clone().unwrap().name,
                                    &co_miro_frame.title,
                                ),
                            ));
                        let handler_screenshot_path = handler_sc_parser
                            .create_screenshot(SourceCodeScreenshotOptions {
                                include_path: true,
                                offset_to_start_line: true,
                                filter_comments: true,
                                font_size: None,
                                filters: None,
                                show_line_number: true,
                            })
                            .change_context(CommandError)?;
                        let mut handler_image = MiroImage::new_from_item_id(
                            &miro_co_metadata.handler_image_id,
                            MiroImageType::FromPath,
                        )
                        .await
                        .change_context(CommandError)?;

                        println!(
                            "\nUpdating handler screenshot in {} frame",
                            co_miro_frame.title.green()
                        );

                        handler_image
                            .update_from_path(&handler_screenshot_path)
                            .await
                            .change_context(CommandError)?;

                        fs::remove_file(&handler_screenshot_path)
                            .into_report()
                            .change_context(CommandError)?;
                    }
                    _ => {
                        unimplemented!()
                    }
                };
            }
        }
        Ok(())
    }
}

pub mod miro_command_functions {
    use super::*;

    pub async fn deploy_miro_frame_for_co(
        entry_point_name: &str,
        entry_point_index: usize,
    ) -> CommandResult<MiroFrame> {
        let frame_name = format!("co: {}", entry_point_name);

        println!("Creating frame in Miro for {}", entry_point_name.green());
        let mut miro_frame = MiroFrame::new(&frame_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
        miro_frame.deploy().await.change_context(CommandError)?;
        let x_modifier = entry_point_index as i64 % MIRO_BOARD_COLUMNS;
        let y_modifier = entry_point_index as i64 / MIRO_BOARD_COLUMNS;
        let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 100) * x_modifier;
        let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 100) * y_modifier;
        miro_frame
            .update_position(x_position, y_position)
            .await
            .change_context(CommandError)?;
        Ok(miro_frame)
    }

    pub fn get_formatted_path(name: String, path: String, start_line_index: usize) -> String {
        format!(
            "{}: {}:{}",
            name.blue(),
            path.trim_start_matches("../"),
            start_line_index
        )
    }

    pub async fn prompt_deploy_dependencies(
        parent_function: FunctionSourceCodeMetadata,
        parent_function_image: Option<MiroImage>,
        selected_miro_frame: MiroFrame,
        deployed_dependencies: &mut Vec<(MiroImage, FunctionSourceCodeMetadata)>,
        pending_to_check: &mut Vec<FunctionSourceCodeMetadata>,
    ) -> Result<(), CommandError> {
        let function_parser = parent_function
            .to_function_parser()
            .change_context(CommandError)?;

        let function_sc_options = SourceCodeScreenshotOptions {
            include_path: true,
            offset_to_start_line: true,
            filter_comments: false,
            font_size: None,
            filters: None,
            show_line_number: true,
        };

        let parent_function_miro_image = if parent_function_image.is_some() {
            parent_function_image.unwrap()
        } else {
            let parent_image = parent_function
                .to_source_code_parser(Some(miro_command_functions::parse_screenshot_name(
                    &parent_function.name,
                    &selected_miro_frame.title,
                )))
                .deploy_screenshot_to_miro_frame(
                    selected_miro_frame.clone(),
                    (selected_miro_frame.height as i64) / 2,
                    -(selected_miro_frame.width as i64) / 2,
                    function_sc_options.clone(),
                )
                .await
                .change_context(CommandError)?;
            deployed_dependencies.push((parent_image.clone(), parent_function.clone()));
            parent_image
        };

        if function_parser.clone().dependencies.is_empty() {
            println!(
                "Function {} does not have dependencies",
                function_parser.name.red()
            );
            return Ok(());
        }

        let bat_metadata = BatMetadata::read_metadata().change_context(CommandError)?;

        let function_dependencies = function_parser
            .dependencies
            .clone()
            .into_iter()
            .map(|f_meta| bat_metadata.source_code.get_function_by_id(f_meta))
            .collect::<Result<Vec<_>, MetadataError>>()
            .change_context(CommandError)?;

        let dependencies_names_vec = function_dependencies
            .clone()
            .into_iter()
            .map(|metadata| metadata.name)
            .collect::<Vec<_>>();

        let selected_function_content = parent_function
            .to_source_code_parser(None)
            .get_source_code_content()
            .lines()
            .map(|line| {
                if dependencies_names_vec
                    .clone()
                    .into_iter()
                    .any(|dep| line.contains(&dep))
                {
                    line.red()
                } else {
                    line.green()
                }
            })
            .collect::<Vec<ColoredString>>();

        println!("{} function:", parent_function.name.bright_blue());
        for line in selected_function_content {
            println!("{}", line);
        }

        let prompt_text = format!(
            "Select the dependencies to deploy for {}",
            parent_function.name.yellow(),
        );

        let formatted_option = function_dependencies
            .clone()
            .into_iter()
            .map(|dep| get_formatted_path(dep.name, dep.path.clone(), dep.start_line_index))
            .collect::<Vec<_>>();

        let multi_selection = BatDialoguer::multiselect(
            prompt_text,
            formatted_option.clone(),
            Some(&vec![true; formatted_option.clone().len()]),
            false,
        )?;

        let mut pending_to_deploy: Vec<FunctionSourceCodeMetadata> = vec![];
        let mut pending_to_connect: Vec<MiroImage> = vec![];

        for selection in multi_selection {
            let selected_dependency = function_dependencies[selection].clone();
            let already_deployed = deployed_dependencies
                .clone()
                .into_iter()
                .find(|dep| dep.1 == selected_dependency);
            if already_deployed.is_none() {
                pending_to_deploy.push(selected_dependency.clone());
            } else {
                pending_to_connect.push(already_deployed.unwrap().0);
            }
            pending_to_check.push(selected_dependency.clone());
        }

        while !pending_to_deploy.is_empty() {
            let dependency = pending_to_deploy.pop().unwrap();
            let dependency_image = dependency
                .to_source_code_parser(Some(miro_command_functions::parse_screenshot_name(
                    &dependency.name,
                    &selected_miro_frame.title,
                )))
                .deploy_screenshot_to_miro_frame(
                    selected_miro_frame.clone(),
                    (selected_miro_frame.height as i64) / 2,
                    (selected_miro_frame.width as i64) / 2,
                    function_sc_options.clone(),
                )
                .await
                .change_context(CommandError)?;
            batbelt::miro::connector::create_connector(
                &parent_function_miro_image.item_id,
                &dependency_image.item_id,
                None,
            )
            .await
            .change_context(CommandError)?;
            deployed_dependencies.push((dependency_image.clone(), dependency.clone()));
        }

        while !pending_to_connect.is_empty() {
            let dependency_image = pending_to_connect.pop().unwrap();
            batbelt::miro::connector::create_connector(
                &parent_function_miro_image.item_id,
                &dependency_image.item_id,
                None,
            )
            .await
            .change_context(CommandError)?;
        }
        Ok(())
    }

    pub fn parse_screenshot_name(name: &str, frame_title: &str) -> String {
        format!(
            "{}::frame={}",
            name,
            frame_title
                .replace([' ', '-'], "_")
                .to_screaming_snake_case()
        )
    }
}

#[test]
fn test_enum_display() {
    let bat_package_json_command = MiroCommand::get_package_json_commands("miro".to_string());
    println!("{:#?}", bat_package_json_command);
}
