use crate::batbelt::miro::helpers::get_id_from_response;
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::{MiroConfig, MiroItemType};

use error_stack::Result;
use reqwest;
use reqwest::header::CONTENT_TYPE;
use reqwest::{
    header::AUTHORIZATION,
    multipart::{self, Form},
};
use serde_json::*;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

#[derive(Debug)]
pub enum MiroImageType {
    FromUrl,
    FromPath,
}

#[derive(Debug)]
pub struct MiroImage {
    pub source: String,
    pub image_type: MiroImageType,
    pub item_type: MiroItemType,
    pub item_id: String,
    pub parent_id: String,
    pub x_position: i64,
    pub y_position: i64,
    pub height: u64,
}

impl MiroImage {
    pub fn new_from_file_path(file_path: &str, parent_id: &str) -> Self {
        MiroImage {
            source: file_path.to_string(),
            image_type: MiroImageType::FromPath,
            item_type: MiroItemType::Image,
            item_id: "".to_string(),
            parent_id: parent_id.to_string(),
            x_position: 0,
            y_position: 0,
            height: 0,
        }
    }

    pub async fn new_from_item_id(item_id: &str, image_type: MiroImageType) -> Self {
        let response = MiroItem::get_specific_item_on_board(item_id).await.unwrap();
        let response_string = response.text().await.unwrap();
        let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        let item_id = response["id"].to_string().replace("\"", "");
        let parent_id = response["parent"]["id"].to_string().replace("\"", "");
        let height = response["geometry"]["height"].as_f64().unwrap() as u64;
        let x_position = response["position"]["x"].as_f64().unwrap() as i64;
        let y_position = response["position"]["y"].as_f64().unwrap() as i64;
        MiroImage {
            source: "".to_string(),
            image_type,
            item_type: MiroItemType::Image,
            item_id,
            parent_id,
            x_position,
            y_position,
            height,
        }
    }

    pub fn new_from_url(
        source_url: &str,
        parent_id: &str,
        x_position: i64,
        y_position: i64,
        height: u64,
    ) -> Self {
        MiroImage {
            source: source_url.to_string(),
            image_type: MiroImageType::FromUrl,
            item_type: MiroItemType::Image,
            item_id: "".to_string(),
            parent_id: parent_id.to_string(),
            x_position,
            y_position,
            height,
        }
    }

    pub async fn deploy(&mut self) {
        let id = match self.image_type {
            MiroImageType::FromPath => api::create_image_from_device(&self.source).await.unwrap(),
            MiroImageType::FromUrl => api::create_image_item_using_url(
                &self.source,
                &self.parent_id,
                self.x_position,
                self.y_position,
                self.height,
            )
            .await
            .unwrap(),
        };
        self.item_id = id;
    }

    pub async fn update_from_path(&mut self, new_path: &str) {
        api::update_image_from_device(new_path, &self.item_id).await;
        self.source = new_path.to_string();
    }

    pub async fn update_position(&mut self, x_position: i64, y_position: i64) {
        let image_item = MiroItem::new(
            &self.item_id,
            &self.parent_id,
            x_position,
            y_position,
            self.item_type.clone(),
        );
        image_item.update_item_position().await;
        self.x_position = x_position;
        self.y_position = y_position;
    }
}

mod api {
    use error_stack::IntoReport;

    use crate::batbelt::miro::MiroError;

    use super::*;
    pub async fn create_image_from_device(file_path: &str) -> Result<String, MiroError> {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        let file_name = file_path.split('/').last().unwrap().to_string();
        let file = File::open(file_path).await.unwrap();
        // read file body stream
        let stream = FramedRead::new(file, BytesCodec::new());
        let file_body = reqwest::Body::wrap_stream(stream);

        //make form part of file
        let some_file = multipart::Part::stream(file_body)
            .file_name(file_name.clone())
            .mime_str("text/plain")
            .unwrap();
        //create the multipart form
        let form = Form::new().part("resource", some_file);
        let client = reqwest::Client::new();
        let response = client
            .post(format!("https://api.miro.com/v2/boards/{board_id}/images"))
            .multipart(form)
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await
            .into_report();
        let id = get_id_from_response(response).await?;
        Ok(id)
    }
    pub async fn create_image_item_using_url(
        source_url: &str,
        parent_id: &str,
        x_position: i64,
        y_position: i64,
        height: u64,
    ) -> Result<String, MiroError> {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        let client = reqwest::Client::new();
        let response = client
            .post(format!("https://api.miro.com/v2/boards/{board_id}/images"))
            .body(
                json!({
                    "data": {
                        "url": source_url
                   },
                   "position": {
                        "origin": "center",
                        "x": x_position,
                        "y": y_position
                   },
                   "geometry": {
                        "height": height
                   },
                   "parent": {
                        "id": parent_id
                   }
                })
                .to_string(),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await
            .into_report();
        let id = get_id_from_response(response).await?;
        Ok(id)
    }

    pub async fn update_image_from_device(file_path: &str, item_id: &str) {
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
        let form = Form::new().part("resource", some_file);
        let client = reqwest::Client::new();
        let _response = client
            .patch(format!(
                "https://api.miro.com/v2/boards/{board_id}/images/{item_id}"
            ))
            .multipart(form)
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await
            .unwrap();
    }

    // // uploads the image in file_path to the board
    // pub async fn create_image_from_device_and_update_position(
    //     file_path: String,
    //     entrypoint_name: &str,
    // ) -> Result<String, MiroError> {
    //     let MiroConfig {
    //         access_token,
    //         board_id,
    //         ..
    //     } = MiroConfig::new();
    //     let file_name = file_path.clone().split('/').last().unwrap().to_string();
    //     let file = File::open(file_path.clone()).await.unwrap();
    //     // read file body stream
    //     let stream = FramedRead::new(file, BytesCodec::new());
    //     let file_body = reqwest::Body::wrap_stream(stream);
    //
    //     //make form part of file
    //     let some_file = multipart::Part::stream(file_body)
    //         .file_name(file_name.clone())
    //         .mime_str("text/plain")
    //         .unwrap();
    //     //create the multipart form
    //     let form = multipart::Form::new().part("resource", some_file);
    //     let client = reqwest::Client::new();
    //     let response = client
    //         .post(format!("https://api.miro.com/v2/boards/{board_id}/images"))
    //         .multipart(form)
    //         .header(AUTHORIZATION, format!("Bearer {access_token}"))
    //         .send()
    //         .await
    //         .unwrap()
    //         .text()
    //         .await
    //         .unwrap();
    //     let response: Value = serde_json::from_str(&response.as_str()).unwrap();
    //     let id = response["id"].to_string().replace("\"", "");
    //     super::item::update_snapshot_position(entrypoint_name.to_string(), &file_name, id.clone())
    //         .await?;
    //     Ok(id)
    // }

    pub async fn update_image_position(
        item_id: &str,
        parent_id: &str,
        x_position: i64,
        y_position: i64,
    ) {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        let client = reqwest::Client::new();
        let _response = client
            .patch(format!(
                "https://api.miro.com/v2/boards/{board_id}/images/{item_id}",
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
            .await
            .unwrap();
    }
}
