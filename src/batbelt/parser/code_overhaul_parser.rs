use crate::batbelt::metadata::code_overhaul_metadata::CodeOverhaulSignerMetadata;
use crate::batbelt::metadata::{BatMetadata, MetadataId};
use crate::batbelt::miro::frame::MiroFrame;
use crate::batbelt::miro::image::MiroImage;
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::MiroItemType;
use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;
use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::code_overhaul_template::CoderOverhaulTemplatePlaceholders;
use crate::commands::miro_commands::MiroCommand;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulParser {
    pub entry_point_name: String,
    pub validations: Vec<String>,
    pub signers: Vec<CodeOverhaulSignerMetadata>,
    pub context_accounts_content: String,
}

impl CodeOverhaulParser {
    pub fn new_from_entry_point_name(entry_point_name: String) -> ParserResult<Self> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let co_metadata = bat_metadata
            .get_code_overhaul_metadata_by_entry_point_name(entry_point_name.clone())
            .change_context(ParserError)?;
        Ok(Self {
            entry_point_name,
            validations: co_metadata.validations,
            signers: co_metadata.signers,
            context_accounts_content: co_metadata.context_accounts_content,
        })
    }

    pub async fn get_validations_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let (validations_x_position, validations_y_position) = (3000, 500);
        let header = "/// Validations";
        let validations_image_content = if self.validations.is_empty() {
            CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
        } else {
            self.validations.clone().join("\n\n")
        };
        let content = format!("{}\n\n{}", header, validations_image_content);
        let validations_image = self
            .deploy_image_and_update_position(
                content,
                "validations",
                miro_frame,
                validations_x_position,
                validations_y_position,
            )
            .await?;
        Ok(validations_image)
    }

    // let validations_section = CodeOverhaulSection::Validations
    // .get_section_content(Some(entrypoint_parser.clone()))
    // .change_context(CommandError)?;
    // let val_sec_formatted = validations_section
    // .lines()
    // .filter_map(|line| {
    // if line.trim() == "# Validations:" {
    // return Some("/// Validations".to_string());
    // };
    // if line.trim() == "- ```rust" || line.trim() == "```" {
    // return Some("".to_string());
    // }
    // Some(line.to_string())
    // })
    // .collect::<Vec<_>>()
    // .join("\n");

    pub async fn get_context_accounts_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let (context_accounts_x_position, context_accounts_y_position) = (2200, 350);
        let header = "/// Context accounts";
        let context_accounts_image_content = self.context_accounts_content.clone();
        let ca_lines = context_accounts_image_content.lines();
        let trailing_ws_first_line =
            BatSonar::get_trailing_whitespaces(ca_lines.clone().next().unwrap());
        let ca_formatted = ca_lines
            .map(|line| {
                let trailing_ws = BatSonar::get_trailing_whitespaces(line);
                format!(
                    "{}{}",
                    " ".repeat(trailing_ws - trailing_ws_first_line),
                    line.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let content = format!("{}\n\n{}", header, ca_formatted);
        let validations_image = self
            .deploy_image_and_update_position(
                content,
                "context_accounts",
                miro_frame,
                context_accounts_x_position,
                context_accounts_y_position,
            )
            .await?;
        Ok(validations_image)
    }

    // let context_accounts_section = CodeOverhaulSection::ContextAccounts
    // .get_section_content(Some(entrypoint_parser.clone()))
    // .change_context(CommandError)?;
    // let ca_formatted = context_accounts_section
    // .lines()
    // .filter_map(|line| {
    // if line.trim() == "# Context accounts:" {
    // return Some("/// Context accounts".to_string());
    // };
    // if line.trim() == "- ```rust" || line.trim() == "```" {
    // return None;
    // }
    // Some(line.to_string())
    // })
    // .collect::<Vec<_>>()
    // .join("\n");

    async fn deploy_image_and_update_position(
        &self,
        content: String,
        title: &str,
        miro_frame: MiroFrame,
        x_position: i64,
        y_position: i64,
    ) -> ParserResult<MiroImage> {
        let file_name = MiroCommand::parse_screenshot_name(title, &miro_frame.title);
        println!(
            "\nCreating {}{} in {} frame",
            file_name.green(),
            ".png".green(),
            miro_frame.title.green()
        );

        let auditor_figures_path = BatFolder::AuditorFigures
            .get_path(false)
            .change_context(ParserError)?;

        let sc_path =
            silicon::create_figure(&content, &auditor_figures_path, &file_name, 0, None, false);
        let mut miro_image = MiroImage::new_from_file_path(&sc_path, &miro_frame.item_id);

        miro_image.deploy().await.change_context(ParserError)?;

        fs::remove_file(&sc_path)
            .into_report()
            .change_context(ParserError)?;

        let miro_item = MiroItem::new(
            &miro_image.item_id,
            &miro_frame.item_id,
            x_position,
            y_position,
            MiroItemType::Image,
        );

        miro_item.update_item_parent_and_position().await;

        Ok(miro_image)
    }
}
