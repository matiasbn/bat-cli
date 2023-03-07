use crate::batbelt;
use std::fs;

use colored::{ColoredString, Colorize};

use crate::batbelt::metadata::functions_source_code_metadata::{
    FunctionMetadataType, FunctionSourceCodeMetadata,
};
use crate::batbelt::metadata::{
    BatMetadata, BatMetadataParser, BatMetadataType, MetadataError, MiroMetadata,
    SourceCodeMetadata,
};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;

use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::miro::connector::{create_connector, ConnectorOptions};
use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::miro::{MiroConfig, MiroItemType};

use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::miro::image::MiroImage;

use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::miro_metadata::{MiroCodeOverhaulMetadata, SignerInfo, SignerType};
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::parser::source_code_parser::{SourceCodeParser, SourceCodeScreenshotOptions};

use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CoderOverhaulTemplatePlaceholders,
};
use crate::batbelt::{silicon, BatEnumerator};
use crate::commands::{BatCommandEnumerator, CommandResult};
use clap::Subcommand;
use error_stack::{FutureExt, IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use regex::Regex;

use super::CommandError;
use crate::batbelt::miro::frame::{
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::config::BatAuditorConfig;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum MiroCommand {
    /// Creates the code-overhaul frames
    #[default]
    CodeOverhaulFrames,
    /// Deploys the entrypoint, context accounts and handler to a Miro frame
    Entrypoint {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
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
        return match self {
            MiroCommand::CodeOverhaulFrames => self.code_overhaul_action().await,
            MiroCommand::Entrypoint { select_all, sorted } => {
                self.entrypoint_action(*select_all, *sorted).await
            }
            MiroCommand::Metadata { select_all } => self.metadata_action(*select_all).await,
            MiroCommand::FunctionDependencies { select_all } => {
                self.function_action(*select_all).await
            }
        };
    }

    async fn entrypoint_action(&self, select_all: bool, sorted: bool) -> Result<(), CommandError> {
        let selected_miro_frame = MiroFrame::prompt_select_frame()
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
                Self::parse_screenshot_name(
                    &entrypoint.entry_point_function.name,
                    &selected_miro_frame.title,
                ),
            ));
            let ca_source_code = entrypoint.context_accounts.to_source_code_parser(Some(
                Self::parse_screenshot_name(
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
                let handler_source_code =
                    entrypoint_handler.to_source_code_parser(Some(Self::parse_screenshot_name(
                        &entrypoint_handler.name,
                        &selected_miro_frame.title,
                    )));
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

    async fn metadata_action(&self, select_all: bool) -> Result<(), CommandError> {
        let selected_miro_frame = MiroFrame::prompt_select_frame()
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
                                    Self::parse_screenshot_name(
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
                                    Self::parse_screenshot_name(
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

    async fn function_action(&self, _select_all: bool) -> Result<(), CommandError> {
        let selected_miro_frame = MiroFrame::prompt_select_frame()
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
                    self.get_formatted_path(
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
                self.prompt_deploy_dependencies(
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

    async fn code_overhaul_action(&self) -> Result<(), CommandError> {
        println!("Deploying code-overhaul frames to the Miro board");

        let miro_board_frames = MiroFrame::get_frames_from_miro()
            .await
            .change_context(CommandError)?;

        let entrypoints_names =
            EntrypointParser::get_entrypoint_names(false).change_context(CommandError)?;

        for (entrypoint_index, entrypoint_name) in entrypoints_names.iter().enumerate() {
            let frame_name = format!("co: {}", entrypoint_name);
            let frame_already_deployed = miro_board_frames
                .iter()
                .find(|frame| &frame.title == entrypoint_name);

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

            match frame_already_deployed {
                Some(frame) => {
                    miro_co_metadata.miro_frame_id = frame.item_id.clone();
                }
                None => {
                    println!("Creating frame in Miro for {}", entrypoint_name.green());
                    let mut miro_frame =
                        MiroFrame::new(entrypoint_name, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, 0, 0);
                    miro_frame.deploy().await.change_context(CommandError)?;
                    let x_modifier = entrypoint_index as i64 % MIRO_BOARD_COLUMNS;
                    let y_modifier = entrypoint_index as i64 / MIRO_BOARD_COLUMNS;
                    let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH as i64 + 100) * x_modifier;
                    let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT as i64 + 100) * y_modifier;
                    miro_frame
                        .update_position(x_position, y_position)
                        .await
                        .change_context(CommandError)?;
                    miro_co_metadata.miro_frame_id = miro_frame.item_id.clone();
                }
            }

            miro_co_metadata
                .update_code_overhaul_metadata()
                .change_context(CommandError)?;
        }

        Ok(())
    }

    fn get_formatted_path(&self, name: String, path: String, start_line_index: usize) -> String {
        format!(
            "{}: {}:{}",
            name.blue(),
            path.trim_start_matches("../"),
            start_line_index
        )
    }

    async fn prompt_deploy_dependencies(
        &self,
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
                .to_source_code_parser(Some(Self::parse_screenshot_name(
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
            .map(|dep| self.get_formatted_path(dep.name, dep.path.clone(), dep.start_line_index))
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
                .to_source_code_parser(Some(Self::parse_screenshot_name(
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

// #[test]
// fn test_screaming_snake_case() {
//     let function_name = "handle_thing";
//     let frame_name = "points-store actors";
//     let expected_output = "handle_thing-frame:POINTS_STORE_ACTORS";
//     println!("{}", parse_screenshot_name(function_name, frame_name));
//     assert_eq!(
//         parse_screenshot_name(function_name, frame_name),
//         expected_output,
//         "incorrect output"
//     )
// }
