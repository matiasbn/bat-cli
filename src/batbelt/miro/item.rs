use crate::batbelt::miro::{MiroConfig, MiroItemType};
use error_stack::Result;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;

use super::MiroError;
use crate::batbelt::miro::MiroApiResult;

pub struct MiroItem {
    pub item_id: String,
    pub item_type: MiroItemType,
    pub parent_id: String,
    pub x_position: i64,
    pub y_position: i64,
}

impl MiroItem {
    pub fn new(
        item_id: &str,
        parent_id: &str,
        x_position: i64,
        y_position: i64,
        item_type: MiroItemType,
    ) -> Self {
        MiroItem {
            item_id: item_id.to_string(),
            parent_id: parent_id.to_string(),
            x_position,
            y_position,
            item_type,
        }
    }

    pub async fn update_item_parent(&self) {
        api::update_item_position(&self.item_id, &self.parent_id, 0, 0)
            .await
            .unwrap();
    }

    pub async fn update_item_position(&self) {
        api::update_item_position(
            &self.item_id,
            &self.parent_id,
            self.x_position,
            self.y_position,
        )
        .await
        .unwrap();
    }

    pub async fn update_item_parent_and_position(&self) {
        Self::update_item_parent(self).await;
        Self::update_item_position(self).await;
    }

    pub async fn get_items_on_board(
        miro_item_type: Option<MiroItemType>,
    ) -> Result<reqwest::Response, MiroError> {
        api::get_items_on_board(miro_item_type).await
    }

    pub async fn get_specific_item_on_board(item_id: &str) -> Result<reqwest::Response, MiroError> {
        api::get_specific_item_on_board(item_id).await
    }
}

mod api {

    use super::*;
    pub async fn update_item_position(
        item_id: &str,
        parent_id: &str,
        x_position: i64,
        y_position: i64,
    ) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        // let started_file_path
        // let x_position = x + x_move;
        let client = reqwest::Client::new();
        let response = client
            .patch(format!(
                "https://api.miro.com/v2/boards/{board_id}/items/{item_id}",
            ))
            .body(
                json!({
                    "parent": {
                        "id": parent_id
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
            .await;
        MiroConfig::parse_response_from_miro(response)
    }

    pub async fn get_items_on_board(miro_item_type: Option<MiroItemType>) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        let client = reqwest::Client::new();
        let url = if let Some(item_type) = miro_item_type {
            let item_type_string = item_type.to_string();
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
            .await;
        MiroConfig::parse_response_from_miro(response)
    }

    pub async fn get_specific_item_on_board(item_id: &str) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "https://api.miro.com/v2/boards/{board_id}/items/{item_id}"
            ))
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await;
        MiroConfig::parse_response_from_miro(response)
    }
}
