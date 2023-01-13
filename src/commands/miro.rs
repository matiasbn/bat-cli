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
        use normalize_url::normalizer;
        use reqwest::Body;
        use serde_json::{json, Value};

        use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
        use reqwest::multipart::{self};
        use tokio::fs::File;
        use tokio_util::codec::{BytesCodec, FramedRead};

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
                            "width": 1878.94949246026,
                            "height": 1056.9090895089
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
        // uploads the image in file_path to the board
        pub async fn create_image_from_device(file_path: String) {
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
                .file_name(file_name)
                .mime_str("text/plain")
                .unwrap();

            //create the multipart form
            let form = multipart::Form::new().part("resource", some_file);

            let client = reqwest::Client::new();
            let _board_response = client
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
