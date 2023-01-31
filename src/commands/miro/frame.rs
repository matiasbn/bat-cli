use super::*;

pub struct MiroFrame {
    pub frame_title: String,
    pub frame_id: String,
    pub frame_url: String,
    pub height: u64,
    pub width: u64,
}

impl MiroFrame {
    pub fn new(
        frame_title: String,
        frame_id: String,
        frame_url: String,
        height: u64,
        width: u64,
    ) -> Self {
        MiroFrame {
            frame_title,
            frame_id,
            frame_url,
            height,
            width,
        }
    }

    pub async fn get_frames_from_miro() -> Vec<MiroFrame> {
        let response = item::api::get_items_on_board(Some(MiroItemType::Frame))
            .await
            .unwrap();
        let response_string = response.text().await.unwrap();
        let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        let data = response["data"].as_array().unwrap();
        // println!("data {:#?}", data);
        let miro_config = MiroConfig::new();
        let frames = data
            .clone()
            .into_iter()
            .map(|data_reponse| {
                let frame_id = data_reponse["id"].to_string().replace("\"", "");
                let frame_url = miro_config.get_frame_url(&frame_id);
                let frame_title = data_reponse["data"]["title"].to_string().replace("\"", "");
                let height = data_reponse["geometry"]["height"].as_f64().unwrap() as u64;
                let width = data_reponse["geometry"]["width"].as_f64().unwrap() as u64;
                MiroFrame::new(frame_title, frame_id, frame_url, height, width)
            })
            .collect();
        frames
    }
}

pub mod api {
    use crate::commands::miro::api::helpers::get_id_from_response;

    use super::*;

    #[derive(Debug)]
    pub struct MiroFrame {
        pub id: String,
        pub url: String,
    }

    // returns the frame url
    pub async fn create_frame(
        frame_title: &str,
        x_position: i32,
        y_position: i32,
        width: i32,
        height: i32,
    ) -> Result<MiroFrame, String> {
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
                let frame_id = get_id_from_response(response).await;
                let frame_url = MiroConfig::new().get_frame_url(&frame_id);
                Ok(MiroFrame {
                    id: frame_id.to_string(),
                    url: frame_url,
                })
            }
            Err(err_message) => Err(err_message.to_string()),
        }
    }
    pub async fn get_frame_positon(frame_id: &str) -> (u64, u64) {
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

    pub async fn get_items_within_frame(frame_id: &str) -> (u64, u64) {
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
}
