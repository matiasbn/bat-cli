pub struct MiroFrame {
    pub id: String,
    pub url: String,
}

pub mod miro_api {
    use crate::config::BatConfig;

    pub fn miro_enabled() -> bool {
        !BatConfig::get_validated_config()
            .auditor
            .miro_oauth_access_token
            .is_empty()
    }

    pub mod board {
        use serde_json::Value;

        use reqwest::header::{ACCEPT, AUTHORIZATION};

        use crate::config::{AuditorConfig, BatConfig, RequiredConfig};

        pub async fn get_board() -> Value {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let client = reqwest::Client::new();
            let board_response = client
                .get(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}
            "
                ))
                .header(ACCEPT, "application/json")
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(board_response.as_str()).unwrap();
            response
        }
    }
    pub mod frame {
        #[derive(Debug)]
        pub struct ConnectorOptions {
            pub start_x_position: String,
            pub start_y_position: String,
            pub end_x_position: String,
            pub end_y_position: String,
        }

        use normalize_url::normalizer;
        use reqwest::Body;
        use serde_json::{json, Value};
        use std::fs;

        use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
        use reqwest::multipart::{self};
        use tokio::fs::File;
        use tokio_util::codec::{BytesCodec, FramedRead};

        use crate::constants::*;
        use crate::{
            commands::miro::MiroFrame,
            config::{AuditorConfig, BatConfig, RequiredConfig},
        };

        // returns the frame url
        pub async fn create_frame(entrypoint_name: &str) -> MiroFrame {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let client = reqwest::Client::new();

            let board_response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/frames"
                ))
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
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(board_response.as_str()).unwrap();
            let RequiredConfig { miro_board_url, .. } = BatConfig::get_validated_config().required;
            let frame_id: String = response["id"].clone().to_string().replace('\"', "");
            let frame_url = normalizer::UrlNormalizer::new(
                format!("{miro_board_url}/?moveToWidget={frame_id}").as_str(),
            )
            .unwrap()
            .normalize(None)
            .unwrap();
            MiroFrame {
                id: frame_id,
                url: frame_url,
            }
        }

        pub async fn get_frame_positon(frame_id: String) -> (u64, u64) {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let client = reqwest::Client::new();
            let board_response = client
                .get(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/frames/{frame_id}"
                ))
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
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
        // uploads the image in file_path to the board
        pub async fn create_image_from_device(file_path: String, entrypoint_name: &str) -> String {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let file_name = file_path.clone().split('/').last().unwrap().to_string();
            let file = File::open(file_path.clone()).await.unwrap();
            // read file body stream
            let stream = FramedRead::new(file, BytesCodec::new());
            let file_body = Body::wrap_stream(stream);

            //make form part of file
            let some_file = multipart::Part::stream(file_body)
                .file_name(file_name.clone())
                .mime_str("text/plain")
                .unwrap();
            //create the multipart form
            let form = multipart::Form::new().part("resource", some_file);
            let client = reqwest::Client::new();
            let response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/images"
                ))
                .multipart(form)
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let response: Value = serde_json::from_str(&response.as_str()).unwrap();
            let id = response["id"].to_string().replace("\"", "");
            update_item_position(entrypoint_name.to_string(), &file_name, id.clone()).await;
            id
        }
        pub async fn update_image_from_device(file_path: String, item_id: &str) {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let file_name = file_path.clone().split('/').last().unwrap().to_string();
            let file = File::open(file_path.clone()).await.unwrap();
            // read file body stream
            let stream = FramedRead::new(file, BytesCodec::new());
            let file_body = Body::wrap_stream(stream);

            //make form part of file
            let some_file = multipart::Part::stream(file_body)
                .file_name(file_name.clone())
                .mime_str("text/plain")
                .unwrap();
            //create the multipart form
            let form = multipart::Form::new().part("resource", some_file);
            let client = reqwest::Client::new();
            let response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/images/{item_id}"
                ))
                .multipart(form)
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
        }
        pub async fn update_item_position(
            entrypoint_name: String,
            file_name: &str,
            item_id: String,
        ) {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            let frame_id = get_frame_id(entrypoint_name.as_str());
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
            let response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/images/{item_id}",
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
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
        }

        pub async fn update_frame_position(entrypoint_name: String, co_finished_files: usize) {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_validated_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_validated_config().auditor;
            let frame_id = get_frame_id(entrypoint_name.as_str());
            let x_modifier = co_finished_files % MIRO_BOARD_COLUMNS;
            let y_modifier = co_finished_files / MIRO_BOARD_COLUMNS;
            let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH + 100) * x_modifier;
            let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT + 100) * y_modifier;
            let client = reqwest::Client::new();
            let response = client
                .patch(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/frames/{frame_id}",
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
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            println!("update frame position response: {response}")
        }

        pub async fn create_connector(
            start_item_id: &str,
            end_item_id: &str,
            connect_options: Option<ConnectorOptions>,
        ) {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
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
            let response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/connectors",
                ))
                .body(body)
                .header(CONTENT_TYPE, "application/json")
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            // println!("connector response {response}");
        }
        fn get_frame_id(entrypoint_name: &str) -> String {
            let started_file_path = BatConfig::get_auditor_code_overhaul_started_path(Some(
                entrypoint_name.to_string(),
            ));
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
            frame_id
        }
        pub async fn create_signer_sticky_note(
            signer_note_text: String,
            signer_counter: usize,
            miro_frame_id: String,
            validated_signer: bool,
        ) -> String {
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let fill_color = if validated_signer { "red" } else { "dark_blue" };
            let response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/sticky_notes",
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
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
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
            let RequiredConfig { miro_board_id, .. } = BatConfig::get_init_config().required;
            let AuditorConfig {
                miro_oauth_access_token,
                ..
            } = BatConfig::get_init_config().auditor;
            // let x_position = x + x_move;
            let client = reqwest::Client::new();
            let y_position = 150 + signer_counter * 270;
            let response = client
                .post(format!(
                    "https://api.miro.com/v2/boards/{miro_board_id}/images",
                ))
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
                .header(AUTHORIZATION, format!("Bearer {miro_oauth_access_token}"))
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
    // pub mod token {
    //     use reqwest::header::{ACCEPT, AUTHORIZATION};
    //     use serde_json::Value;

    //     pub async fn get_token_context(moat: &String) -> Value {
    //         let client = reqwest::Client::new();
    //         let token_context_response = client
    //             .get("https://api.miro.com/v1/oauth-token")
    //             .header(ACCEPT, "application/json")
    //             .header(AUTHORIZATION, format!("Bearer {moat}"))
    //             .send()
    //             .await
    //             .unwrap()
    //             .text()
    //             .await
    //             .unwrap();
    //         let token_context = serde_json::from_str(token_context_response.as_str()).unwrap();
    //         token_context
    //     }

    //     // pub async fn validate_token_permissions(moat: &String) {
    //     //     let board = super::board::get_board().await;
    //     //     let board_owner_id = board["owner"]["id"].clone();
    //     //     let token_context = get_token_context(moat).await;
    //     //     let
    //     // }
    // }
}
