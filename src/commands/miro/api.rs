use super::*;
pub mod frame {
    use super::*;

    #[derive(Debug)]
    pub struct MiroFrameIdAndUrl {
        pub id: String,
        pub url: String,
    }

    // returns the frame url
    pub async fn create_frame_for_entrypoint(
        entrypoint_name: &str,
    ) -> Result<MiroFrameIdAndUrl, String> {
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
                Ok(MiroFrameIdAndUrl {
                    id: frame_id,
                    url: frame_url,
                })
            }
            Err(err_message) => Err(err_message.to_string()),
        }
    }
    // returns the frame url
    pub async fn create_frame(
        frame_title: &str,
        x_position: i32,
        y_position: i32,
        width: i32,
        height: i32,
    ) -> Result<MiroFrameIdAndUrl, String> {
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
                          "title": frame_title,
                          "type": "freeform"
                     },
                     "position": {
                          "origin": "center",
                          "x": x_position,
                          "y": y_position
                     },
                     "geometry": {
                        "width": width,
                        "height": height
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
                Ok(MiroFrameIdAndUrl {
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
pub mod custom_image {
    use super::*;

    // uploads the image in file_path to the board
    pub async fn create_image_from_device_and_update_position(
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
        super::item::update_snapshot_position(entrypoint_name.to_string(), &file_name, id.clone())
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

    pub async fn get_items_on_board(miro_item_type: Option<MiroItemType>) -> Result<Value, String> {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        let client = reqwest::Client::new();
        let url = if let Some(item_type) = miro_item_type {
            let item_type_string = item_type.str_item_type();
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
    pub async fn create_sticky_note(
        sticky_note_content: String,
        sticky_note_color: String,
        frame_id: String,
        x_position: i32,
        y_position: i32,
    ) -> String {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        // let x_position = x + x_move;
        let client = reqwest::Client::new();
        let response = client
            .post(format!(
                "https://api.miro.com/v2/boards/{board_id}/sticky_notes",
            ))
            .body(
                json!({
                    "data": {
                        "content": sticky_note_content,
                        "shape": "rectangle"
                    },
                    "style": {
                        "fillColor": sticky_note_color
                    },
                    "position": {
                        "origin": "center",
                        "x": x_position,
                        "y": y_position
                    },
                    "geometry": {
                        "width": MIRO_WIDTH_ACCOUNTS_STICKY_NOTE
                    },
                    "parent": {
                        "id": frame_id
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
    use crate::utils::path::FilePathType;

    use super::*;
    pub async fn get_id_from_response(response: reqwest::Response) -> String {
        let response_string = response.text().await.unwrap();
        let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        response["id"].to_string().replace("\"", "")
    }
    pub fn get_frame_id_from_co_file(entrypoint_name: &str) -> Result<String, String> {
        // let started_file_path = utils::path::get_auditor_code_overhaul_started_file_path(
        //     Some(entrypoint_name.to_string()),
        // )?;
        let started_file_path = utils::path::get_file_path(
            FilePathType::CodeOverhaulStarted {
                file_name: entrypoint_name.to_string(),
            },
            true,
        );
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
