use super::*;
pub mod frame {
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
}
