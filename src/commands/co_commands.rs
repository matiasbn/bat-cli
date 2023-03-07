use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::{execute_command, CodeEditor};
use crate::batbelt::git::{deprecated_check_correct_branch, GitCommit};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};

use crate::{batbelt, Suggestion};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{FutureExt, Report, ResultExt};

use crate::batbelt::metadata::miro_metadata::{SignerInfo, SignerType};
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, MiroMetadata};
use crate::batbelt::miro::connector::ConnectorOptions;
use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::image::MiroImage;

use crate::batbelt::miro::sticky_note::MiroStickyNote;
use crate::batbelt::miro::MiroConfig;
use crate::batbelt::parser::code_overhaul_parser::CodeOverhaulParser;
use crate::batbelt::parser::source_code_parser::SourceCodeScreenshotOptions;
use crate::commands::miro_commands::MiroCommand;

use std::fs;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum CodeOverhaulCommand {
    /// Starts a code-overhaul file audit
    #[default]
    Start,
    /// Moves the code-overhaul file from to-review to finished
    Finish,
    /// Deploy the Miro diagram
    DeployMiro,
}

impl BatEnumerator for CodeOverhaulCommand {}

impl BatCommandEnumerator for CodeOverhaulCommand {
    fn execute_command(&self) -> CommandResult<()> {
        unimplemented!()
    }

    fn check_metadata_is_initialized(&self) -> bool {
        true
    }

    fn check_correct_branch(&self) -> bool {
        true
    }
}

impl CodeOverhaulCommand {
    pub async fn execute_command(&self) -> CommandResult<()> {
        match self {
            CodeOverhaulCommand::Start => self.start_co_file(),
            CodeOverhaulCommand::Finish => self.finish_co_file(),
            CodeOverhaulCommand::DeployMiro => self.deploy_diagram().await,
        }
    }

    async fn deploy_diagram(&self) -> CommandResult<()> {
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
                "bat-cli miro deploy-co".green()
            );
            return Err(Report::new(CommandError)
                .attach_printable(message)
                .attach(Suggestion(suggestion_message)));
        }

        let co_started_bat_folder = BatFolder::CodeOverhaulStarted;
        let started_files_names = co_started_bat_folder
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;
        let prompt_text = "Select the co file to deploy to Miro".to_string();
        let selection = BatDialoguer::select(prompt_text, started_files_names.clone(), None)?;
        let selected_file_name = started_files_names[selection].clone();
        let entrypoint_name = selected_file_name.trim_end_matches(".md").to_string();

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
                        format!("Validated signer:\n\n <strong>{}</strong>", signer.name)
                    } else {
                        format!("Not validated signer:\n\n <strong>{}</strong>", signer.name)
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
                        MiroCommand::parse_screenshot_name(
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
                .to_source_code_parser(Some(MiroCommand::parse_screenshot_name(
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
                .get_validations_image_for_miro_co_frame(co_miro_frame.clone())
                .await
                .change_context(CommandError)?;

            let ca_miro_image = co_parser
                .get_context_accounts_image_for_miro_co_frame(co_miro_frame.clone())
                .await
                .change_context(CommandError)?;

            miro_co_metadata.validations_image_id = validations_miro_image.item_id.clone();
            miro_co_metadata.context_accounts_image_id = ca_miro_image.item_id.clone();
            miro_co_metadata.images_deployed = true;

            miro_co_metadata
                .update_code_overhaul_metadata()
                .change_context(CommandError)?;

            GitCommit::UpdateMetadataJson
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
        Ok(())
    }

    fn finish_co_file(&self) -> error_stack::Result<(), CommandError> {
        deprecated_check_correct_branch().change_context(CommandError)?;
        // get to-review files
        let started_entrypoints = BatFolder::CodeOverhaulStarted
            .get_all_files_dir_entries(true, None, None)
            .change_context(CommandError)?;
        let started_entrypoints_names = started_entrypoints
            .into_iter()
            .map(|dir_entry| dir_entry.file_name().to_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let prompt_text = "Select the code-overhaul to finish:";
        let selection = BatDialoguer::select(
            prompt_text.to_string(),
            started_entrypoints_names.clone(),
            None,
        )
        .change_context(CommandError)?;

        let finished_endpoint = started_entrypoints_names[selection].clone();
        let finished_co_folder_path = BatFolder::CodeOverhaulFinished
            .get_path(true)
            .change_context(CommandError)?;
        let started_co_file_path = BatFile::CodeOverhaulStarted {
            file_name: finished_endpoint.clone(),
        }
        .get_path(true)
        .change_context(CommandError)?;
        self.check_code_overhaul_file_completed(started_co_file_path.clone())?;
        execute_command(
            "mv",
            &[&started_co_file_path, &finished_co_folder_path],
            false,
        )
        .change_context(CommandError)?;
        GitCommit::FinishCO {
            entrypoint_name: finished_endpoint.clone(),
        }
        .create_commit()
        .change_context(CommandError)?;

        println!("{} moved to finished", finished_endpoint.green());
        Ok(())
    }

    fn check_code_overhaul_file_completed(
        &self,
        file_path: String,
    ) -> error_stack::Result<(), CommandError> {
        let file_data = fs::read_to_string(file_path).unwrap();
        if file_data.contains(
            &CoderOverhaulTemplatePlaceholders::CompleteWithTheRestOfStateChanges.to_placeholder(),
        ) {
            return Err(Report::new(CommandError).attach_printable(
                "Please complete the \"What it does?\" section of the {file_name} file",
            ));
        }

        if file_data
            .contains(&CoderOverhaulTemplatePlaceholders::CompleteWithNotes.to_placeholder())
        {
            let user_decided_to_continue = batbelt::bat_dialoguer::select_yes_or_no(
                "Notes section not completed, do you want to proceed anyway?",
            )
            .change_context(CommandError)?;
            if !user_decided_to_continue {
                return Err(Report::new(CommandError).attach_printable("Aborted by the user"));
            }
        }

        if file_data.contains(
            &CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription.to_placeholder(),
        ) {
            return Err(Report::new(CommandError).attach_printable(
                "Please complete the \"Signers\" section of the {file_name} file",
            ));
        }

        if file_data
            .contains(&CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder())
        {
            let user_decided_to_continue = batbelt::bat_dialoguer::select_yes_or_no(
                "Validations section not completed, do you want to proceed anyway?",
            )
            .change_context(CommandError)?;
            if !user_decided_to_continue {
                return Err(Report::new(CommandError).attach_printable("Aborted by the user"));
            }
        }

        if file_data
            .contains(&CoderOverhaulTemplatePlaceholders::CompleteWithMiroFrameUrl.to_placeholder())
        {
            return Err(Report::new(CommandError).attach_printable(
                "Please complete the \"Miro board frame\" section of the {file_name} file",
            ));
        }
        Ok(())
    }

    fn start_co_file(&self) -> error_stack::Result<(), CommandError> {
        let review_files = BatFolder::CodeOverhaulToReview
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;

        if review_files.is_empty() {
            return Err(Report::new(CommandError)
                .attach_printable("no to-review files in code-overhaul folder"));
        }
        let prompt_text = "Select the code-overhaul file to start:";
        let selection = BatDialoguer::select(prompt_text.to_string(), review_files.clone(), None)
            .change_context(CommandError)?;

        // user select file
        let to_start_file_name = &review_files[selection].clone();
        let entrypoint_name = to_start_file_name.trim_end_matches(".md");

        BatFile::CodeOverhaulToReview {
            file_name: to_start_file_name.clone(),
        }
        .remove_file()
        .change_context(CommandError)?;

        let started_bat_file = BatFile::CodeOverhaulStarted {
            file_name: to_start_file_name.clone(),
        };

        let started_template =
            CodeOverhaulTemplate::new(entrypoint_name, true).change_context(CommandError)?;

        let started_markdown_content = started_template
            .get_markdown_content()
            .change_context(CommandError)?;

        started_bat_file
            .write_content(false, &started_markdown_content)
            .change_context(CommandError)?;

        println!("{to_start_file_name} file moved to started");

        GitCommit::StartCO {
            entrypoint_name: to_start_file_name.clone(),
        }
        .create_commit()
        .change_context(CommandError)?;

        started_bat_file
            .open_in_editor(true, None)
            .change_context(CommandError)?;

        // open instruction file in VSCode
        if started_template.entrypoint_parser.is_some() {
            let ep_parser = started_template.entrypoint_parser.unwrap();
            if ep_parser.handler.is_some() {
                let handler = ep_parser.handler.unwrap();
                CodeEditor::open_file_in_editor(&handler.path, Some(handler.start_line_index))?;
            }
            CodeEditor::open_file_in_editor(
                &ep_parser.entry_point_function.path,
                Some(ep_parser.entry_point_function.start_line_index),
            )?;
        }
        Ok(())
    }
}
