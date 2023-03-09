use super::*;
use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::miro::MiroItemType;
use colored::Colorize;
use error_stack::{IntoReport, Result};
use regex::Regex;
use serde_json::json;

pub const MIRO_FRAME_WIDTH: u64 = 5600;
pub const MIRO_FRAME_HEIGHT: u64 = 2600;
pub const MIRO_BOARD_COLUMNS: i64 = 5;
pub const MIRO_INITIAL_X: i64 = 4800;
pub const MIRO_INITIAL_Y: i64 = 0;

#[derive(Debug, Clone)]
pub enum MiroCodeOverhaulConfig {
    EntryPoint,
    ContextAccount,
    Validations,
    Handler,
}

impl MiroCodeOverhaulConfig {
    pub fn get_positions(&self) -> (i64, i64) {
        match self {
            MiroCodeOverhaulConfig::EntryPoint => (
                MIRO_FRAME_WIDTH as i64 * 3 / 10,
                (MIRO_FRAME_HEIGHT as i64) / 10,
            ),
            MiroCodeOverhaulConfig::ContextAccount => (
                MIRO_FRAME_WIDTH as i64 * 6 / 10,
                (MIRO_FRAME_HEIGHT as i64) / 4,
            ),
            MiroCodeOverhaulConfig::Validations => (
                MIRO_FRAME_WIDTH as i64 * 10 / 12,
                (MIRO_FRAME_HEIGHT as i64) / 4,
            ),
            MiroCodeOverhaulConfig::Handler => (
                MIRO_FRAME_WIDTH as i64 * 10 / 12,
                (MIRO_FRAME_HEIGHT as i64) * 3 / 4,
            ),
        }
    }
}

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
        let response_string = api_response
            .text()
            .await
            .into_report()
            .change_context(MiroError)?;
        let value: Value = serde_json::from_str(response_string.as_str())
            .into_report()
            .change_context(MiroError)?;
        self.parse_value(value)?;
        Ok(())
    }

    fn parse_value(&mut self, value: Value) -> Result<(), MiroError> {
        let miro_config = MiroConfig::new()?;
        self.item_id = value["id"].to_string().replace('\"', "");
        self.frame_url = Some(miro_config.get_frame_url(&self.item_id));
        self.title = value["data"]["title"].to_string().replace('\"', "");
        self.height = value["geometry"]["height"].as_f64().ok_or(MiroError)? as u64;
        self.width = value["geometry"]["width"].as_f64().ok_or(MiroError)? as u64;
        self.x_position = value["position"]["x"].as_f64().ok_or(MiroError)? as i64;
        self.y_position = value["position"]["y"].as_f64().ok_or(MiroError)? as i64;
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
        let response = MiroItem::get_items_on_board(Some(MiroItemType::Frame), None)
            .await
            .ok()
            .ok_or(MiroError)
            .into_report()?;
        let response_string = response.text().await.ok().ok_or(MiroError).into_report()?;
        let response: Value = serde_json::from_str(response_string.as_str())
            .ok()
            .ok_or(MiroError)
            .into_report()?;
        debug!("MiroItem::get_items_on_board:\n {}", response);
        let mut data: Vec<Value> = response["data"]
            .as_array()
            .ok_or(MiroError)
            .into_report()?
            .clone();
        let mut size = response["size"].as_i64().ok_or(MiroError).into_report()?;
        let mut limit = response["limit"].as_i64().ok_or(MiroError).into_report()?;
        while limit == size {
            let cursor = response["cursor"].as_str().ok_or(MiroError).into_report()?;
            let response =
                MiroItem::get_items_on_board(Some(MiroItemType::Frame), Some(cursor.to_string()))
                    .await
                    .ok()
                    .ok_or(MiroError)
                    .into_report()?;
            let response_string = response.text().await.ok().ok_or(MiroError).into_report()?;
            let response: Value = serde_json::from_str(response_string.as_str())
                .ok()
                .ok_or(MiroError)
                .into_report()?;
            let mut cursor_data: Vec<Value> = response["data"]
                .as_array()
                .ok_or(MiroError)
                .into_report()?
                .clone();
            data.append(&mut cursor_data);
            size = response["size"].as_i64().ok_or(MiroError).into_report()?;
            limit = response["limit"].as_i64().ok_or(MiroError).into_report()?;
        }
        let mut frames = vec![];
        for frame_data in data {
            let mut new_frame = Self::new_empty();
            new_frame
                .parse_value(frame_data)
                .ok()
                .ok_or(MiroError)
                .into_report()?;
            frames.push(new_frame);
        }
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

    pub async fn prompt_select_frame(
        title_regex_filter_vec: Option<Vec<Regex>>,
    ) -> MiroResult<Self> {
        MiroConfig::check_miro_enabled()?;

        println!(
            "\nGetting the {} from the {} ...\n",
            "frames".yellow(),
            "Miro board".yellow()
        );
        let mut miro_frames: Vec<MiroFrame> = MiroFrame::get_frames_from_miro().await?;

        if let Some(regex_filter_vec) = title_regex_filter_vec {
            miro_frames = miro_frames
                .into_iter()
                .filter(|frame| {
                    for filter in regex_filter_vec.clone() {
                        if filter.is_match(&frame.title) {
                            return false;
                        }
                    }
                    true
                })
                .collect::<Vec<_>>();
        }

        log::info!("miro_frames:\n{:#?}", miro_frames);

        miro_frames.sort_by(|a, b| a.title.cmp(&b.title));
        let miro_frame_titles: Vec<String> = miro_frames
            .iter()
            .map(|frame| frame.title.clone())
            .collect();

        let prompt_text = format!("Please select the destination {}", "Miro Frame".green());
        let selection =
            BatDialoguer::select(prompt_text, miro_frame_titles, None).change_context(MiroError)?;
        let selected_miro_frame: MiroFrame = miro_frames[selection].clone();
        Ok(selected_miro_frame)
    }

    pub fn get_frame_url_by_frame_id(frame_id: &str) -> MiroResult<String> {
        let bat_config = BatConfig::get_config().change_context(MiroError)?;
        let url = normalizer::UrlNormalizer::new(
            format!("{}/?moveToWidget={frame_id}", bat_config.miro_board_url).as_str(),
        )
        .into_report()
        .change_context(MiroError)?
        .normalize(None)
        .into_report()
        .change_context(MiroError)?;
        Ok(url)
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

// #[cfg(debug_assertions)]
// mod miro_frame_test {
//
//     #[test]
//     fn test_get_miro_frame_url() {
//         let bat_config = BatConfig {
//             initialized: false,
//             project_name: "".to_string(),
//             client_name: "".to_string(),
//             commit_hash_url: "".to_string(),
//             starting_date: "".to_string(),
//             miro_board_url: "https://miro.com/app/board/uXjVPzsgmiY=/".to_string(),
//             auditor_names: vec![],
//             program_lib_path: "".to_string(),
//             program_name: "".to_string(),
//             project_repository_url: "".to_string(),
//         };
//
//         assert_fs::NamedTempFile::new("Bat.toml").unwrap();
//
//         bat_config.save().unwrap();
//
//         let expected_miro_url =
//             "https://miro.com/app/board/uXjVPzsgmiY=/?moveToWidget=3458764548095165114";
//         let miro_frame_id = "3458764548095165114";
//         let frame_parsed = MiroFrame::get_frame_url_by_frame_id(miro_frame_id).unwrap();
//         assert_eq!(expected_miro_url, frame_parsed);
//     }
// }
