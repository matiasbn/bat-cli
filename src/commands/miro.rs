use crate::config::*;
use crate::utils::git::*;
use normalize_url::normalizer;
use reqwest;
use serde_json::*;
use std::result::Result;
use std::{fmt, fs};

use crate::utils;
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
        // pub async fn get_frame_positon(frame_id: String) -> (u64, u64) {
        //     let MiroConfig {
        //         access_token,
        //         board_id,
        //         ..
        //     } = MiroConfig::new();
        //     let client = reqwest::Client::new();
        //     let board_response = client
        //         .get(format!(
        //             "https://api.miro.com/v2/boards/{board_id}/frames/{frame_id}"
        //         ))
        //         .header(CONTENT_TYPE, "application/json")
        //         .header(AUTHORIZATION, format!("Bearer {access_token}"))
        //         .send()
        //         .await
        //         .unwrap()
        //         .text()
        //         .await
        //         .unwrap();
        //     let response: Value = serde_json::from_str(board_response.as_str()).unwrap();
        //     let x_position = response["position"]["x"].clone();
        //     let y_position = response["position"]["y"].clone();
        //     (
        //         x_position.as_f64().unwrap() as u64,
        //         y_position.as_f64().unwrap() as u64,
        //     )
        // }
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
            super::item::update_snapshot_position(
                entrypoint_name.to_string(),
                &file_name,
                id.clone(),
            )
            .await?;
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

        pub async fn update_snapshot_position(
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

        pub async fn get_items_on_board(
            miro_item_type: Option<MiroItemType>,
        ) -> Result<Value, String> {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            let client = reqwest::Client::new();
            let url = if let Some(item_type) = miro_item_type {
                let item_type_string = item_type.str_item_type().unwrap();
                format!(
                    "https://api.miro.com/v2/boards/{board_id}/items?limit=50&type={}",
                    item_type_string
                )
            } else {
                format!("https://api.miro.com/v2/boards/{board_id}/items?limit=50",)
            };
            let response = client
                .get(url)
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            Ok(response)
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
            let started_file_path = utils::path::get_auditor_code_overhaul_started_file_path(
                Some(entrypoint_name.to_string()),
            )?;
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
    use super::*;
    use colored::Colorize;

    use crate::{
        commands::miro::api::connector::ConnectorOptions,
        structs::{SignerInfo, SignerType},
        utils::{
            self,
            helpers::get::{
                get_string_between_two_index_from_string, get_string_between_two_str_from_string,
            },
        },
    };

    use super::*;
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
        let is_deploying =
            to_start_file_content.contains(CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER);
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
            let miro_frame = super::api::frame::create_frame(&selected_folder)
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
                let id = super::api::image::create_image_from_device(
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
            let entrypoint_id = utils::helpers::get::get_screenshot_id(
                &ENTRYPOINT_PNG_NAME,
                &selected_co_started_path,
            );
            let context_accounts_id = utils::helpers::get::get_screenshot_id(
                &CONTEXT_ACCOUNTS_PNG_NAME,
                &selected_co_started_path,
            );
            let validations_id = utils::helpers::get::get_screenshot_id(
                &VALIDATIONS_PNG_NAME,
                &selected_co_started_path,
            );
            let handler_id = utils::helpers::get::get_screenshot_id(
                &HANDLER_PNG_NAME,
                &selected_co_started_path,
            );
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
            println!("Connecting snapshots in Miro");
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
                    let snapshot_path_vec = &snapshot_paths.clone().collect::<Vec<_>>();
                    let snapshot_path = &snapshot_path_vec.as_slice()[*selection];
                    let file_name = snapshot_path.split('/').last().unwrap();
                    println!("Updating: {file_name}");
                    let item_id = utils::helpers::get::get_screenshot_id(
                        file_name,
                        &selected_co_started_path,
                    );
                    super::api::image::update_image_from_device(snapshot_path.to_string(), &item_id)
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
        let selected_co_started_file_path =
            utils::path::get_auditor_code_overhaul_started_file_path(Some(
                selected_folder.clone(),
            ))?;
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

    mod helpers {
        use super::*;
        pub async fn get_accounts_frame_id() -> Result<String, String> {
            let response = super::api::item::get_items_on_board(Some(MiroItemType::Frame)).await?;
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
                    end_entrypoint_index + 1,
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
    }
}
