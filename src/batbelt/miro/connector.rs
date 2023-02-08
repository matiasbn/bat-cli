use crate::batbelt::miro::MiroConfig;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
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
