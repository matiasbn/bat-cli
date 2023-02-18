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

use crate::batbelt::constants::*;
use error_stack::{Report, Result, ResultExt};

#[derive(Debug)]
pub struct MiroError;

impl fmt::Display for MiroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("General Bat error")
    }
}

impl Error for MiroError {}

pub struct MiroConfig {
    access_token: String,
    board_id: String,
    board_url: String,
}

pub type MiroApiResult = Result<reqwest::Response, MiroError>;

impl MiroConfig {
    pub fn new() -> Self {
        let BatConfig {
            required, auditor, ..
        } = BatConfig::get_validated_config().unwrap();
        let access_token = auditor.miro_oauth_access_token;
        let board_url = required.miro_board_url;
        let board_id = Self::get_miro_board_id(board_url.clone());
        MiroConfig {
            access_token,
            board_id,
            board_url,
        }
    }

    pub fn parse_response_from_miro(
        response: std::result::Result<reqwest::Response, reqwest::Error>,
    ) -> Result<reqwest::Response, MiroError> {
        match response {
            Ok(resp) => Ok(resp),
            Err(error) => {
                let message = "Bad response from Miro";
                log::error!("Miro response: \n {:#?}", error);
                return Err(Report::new(MiroError).attach_printable(message));
            }
        }
    }

    pub fn miro_enabled(&self) -> bool {
        !self.access_token.is_empty()
    }

    pub fn check_miro_enabled() {
        let BatConfig { auditor, .. } = BatConfig::get_validated_config().unwrap();
        let access_token = auditor.miro_oauth_access_token;
        assert!(
            !access_token.is_empty(),
            "miro_oauth_access_token is empty in BatAuditor.toml"
        );
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

    pub fn get_miro_board_id(miro_board_url: String) -> String {
        let error_msg = format!(
            "Error obtaining the miro board id for the url: {}",
            miro_board_url
        );
        let miro_board_id = miro_board_url
            .split("board/")
            .last()
            .expect(&error_msg)
            .split("/")
            .next()
            .expect(&error_msg)
            .to_string();
        miro_board_id
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
        let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
        let data = response["data"].as_array().unwrap();
        let objects = data
            .clone()
            .into_iter()
            .map(|data_response| {
                let item_id = data_response["id"].to_string().replace("\"", "");
                let item_type = data_response["type"].to_string().replace("\"", "");
                let title = data_response["data"]["title"].to_string().replace("\"", "");
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

    use crate::batbelt::miro::MiroItemType;

    use super::*;
    pub async fn get_accounts_frame_id() -> Result<String, MiroError> {
        let response = MiroItem::get_items_on_board(Some(MiroItemType::Frame)).await?;
        let response = response.text().await.unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&response.to_string()).expect("JSON was not well-formatted");
        let frames = value["data"].as_array().unwrap();
        let accounts_frame_id = frames
            .into_iter()
            .find(|f| f["data"]["title"] == "Accounts")
            .unwrap()["id"]
            .to_string();
        Ok(accounts_frame_id.clone().replace("\"", ""))
    }

    // pub fn get_data_for_snapshots(
    //     co_file_string: String,
    //     selected_co_started_path: String,
    //     selected_folder_name: String,
    //     snapshot_name: String,
    // ) -> Result<(String, String, String, Option<usize>), String> {
    //     if snapshot_name == CONTEXT_ACCOUNTS_PNG_NAME {
    //         let context_account_lines = get_string_between_two_str_from_string(
    //             co_file_string,
    //             "# Context Accounts:",
    //             "# Validations:",
    //         )?;
    //         let snapshot_image_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "context_accounts.png",
    //         );
    //         let snapshot_markdown_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "context_accounts.md",
    //         );
    //         Ok((
    //             context_account_lines
    //                 .replace("\n- ```rust", "")
    //                 .replace("\n  ```", ""),
    //             snapshot_image_path,
    //             snapshot_markdown_path,
    //             None,
    //         ))
    //     } else if snapshot_name == VALIDATIONS_PNG_NAME {
    //         let validation_lines = get_string_between_two_str_from_string(
    //             co_file_string,
    //             "# Validations:",
    //             "# Miro board frame:",
    //         )?;
    //         let snapshot_image_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "validations.png",
    //         );
    //         let snapshot_markdown_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "validations.md",
    //         );
    //         Ok((
    //             validation_lines,
    //             snapshot_image_path,
    //             snapshot_markdown_path,
    //             None,
    //         ))
    //     } else if snapshot_name == ENTRYPOINT_PNG_NAME {
    //         let RequiredConfig {
    //             program_lib_path, ..
    //         } = BatConfig::get_validated_config()?.required;
    //         let lib_file_string = fs::read_to_string(program_lib_path.clone()).unwrap();
    //         let start_entrypoint_index = lib_file_string
    //             .lines()
    //             .into_iter()
    //             .position(|f| f.contains("pub fn") && f.contains(&selected_folder_name))
    //             .unwrap();
    //         let end_entrypoint_index = lib_file_string
    //             .lines()
    //             .into_iter()
    //             .enumerate()
    //             .position(|(f_index, f)| f.trim() == "}" && f_index > start_entrypoint_index)
    //             .unwrap();
    //         let entrypoint_lines = get_string_between_two_index_from_string(
    //             lib_file_string,
    //             start_entrypoint_index,
    //             end_entrypoint_index,
    //         )?;
    //         let snapshot_image_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "entrypoint.png",
    //         );
    //         let snapshot_markdown_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "entrypoint.md",
    //         );
    //         Ok((
    //             format!(
    //                 "///{}\n\n{}",
    //                 program_lib_path.replace("../", ""),
    //                 entrypoint_lines,
    //             ),
    //             snapshot_image_path,
    //             snapshot_markdown_path,
    //             Some(start_entrypoint_index - 1),
    //         ))
    //     } else {
    //         //
    //         let (handler_string, instruction_file_path, start_index, _) =
    //             batbelt::helpers::get::get_instruction_handler_of_entrypoint(
    //                 selected_folder_name.clone(),
    //             )?;
    //         let snapshot_image_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name.clone()).as_str(),
    //             "handler.png",
    //         );
    //         let snapshot_markdown_path = selected_co_started_path.replace(
    //             format!("{}.md", selected_folder_name).as_str(),
    //             "handler.md",
    //         );
    //         // Handler
    //         Ok((
    //             format!("///{}\n\n{}", instruction_file_path, handler_string),
    //             snapshot_image_path,
    //             snapshot_markdown_path,
    //             Some(start_index - 1),
    //         ))
    //     }
    // }

    // pub fn create_co_figure(
    //     contents: String,
    //     image_path: String,
    //     temporary_markdown_path: String,
    //     index: Option<usize>,
    // ) {
    //     // write the temporary markdown file
    //     fs::write(temporary_markdown_path.clone(), contents).unwrap();
    //     // take the snapshot
    //     if let Some(offset) = index {
    //         take_silicon_snapshot(image_path.clone(), temporary_markdown_path.clone(), offset);
    //     } else {
    //         take_silicon_snapshot(image_path.clone(), temporary_markdown_path.clone(), 1);
    //     }
    //
    //     // delete the markdown
    //     delete_file(temporary_markdown_path);
    // }

    // pub fn take_silicon_snapshot<'a>(
    //     image_path: String,
    //     temporary_markdown_path: String,
    //     index: usize,
    // ) {
    //     let offset = format!("{}", index);
    //     let image_file_name = image_path.split("/").last().unwrap();
    //     let mut args = vec![
    //         "--no-window-controls",
    //         "--language",
    //         "Rust",
    //         "--line-offset",
    //         &offset,
    //         "--theme",
    //         "Monokai Extended",
    //         "--pad-horiz",
    //         "40",
    //         "--pad-vert",
    //         "40",
    //         "--background",
    //         "#d3d4d5",
    //         "--font",
    //         match image_file_name {
    //             ENTRYPOINT_PNG_NAME => "Hack=15",
    //             CONTEXT_ACCOUNTS_PNG_NAME => "Hack=15",
    //             VALIDATIONS_PNG_NAME => "Hack=14",
    //             HANDLER_PNG_NAME => "Hack=11",
    //             _ => "Hack=13",
    //         },
    //         "--output",
    //         &image_path,
    //         &temporary_markdown_path,
    //     ];
    //     if index == 1 {
    //         args.insert(0, "--no-line-number");
    //     }
    //     std::process::Command::new("silicon")
    //         .args(args)
    //         .output()
    //         .unwrap();
    //     // match output {
    //     //     Ok(_) => println!(""),
    //     //     Err(_) => false,
    //     // }
    // }

    // pub fn delete_file(path: String) {
    //     std::process::Command::new("rm")
    //         .args([path])
    //         .output()
    //         .unwrap();
    // }

    // pub fn check_silicon_installed() -> bool {
    //     let output = std::process::Command::new("silicon")
    //         .args(["--version"])
    //         .output();
    //     match output {
    //         Ok(_) => true,
    //         Err(_) => false,
    //     }
    // }

    // pub fn get_item_id_from_miro_url(miro_url: &str) -> String {
    //     // example https://miro.com/app/board/uXjVP7aqTzc=/?moveToWidget=3458764541840480526&cot=14
    //     let frame_id = Url::parse(miro_url).unwrap();
    //     let hash_query: HashMap<_, _> = frame_id.query_pairs().into_owned().collect();
    //     hash_query.get("moveToWidget").unwrap().to_owned()
    // }

    pub async fn get_id_from_response(
        response: Result<reqwest::Response, reqwest::Error>,
    ) -> Result<String, MiroError> {
        match response {
            Ok(response) => {
                let response_string = response.text().await.unwrap();
                let response: Value = serde_json::from_str(&&response_string.as_str()).unwrap();
                Ok(response["id"].to_string().replace("\"", ""))
            }
            Err(err_message) => {
                Err(Report::new(MiroError).attach_printable(err_message.to_string()))
            }
        }
    }

    // pub fn get_frame_id_from_co_file(entrypoint_name: &str) -> Result<String, String> {
    //     // let started_file_path = utils::path::get_auditor_code_overhaul_started_file_path(
    //     //     Some(entrypoint_name.to_string()),
    //     // )?;
    //     let started_file_path = batbelt::path::get_file_path(
    //         FilePathType::CodeOverhaulStarted {
    //             file_name: entrypoint_name.to_string(),
    //         },
    //         true,
    //     );
    //     let miro_url = fs::read_to_string(started_file_path)
    //         .unwrap()
    //         .lines()
    //         .find(|line| line.contains("https://miro.com/app/board/"))
    //         .unwrap()
    //         .to_string();
    //     let frame_id = miro_url
    //         .split("moveToWidget=")
    //         .last()
    //         .unwrap()
    //         .to_string()
    //         .replace("\"", "");
    //     Ok(frame_id)
    // }
}

// #[test]
//
// fn test_get_miro_item_id_from_url() {
//     let miro_url =
//         "https://miro.com/app/board/uXjVPvhKFIg=/?moveToWidget=3458764544363318703&cot=14";
//     let item_id = helpers::get_item_id_from_miro_url(miro_url);
//     println!("item id: {}", item_id);
//     assert_eq!(item_id, "3458764541840480526".to_string())
// }
