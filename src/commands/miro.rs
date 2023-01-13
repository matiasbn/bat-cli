pub mod miro_api {

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
        use serde_json::{json, Value};

        use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

        use crate::config::{AuditorConfig, BatConfig, RequiredConfig};

        // returns the frame url
        pub async fn create_frame(entrypoint_name: &str) -> Value {
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
            response
        }
    }
    pub mod token {
        use reqwest::header::{ACCEPT, AUTHORIZATION};
        use serde_json::Value;

        pub async fn get_token_context(moat: &String) -> Value {
            let client = reqwest::Client::new();
            let token_context_response = client
                .get("https://api.miro.com/v1/oauth-token")
                .header(ACCEPT, "application/json")
                .header(AUTHORIZATION, format!("Bearer {moat}"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let token_context = serde_json::from_str(token_context_response.as_str()).unwrap();
            token_context
        }

        // pub async fn validate_token_permissions(moat: &String) {
        //     let board = super::board::get_board().await;
        //     let board_owner_id = board["owner"]["id"].clone();
        //     let token_context = get_token_context(moat).await;
        //     let
        // }
    }
}
