use std::error::Error;
use std::fmt;

use crate::config::*;
use normalize_url::normalizer;
use reqwest;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

use serde_json::{self, Value};

pub mod connector;
pub mod frame;
pub mod image;
pub mod item;
pub mod shape;
pub mod sticky_note;

use error_stack::{Report, Result, ResultExt};

#[derive(Debug)]
pub struct MiroError;

impl fmt::Display for MiroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Miro error")
    }
}

impl Error for MiroError {}

pub struct MiroConfig {
    access_token: String,
    board_id: String,
    board_url: String,
}

pub type MiroApiResult = Result<reqwest::Response, MiroError>;

pub type MiroResult<T> = Result<T, MiroError>;

impl MiroConfig {
    pub fn new() -> Result<Self, MiroError> {
        Self::check_miro_enabled()?;
        let bat_config = BatConfig::get_config().change_context(MiroError)?;
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(MiroError)?;
        let access_token = bat_auditor_config.miro_oauth_access_token;
        let board_url = bat_config.miro_board_url;
        let board_id = Self::get_miro_board_id(board_url.clone())?;
        Ok(MiroConfig {
            access_token,
            board_id,
            board_url,
        })
    }

    pub fn parse_response_from_miro(
        response: std::result::Result<reqwest::Response, reqwest::Error>,
    ) -> Result<reqwest::Response, MiroError> {
        match response {
            Ok(resp) => Ok(resp),
            Err(error) => {
                let message = "Bad response from Miro";
                log::error!("Miro response: \n {:#?}", error);
                Err(Report::new(MiroError).attach_printable(message))
            }
        }
    }

    pub fn miro_enabled(&self) -> bool {
        !self.access_token.is_empty()
    }

    pub fn check_miro_enabled() -> Result<(), MiroError> {
        let bat_auditor_config = BatAuditorConfig::get_config().unwrap();
        if bat_auditor_config.miro_oauth_access_token.is_empty() {
            return Err(Report::new(MiroError)
                .attach_printable("miro_oauth_access_token is empty in BatAuditor.toml"));
        };
        Ok(())
    }

    pub fn get_frame_url(&self, frame_id: &str) -> String {
        let url = normalizer::UrlNormalizer::new(
            format!("{}/?moveToWidget={frame_id}", self.board_url).as_str(),
        )
        .unwrap()
        .normalize(None)
        .unwrap();
        url
    }

    pub fn get_miro_board_id(miro_board_url: String) -> Result<String, MiroError> {
        let _error_msg = format!(
            "Error obtaining the miro board id for the url: {}",
            miro_board_url
        );
        let miro_board_id = miro_board_url
            .split("board/")
            .last()
            .ok_or(MiroError)?
            .split('/')
            .next()
            .ok_or(MiroError)?
            .to_string();
        Ok(miro_board_id)
    }

    /// Fetches the user's boards from the Miro API using the given OAuth token.
    /// Returns a list of (board_name, board_url) tuples.
    pub async fn list_boards(access_token: &str) -> Result<Vec<(String, String)>, MiroError> {
        let client = reqwest::Client::new();
        let mut all_boards: Vec<(String, String)> = vec![];
        let mut offset: usize = 0;
        let limit = 50;

        loop {
            let response = client
                .get(format!(
                    "https://api.miro.com/v2/boards?limit={}&offset={}&sort=last_modified",
                    limit, offset
                ))
                .header(AUTHORIZATION, format!("Bearer {}", access_token))
                .header(CONTENT_TYPE, "application/json")
                .send()
                .await
                .map_err(|e| {
                    log::error!("Miro list boards error: {:#?}", e);
                    Report::new(MiroError).attach_printable("Failed to fetch boards from Miro")
                })?;

            let body = response.text().await.map_err(|_| {
                Report::new(MiroError).attach_printable("Failed to read Miro response body")
            })?;

            let json: Value = serde_json::from_str(&body).map_err(|_| {
                Report::new(MiroError).attach_printable("Failed to parse Miro response JSON")
            })?;

            let data = json["data"].as_array().ok_or_else(|| {
                Report::new(MiroError).attach_printable("No 'data' array in Miro response")
            })?;

            if data.is_empty() {
                break;
            }

            for board in data {
                let name = board["name"].as_str().unwrap_or("(unnamed)").to_string();
                let id = board["id"].as_str().unwrap_or("").to_string();
                if !id.is_empty() {
                    let url = format!("https://miro.com/app/board/{}/", id);
                    all_boards.push((name, url));
                }
            }

            let total = json["total"].as_u64().unwrap_or(0) as usize;
            offset += limit;
            if offset >= total {
                break;
            }
        }

        Ok(all_boards)
    }
}

#[derive(Debug, Clone)]
pub struct MiroObject {
    pub item_id: String,
    pub title: String,
    pub height: u64,
    pub width: u64,
    pub x_position: i64,
    pub y_position: i64,
    pub item_type: MiroItemType,
}

impl MiroObject {
    pub fn new(
        item_id: String,
        title: String,
        height: u64,
        width: u64,
        x_position: i64,
        y_position: i64,
        item_type: MiroItemType,
    ) -> Self {
        Self {
            item_id,
            title,
            height,
            width,
            x_position,
            y_position,
            item_type,
        }
    }

    pub async fn multiple_from_response(
        response: reqwest::Response,
    ) -> Result<Vec<Self>, MiroError> {
        let response_string = response.text().await.unwrap();
        let response: Value = serde_json::from_str(response_string.as_str()).unwrap();
        let data = response["data"].as_array().unwrap();
        let objects = data
            .clone()
            .into_iter()
            .map(|data_response| {
                let item_id = data_response["id"].to_string().replace('\"', "");
                let item_type = data_response["type"].to_string().replace('\"', "");
                let title = data_response["data"]["title"].to_string().replace('\"', "");
                let height = data_response["geometry"]["height"].as_f64().unwrap() as u64;
                let width = data_response["geometry"]["width"].as_f64().unwrap() as u64;
                let x_position = data_response["position"]["x"].as_f64().unwrap() as i64;
                let y_position = data_response["position"]["y"].as_f64().unwrap() as i64;
                MiroObject::new(
                    item_id,
                    title,
                    height,
                    width,
                    x_position,
                    y_position,
                    MiroItemType::from_str(&item_type),
                )
            })
            .collect();
        Ok(objects)
    }
}

#[derive(Debug, Clone)]
pub enum MiroItemType {
    AppCard,
    Card,
    Document,
    Embed,
    Frame,
    Image,
    Shape,
    StickyNote,
    Text,
}

impl MiroItemType {
    pub fn to_string(&self) -> String {
        match self {
            MiroItemType::AppCard => "app_card".to_string(),
            MiroItemType::Card => "card".to_string(),
            MiroItemType::Document => "document".to_string(),
            MiroItemType::Embed => "embed".to_string(),
            MiroItemType::Frame => "frame".to_string(),
            MiroItemType::Image => "image".to_string(),
            MiroItemType::Shape => "shape".to_string(),
            MiroItemType::StickyNote => "sticky_note".to_string(),
            MiroItemType::Text => "text".to_string(),
        }
    }

    pub fn from_str(type_str: &str) -> MiroItemType {
        match type_str {
            "app_card" => MiroItemType::AppCard,
            "card" => MiroItemType::Card,
            "document" => MiroItemType::Document,
            "embed" => MiroItemType::Embed,
            "frame" => MiroItemType::Frame,
            "image" => MiroItemType::Image,
            "shape" => MiroItemType::Shape,
            "sticky_note" => MiroItemType::StickyNote,
            "text" => MiroItemType::Text,
            _ => unimplemented!(),
        }
    }
}
#[derive(Clone)]
pub enum MiroColor {
    Gray,
    LightYellow,
    Yellow,
    Orange,
    LightGreen,
    Green,
    DarkGreen,
    Cyan,
    LightPink,
    Pink,
    Violet,
    Red,
    LightBlue,
    Blue,
    DarkBlue,
    Black,
}

impl MiroColor {
    pub fn to_str(&self) -> &str {
        match self {
            MiroColor::Gray => "gray",
            MiroColor::LightYellow => "light_yellow",
            MiroColor::Yellow => "yellow",
            MiroColor::Orange => "orange",
            MiroColor::LightGreen => "light_green",
            MiroColor::Green => "green",
            MiroColor::DarkGreen => "dark_green",
            MiroColor::Cyan => "cyan",
            MiroColor::LightPink => "light_pink",
            MiroColor::Pink => "pink",
            MiroColor::Violet => "violet",
            MiroColor::Red => "red",
            MiroColor::LightBlue => "light_blue",
            MiroColor::Blue => "blue",
            MiroColor::DarkBlue => "dark_blue",
            MiroColor::Black => "black",
        }
    }
    pub fn from_str(color_str: &str) -> MiroColor {
        match color_str {
            "gray" => MiroColor::Gray,
            "light_yellow" => MiroColor::LightYellow,
            "yellow" => MiroColor::Yellow,
            "orange" => MiroColor::Orange,
            "light_green" => MiroColor::LightGreen,
            "green" => MiroColor::Green,
            "dark_green" => MiroColor::DarkGreen,
            "cyan" => MiroColor::Cyan,
            "light_pink" => MiroColor::LightPink,
            "pink" => MiroColor::Pink,
            "violet" => MiroColor::Violet,
            "red" => MiroColor::Red,
            "light_blue" => MiroColor::LightBlue,
            "blue" => MiroColor::Blue,
            "dark_blue" => MiroColor::DarkBlue,
            "black" => MiroColor::Black,
            _ => unimplemented!(),
        }
    }

    pub fn get_colors_vec() -> Vec<String> {
        vec![
            "gray".to_string(),
            "light_yellow".to_string(),
            "yellow".to_string(),
            "orange".to_string(),
            "light_green".to_string(),
            "green".to_string(),
            "dark_green".to_string(),
            "cyan".to_string(),
            "light_pink".to_string(),
            "pink".to_string(),
            "violet".to_string(),
            "red".to_string(),
            "light_blue".to_string(),
            "blue".to_string(),
            "dark_blue".to_string(),
            "black".to_string(),
        ]
    }
}

use self::item::MiroItem;

pub mod helpers {

    use error_stack::Report;

    use super::*;

    pub async fn get_id_from_response(
        response: Result<reqwest::Response, reqwest::Error>,
    ) -> Result<String, MiroError> {
        match response {
            Ok(response) => {
                let response_string = response.text().await.unwrap();
                let response: Value = serde_json::from_str(response_string.as_str()).unwrap();
                Ok(response["id"].to_string().replace('\"', ""))
            }
            Err(err_message) => {
                Err(Report::new(MiroError).attach_printable(err_message.to_string()))
            }
        }
    }
}
