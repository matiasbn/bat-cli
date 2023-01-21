use crate::config::*;
use crate::utils::git::*;
use normalize_url::normalizer;
use reqwest;
use serde_json::*;
use std::fs;
use std::result::Result;

use crate::utils::helpers;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::multipart::{self};
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

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

pub mod api {

    use super::*;
    pub mod frame {
        use super::*;

        #[derive(Debug)]
        pub struct MiroFrame {
            pub id: String,
            pub url: String,
        }

        // returns the frame url
        pub async fn create_frame(entrypoint_name: &str) -> Result<MiroFrame, String> {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let client = reqwest::Client::new();

            let board_response = client
                .post(format!("https://api.miro.com/v2/boards/{board_id}/frames"))
                .body(
                    json!({
                         "data": {
                              "format": "custom",
                              "title": entrypoint_name,
                              "type": "freeform"
                         },
                         "position": {
                              "origin": "center",
                              "x": 0,
                              "y": 0
                         },
                         "geometry": {
                            "width": MIRO_FRAME_WIDTH,
                            "height": MIRO_FRAME_HEIGHT
                       }
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await;
            match board_response {
                Ok(response) => {
                    let frame_id = super::helpers::get_id_from_response(response).await;
                    let frame_url = MiroConfig::new().get_frame_url(&frame_id);
                    Ok(MiroFrame {
                        id: frame_id,
                        url: frame_url,
                    })
                }
                Err(err_message) => Err(err_message.to_string()),
            }
        }
        pub async fn get_frame_positon(frame_id: String) -> (u64, u64) {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let client = reqwest::Client::new();
            let board_response = client
                .get(format!(
                    "https://api.miro.com/v2/boards/{board_id}/frames/{frame_id}"
                ))
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(board_response.as_str()).unwrap();
            let x_position = response["position"]["x"].clone();
            let y_position = response["position"]["y"].clone();
            (
                x_position.as_f64().unwrap() as u64,
                y_position.as_f64().unwrap() as u64,
            )
        }
        pub async fn update_frame_position(
            entrypoint_name: String,
            co_finished_files: i32,
        ) -> Result<(), String> {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let frame_id = super::helpers::get_frame_id_from_co_file(entrypoint_name.as_str())?;
            let x_modifier = co_finished_files % MIRO_BOARD_COLUMNS;
            let y_modifier = co_finished_files / MIRO_BOARD_COLUMNS;
            let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH + 100) * x_modifier;
            let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT + 100) * y_modifier;
            let client = reqwest::Client::new();
            let _response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{board_id}/frames/{frame_id}",
                ))
                .body(
                    json!({
                        "position": {
                            "x": x_position,
                            "y": y_position,
                            "origin": "center",
                        },
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            Ok(())
            // println!("update frame position response: {response}")
        }
    }
    pub mod image {
        use super::*;

        // uploads the image in file_path to the board
        pub async fn create_image_from_device(
            file_path: String,
            entrypoint_name: &str,
        ) -> Result<String, String> {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let file_name = file_path.clone().split('/').last().unwrap().to_string();
            let file = File::open(file_path.clone()).await.unwrap();
            // read file body stream
            let stream = FramedRead::new(file, BytesCodec::new());
            let file_body = reqwest::Body::wrap_stream(stream);

            //make form part of file
            let some_file = multipart::Part::stream(file_body)
                .file_name(file_name.clone())
                .mime_str("text/plain")
                .unwrap();
            //create the multipart form
            let form = multipart::Form::new().part("resource", some_file);
            let client = reqwest::Client::new();
            let response = client
                .post(format!("https://api.miro.com/v2/boards/{board_id}/images"))
                .multipart(form)
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            let id = response["id"].to_string().replace("\"", "");
            super::item::update_screenshot_position(
                entrypoint_name.to_string(),
                &file_name,
                id.clone(),
            )
            .await;
            Ok(id)
        }

        pub async fn update_image_from_device(file_path: String, item_id: &str) {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let file_name = file_path.clone().split('/').last().unwrap().to_string();
            let file = File::open(file_path.clone()).await.unwrap();
            // read file body stream
            let stream = FramedRead::new(file, BytesCodec::new());
            let file_body = reqwest::Body::wrap_stream(stream);

            //make form part of file
            let some_file = multipart::Part::stream(file_body)
                .file_name(file_name.clone())
                .mime_str("text/plain")
                .unwrap();
            //create the multipart form
            let form = multipart::Form::new().part("resource", some_file);
            let client = reqwest::Client::new();
            let _response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{board_id}/images/{item_id}"
                ))
                .multipart(form)
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
        }

        pub async fn create_user_figure_for_signer(
            signer_counter: usize,
            miro_frame_id: String,
        ) -> String {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let response = client
                .post(format!("https://api.miro.com/v2/boards/{board_id}/images",))
                .body(
                    json!({
                        "data": {
                            "url": "https://mirostatic.com/app/static/12079327f83ff492.svg"
                       },
                       "position": {
                            "origin": "center",
                            "x": 150,
                            "y": y_position
                       },
                       "geometry": {
                            "height": 200.1
                       },
                       "parent": {
                            "id": miro_frame_id
                       }
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            let id = response["id"].to_string().replace("\"", "");
            id
        }
    }
    pub mod item {
        use super::*;

        pub async fn update_screenshot_position(
            entrypoint_name: String,
            file_name: &str,
            item_id: String,
        ) -> Result<(), String> {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let frame_id = super::helpers::get_frame_id_from_co_file(entrypoint_name.as_str())?;
            // let started_file_path
            let (x_position, y_position) = match file_name.to_string().as_str() {
                ENTRYPOINT_PNG_NAME => (1300, 250),
                CONTEXT_ACCOUNTS_PNG_NAME => (2200, 350),
                VALIDATIONS_PNG_NAME => (3000, 500),
                HANDLER_PNG_NAME => (2900, 1400),
                _ => todo!(),
            };
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let _response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{board_id}/images/{item_id}",
                ))
                .body(
                    json!({
                        "parent": {
                            "id": frame_id
                        },
                        "position": {
                            "x": x_position,
                            "y": y_position,
                            "origin": "center",
                        },
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            Ok(())
        }
    }
    pub mod sticky_note {
        use crate::structs::SignerType;

        use super::*;
        pub async fn create_signer_sticky_note(
            signer_note_text: String,
            signer_counter: usize,
            miro_frame_id: String,
            signer_type: SignerType,
        ) -> String {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let fill_color = match signer_type {
                SignerType::Validated => "red",
                SignerType::NotValidated => "dark_blue",
                SignerType::NotSigner => "gray",
            };
            let response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{board_id}/sticky_notes",
                ))
                .body(
                    json!({
                        "data": {
                            "content": signer_note_text,
                            "shape": "rectangle"
                        },
                        "style": {
                            "fillColor": fill_color
                        },
                        "position": {
                            "origin": "center",
                            "x": 550,
                            "y": y_position
                        },
                        "geometry": {
                            "width": 374.5
                        },
                        "parent": {
                            "id": miro_frame_id
                        }
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            // println!("sticky not response {response}");
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            let id = response["id"].to_string().replace("\"", "");
            id
        }
        pub async fn create_user_figure_for_signer(
            signer_counter: usize,
            miro_frame_id: String,
        ) -> String {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let response = client
                .post(format!("https://api.miro.com/v2/boards/{board_id}/images",))
                .body(
                    json!({
                        "data": {
                            "url": "https://mirostatic.com/app/static/12079327f83ff492.svg"
                       },
                       "position": {
                            "origin": "center",
                            "x": 150,
                            "y": y_position
                       },
                       "geometry": {
                            "height": 200.1
                       },
                       "parent": {
                            "id": miro_frame_id
                       }
                    })
                    .to_string(),
                )
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            let id = response["id"].to_string().replace("\"", "");
            id
        }
    }
    pub mod connector {
        use super::*;
        #[derive(Debug)]
        pub struct ConnectorOptions {
            pub start_x_position: String,
            pub start_y_position: String,
            pub end_x_position: String,
            pub end_y_position: String,
        }

        pub async fn create_connector(
            start_item_id: &str,
            end_item_id: &str,
            connect_options: Option<ConnectorOptions>,
        ) {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let body = if let Some(options) = connect_options {
                let ConnectorOptions {
                    start_x_position,
                    start_y_position,
                    end_x_position,
                    end_y_position,
                } = options;
                json!({
                    "startItem": {
                        "id": start_item_id,
                        "position": {
                            "x": start_x_position,
                            "y": start_y_position,
                        },
                    },
                    "endItem": {
                        "id": end_item_id,
                        "position": {
                            "x": end_x_position,
                            "y": end_y_position,
                        },
                    },
                    "style": {
                         "strokeWidth": "3"
                    },
                   "shape": "elbowed"
                })
                .to_string()
            } else {
                json!({
                    "startItem": {
                        "id": start_item_id
                   },
                   "endItem": {
                        "id": end_item_id
                   },
                   "style": {
                        "strokeWidth": "3"
                   },
                   "shape": "elbowed"
                })
                .to_string()
            };
            let client = reqwest::Client::new();
            let _response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{board_id}/connectors",
                ))
                .body(body)
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            // println!("connector response {response}");
        }
    }
    pub mod helpers {
        use super::*;
        pub async fn get_id_from_response(response: reqwest::Response) -> String {
            let respons_string = response.text().await.unwrap();
            let response: Value = serde_json::from_str(&&respons_string.as_str()).unwrap();
            response["id"].to_string().replace("\"", "")
        }
        pub fn get_frame_id_from_co_file(entrypoint_name: &str) -> Result<String, String> {
            let started_file_path = BatConfig::get_auditor_code_overhaul_started_path(Some(
                entrypoint_name.to_string(),
            ))?;
            let miro_url = fs::read_to_string(started_file_path)
                .unwrap()
                .lines()
                .find(|line| line.contains("https://miro.com/app/board/"))
                .unwrap()
                .to_string();
            let frame_id = miro_url
                .split("moveToWidget=")
                .last()
                .unwrap()
                .to_string()
                .replace("\"", "");
            Ok(frame_id)
        }
    }
}

pub mod commands {
    use colored::Colorize;

    use crate::{
        commands::miro::api::connector::ConnectorOptions,
        structs::{SignerInfo, SignerType},
        utils,
    };

    use super::*;
    pub async fn deploy_miro() -> Result<(), String> {
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
        let selection = utils::cli_inputs::select(&prompt_text, started_folders.clone(), None)?;

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
        let is_deploying =
            to_start_file_content.contains(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER);
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
            let miro_frame = super::api::frame::create_frame(selected_folder)
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
                let sticky_note_id = super::api::sticky_note::create_signer_sticky_note(
                    signer.signer_text.clone(),
                    signer_index,
                    miro_frame.id.clone(),
                    signer.signer_type,
                )
                .await;
                let user_figure_id = super::api::image::create_user_figure_for_signer(
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
                let id = super::api::image::create_image_from_device(
                    screenshot_path.to_string(),
                    &selected_folder,
                )
                .await?;
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
            let handler_id =
                helpers::get::get_screenshot_id(&HANDLER_PNG_NAME, &started_co_file_path);
            println!("Connecting signers to entrypoint");
            for signer_miro_ids in signers_info {
                super::api::connector::create_connector(
                    &signer_miro_ids.user_figure_id,
                    &signer_miro_ids.sticky_note_id,
                    None,
                )
                .await;
                super::api::connector::create_connector(
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
            super::api::connector::create_connector(&entrypoint_id, &context_accounts_id, None)
                .await;
            super::api::connector::create_connector(&context_accounts_id, &validations_id, None)
                .await;
            super::api::connector::create_connector(&validations_id, &handler_id, None).await;
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
                    let screenshot_path_vec = &screenshot_paths.clone().collect::<Vec<_>>();
                    let screenshot_path = &screenshot_path_vec.as_slice()[*selection];
                    let file_name = screenshot_path.split('/').last().unwrap();
                    println!("Updating: {file_name}");
                    let item_id = helpers::get::get_screenshot_id(file_name, &started_co_file_path);
                    super::api::image::update_image_from_device(
                        screenshot_path.to_string(),
                        &item_id,
                    )
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
}
