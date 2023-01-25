use reqwest;
use serde_json::*;
use std::result::Result;

pub struct MiroShapeStyle {
    fill_color: String,
    fill_opacity: String,
    font_family: String,
    font_size: String,
    border_color: String,
    border_width: String,
    border_opacity: String,
    border_style: String,
    text_align: String,
    text_align_vertical: String,
    color: String,
}

impl MiroShapeStyle {
    pub fn new(
        fill_color: String,
        fill_opacity: String,
        font_family: String,
        font_size: String,
        border_color: String,
        border_width: String,
        border_opacity: String,
        border_style: String,
        text_align: String,
        text_align_vertical: String,
        color: String,
    ) -> Self {
        MiroShapeStyle {
            fill_color,
            fill_opacity,
            font_family,
            font_size,
            border_color,
            border_width,
            border_opacity,
            border_style,
            text_align,
            text_align_vertical,
            color,
        }
    }
    pub fn new_from_hex_fill_color(fill_color: &str) -> Self {
        MiroShapeStyle {
            fill_color: fill_color.to_string(),
            fill_opacity: "1.0".to_string(),
            font_family: "open_sans".to_string(),
            font_size: "36".to_string(),
            border_color: "#1a1a1a".to_string(),
            border_width: "2.0".to_string(),
            border_opacity: "1.0".to_string(),
            border_style: "normal".to_string(),
            text_align: "center".to_string(),
            text_align_vertical: "middle".to_string(),
            color: "#1a1a1a".to_string(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct MiroShape {
    x_position: i32,
    y_position: i32,
    width: i32,
    height: i32,
    content: String,
}

impl MiroShape {
    pub fn new(x_position: i32, y_position: i32, width: i32, height: i32, content: String) -> Self {
        MiroShape {
            x_position,
            y_position,
            width,
            height,
            content,
        }
    }

    pub async fn create_shape_in_frame(
        &self,
        miro_shape_style: MiroShapeStyle,
        frame_id: &str,
    ) -> Result<(), String> {
        commands::miro::shape::api::create_shape(self.clone(), miro_shape_style, frame_id).await?;
        Ok(())
    }
}

use crate::commands::{
    self,
    miro::{api::helpers::get_id_from_response, MiroConfig},
};

pub mod api {
    use super::*;
    use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

    pub async fn create_shape(
        miro_shape: MiroShape,
        miro_shape_style: MiroShapeStyle,
        miro_frame_id: &str,
    ) -> Result<String, String> {
        let MiroConfig {
            access_token,
            board_id,
            ..
        } = MiroConfig::new();
        let client = reqwest::Client::new();
        let response = client
            .post(format!("https://api.miro.com/v2/boards/{board_id}/shapes",))
            .body(
                json!({
                    "data": {
                        "content": miro_shape.content,
                        "shape": "rectangle"
                   },
                   "style": {
                    "fillColor":  miro_shape_style.fill_color,
                    "fillOpacity": miro_shape_style.fill_opacity,
                    "fontFamily": miro_shape_style.font_family,
                    "fontSize": miro_shape_style.font_size,
                    "borderColor":  miro_shape_style.border_color,
                    "borderWidth": miro_shape_style.border_width,
                    "borderOpacity": miro_shape_style.border_opacity,
                    "borderStyle": miro_shape_style.border_style,
                    "textAlign": miro_shape_style.text_align,
                    "textAlignVertical": miro_shape_style.text_align_vertical,
                    "color":  miro_shape_style.color,
                  },
                   "position": {
                        "origin": "center",
                        "x": miro_shape.x_position,
                        "y": miro_shape.y_position
                   },
                   "geometry": {
                    "height": miro_shape.height,
                    "width": miro_shape.width
                   },
                   "parent": {
                        "id": miro_frame_id
                   }
                })
                .to_string(),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .send()
            .await
            .unwrap();
        let id = get_id_from_response(response).await;
        Ok(id)
    }
}
