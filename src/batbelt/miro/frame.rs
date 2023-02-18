use super::*;
use crate::batbelt::miro::MiroItemType;
use error_stack::{IntoReport, Result, ResultExt};
use serde_json::json;

#[derive(Debug, Clone)]
pub struct MiroFrame {
    pub title: String,
    pub item_id: String,
    pub frame_url: Option<String>,
    pub height: u64,
    pub width: u64,
    pub x_position: i64,
    pub y_position: i64,
}

impl MiroFrame {
    pub fn new(title: &str, height: u64, width: u64, x_position: i64, y_position: i64) -> Self {
        MiroFrame {
            title: title.to_string(),
            item_id: "".to_string(),
            frame_url: None,
            height,
            width,
            x_position,
            y_position,
        }
    }

    pub fn new_empty() -> Self {
        MiroFrame {
            title: "".to_string(),
            item_id: "".to_string(),
            frame_url: None,
            height: 0,
            width: 0,
            x_position: 0,
            y_position: 0,
        }
    }

    async fn parse_api_response(
        &mut self,
        api_response: reqwest::Response,
    ) -> Result<(), MiroError> {
        let response_string = api_response.text().await.unwrap();
        let value: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        self.parse_value(value)?;
        Ok(())
    }

    fn parse_value(&mut self, value: Value) -> Result<(), MiroError> {
        let miro_config = MiroConfig::new()?;
        self.item_id = value["id"].to_string().replace("\"", "");
        self.frame_url = Some(miro_config.get_frame_url(&self.item_id));
        self.title = value["data"]["title"].to_string().replace("\"", "");
        self.height = value["geometry"]["height"]
            .as_f64()
            .ok_or(MiroError)
            .into_report()? as u64;
        self.width = value["geometry"]["width"]
            .as_f64()
            .ok_or(MiroError)
            .into_report()? as u64;
        self.x_position = value["position"]["x"]
            .as_f64()
            .ok_or(MiroError)
            .into_report()? as i64;
        self.y_position = value["position"]["y"]
            .as_f64()
            .ok_or(MiroError)
            .into_report()? as i64;
        Ok(())
    }

    // let message = format!(
    //     "Error getting geometry.height for frame: \n title:{} \n item_id:{}\n frame_url:{}",
    //     self.title, self.item_id, self.frame_url.unwrap()
    //     );
    // return Err(Report::new(MiroError).attach_printable(message));

    pub async fn new_from_item_id(item_id: &str) -> Result<Self, MiroError> {
        let api_response = MiroItem::get_specific_item_on_board(item_id).await.unwrap();
        let mut new_frame = Self::new_empty();
        new_frame.parse_api_response(api_response).await?;
        Ok(new_frame)
    }

    pub async fn deploy(&mut self) -> Result<(), MiroError> {
        let api_response = api::create_frame(
            &self.title,
            self.x_position,
            self.y_position,
            self.width,
            self.height,
        )
        .await?;
        self.parse_api_response(api_response).await?;
        Ok(())
    }

    pub async fn get_frames_from_miro() -> Result<Vec<MiroFrame>, MiroError> {
        let response = MiroItem::get_items_on_board(Some(MiroItemType::Frame))
            .await
            .unwrap();
        let response_string = response.text().await.unwrap();
        let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        debug!("MiroItem::get_items_on_board:\n {}", response);
        let data = response["data"].as_array().unwrap();
        let frames = data
            .clone()
            .into_iter()
            .map(|data_response| {
                let mut new_frame = Self::new_empty();
                new_frame.parse_value(data_response);
                new_frame
            })
            .collect();
        Ok(frames)
    }

    pub async fn get_items_within_frame(&self) -> Result<Vec<MiroObject>, MiroError> {
        let response = api::get_items_within_frame(&self.item_id).await.unwrap();
        MiroObject::multiple_from_response(response).await
    }

    pub async fn update_position(
        &mut self,
        x_position: i64,
        y_position: i64,
    ) -> Result<(), MiroError> {
        api::update_frame_position(&self.item_id, x_position, y_position).await?;
        self.x_position = x_position;
        self.y_position = y_position;
        Ok(())
    }
}

mod api {
    use super::*;

    // returns the frame url
    pub async fn create_frame(
        frame_title: &str,
        x_position: i64,
        y_position: i64,
        width: u64,
        height: u64,
    ) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        let client = reqwest::Client::new();

        let response = client
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
        MiroConfig::parse_response_from_miro(response)
    }

    // returns the frame url
    // pub async fn create_frame_for_entrypoint(entrypoint_name: &str) -> MiroApiResult {
    //     let MiroConfig {
    //         access_token,
    //         board_id,
    //         ..
    //     } = MiroConfig::new();
    //     let client = reqwest::Client::new();

    //     let response = client
    //         .post(format!("https://api.miro.com/v2/boards/{board_id}/frames"))
    //         .body(
    //             json!({
    //                  "data": {
    //                       "format": "custom",
    //                       "title": entrypoint_name,
    //                       "type": "freeform"
    //                  },
    //                  "position": {
    //                       "origin": "center",
    //                       "x": 0,
    //                       "y": 0
    //                  },
    //                  "geometry": {
    //                     "width": MIRO_FRAME_WIDTH,
    //                     "height": MIRO_FRAME_HEIGHT
    //                }
    //             })
    //             .to_string(),
    //         )
    //         .header(CONTENT_TYPE, "application/json")
    //         .header(AUTHORIZATION, format!("Bearer {access_token}"))
    //         .send()
    //         .await;
    //     MiroConfig::parse_response_from_miro(response)
    // }

    // pub async fn get_frame_position(frame_id: &str) -> (u64, u64) {
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
        frame_id: &str,
        x_position: i64,
        y_position: i64,
    ) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        let client = reqwest::Client::new();
        let response = client
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
            .await;
        MiroConfig::parse_response_from_miro(response)
    }

    pub async fn get_items_within_frame(frame_id: &str) -> MiroApiResult {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new()?;
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "https://api.miro.com/v2/boards/{board_id}/items?parent_item_id={frame_id}"
            ))
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await;
        MiroConfig::parse_response_from_miro(response)
    }

    // pub async fn update_frame_position(
    //     entrypoint_name: String,
    //     co_finished_files: i32,
    // ) -> Result<(), String> {
    //     let MiroConfig {
    //         access_token,
    //         board_id,
    //         ..
    //     } = MiroConfig::new();
    //     let frame_id = super::helpers::get_frame_id_from_co_file(entrypoint_name.as_str())?;
    //     let x_modifier = co_finished_files % MIRO_BOARD_COLUMNS;
    //     let y_modifier = co_finished_files / MIRO_BOARD_COLUMNS;
    //     let x_position = MIRO_INITIAL_X + (MIRO_FRAME_WIDTH + 100) * x_modifier;
    //     let y_position = MIRO_INITIAL_Y + (MIRO_FRAME_HEIGHT + 100) * y_modifier;
    //     let client = reqwest::Client::new();
    //     let _response = client
    //         .patch(format!(
    //             "https://api.miro.com/v2/boards/{board_id}/frames/{frame_id}",
    //         ))
    //         .body(
    //             json!({
    //                 "position": {
    //                     "x": x_position,
    //                     "y": y_position,
    //                     "origin": "center",
    //                 },
    //             })
    //                 .to_string(),
    //         )
    //         .header(CONTENT_TYPE, "application/json")
    //         .header(AUTHORIZATION, format!("Bearer {access_token}"))
    //         .send()
    //         .await
    //         .unwrap()
    //         .text()
    //         .await
    //         .unwrap();
    //     Ok(())
    //     // println!("update frame position response: {response}")
    // }
}
