use crate::config::*;
use normalize_url::normalizer;
use reqwest;
use serde_json::*;
use std::result::Result;
use std::{fs, io};

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
        ) -> super::io::Result<()> {
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
        pub async fn create_image_from_device(file_path: String, entrypoint_name: &str) -> String {
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
            id
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
        ) -> super::io::Result<()> {
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
        use super::*;
        pub async fn create_signer_sticky_note(
            signer_note_text: String,
            signer_counter: usize,
            miro_frame_id: String,
            validated_signer: bool,
        ) -> String {
            let MiroConfig {
                access_token,
                board_id,
                ..
            } = MiroConfig::new();
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let fill_color = if validated_signer { "red" } else { "dark_blue" };
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
        pub fn get_frame_id_from_co_file(entrypoint_name: &str) -> super::io::Result<String> {
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
