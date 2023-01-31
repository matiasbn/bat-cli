use reqwest;
use reqwest::{
    header::AUTHORIZATION,
    multipart::{self, Form},
};
use serde_json::*;
use std::result::Result;
use tokio::fs::File;
use tokio_util::codec::{BytesCodec, FramedRead};

use crate::commands::miro::MiroConfig;

pub mod api {
    use super::*;
    pub async fn create_image_from_device(file_path: &str) -> Result<String, String> {
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
            .unwrap()
            .text()
            .await
            .unwrap();
        let response: Value = serde_json::from_str(&response.as_str()).unwrap();
        let id = response["id"].to_string().replace("\"", "");
        // update_snapshot_position(entrypoint_name.to_string(), &file_name, id.clone()).await?;
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
            .unwrap()
            .text()
            .await
            .unwrap();
    }
}
