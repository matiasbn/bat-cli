use crate::batbelt::metadata::{BatMetadata, MetadataId};
use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::image::MiroImage;
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::MiroItemType;
use crate::batbelt::parser::solana_account_parser::SolanaAccountType;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::path::{BatFile, BatFolder};
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
pub struct CodeOverhaulSigner {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulParser {
    pub entry_point_name: String,
    pub bat_file: BatFile,
    pub validations: Vec<String>,
    pub signers: Vec<CodeOverhaulSigner>,
    pub context_accounts_content: String,
}

impl CodeOverhaulParser {
    pub fn new_from_entry_point_name(entry_point_name: String) -> ParserResult<Self> {
        let bat_file = BatFile::CodeOverhaulStarted {
            file_name: entry_point_name.clone(),
        };
        if !bat_file.file_exists().change_context(ParserError)? {
            return Err(Report::new(ParserError).attach_printable(format!(
                "code-overhaul file started not found for entrypoint: {}",
                entry_point_name.clone()
            )));
        }
        let mut new_co_parser = CodeOverhaulParser {
            entry_point_name,
            bat_file,
            validations: vec![],
            signers: vec![],
            context_accounts_content: "".to_string(),
        };
        new_co_parser.get_signers()?;
        // let co_metadata = bat_metadata
        //     .get_code_overhaul_metadata_by_entry_point_name(entry_point_name.clone())
        //     .change_context(ParserError)?;
        Ok(new_co_parser)
    }

    fn get_signers(&mut self) -> ParserResult<()> {
        let signers_section_regex = Regex::new(r"# Signers:[\s\S]*?#").unwrap();
        let bat_file_content = self
            .bat_file
            .read_content(true)
            .change_context(ParserError)?;
        let signers = signers_section_regex
            .find(&bat_file_content)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .to_string()
            .lines()
            .filter_map(|line| {
                if line.starts_with("- ") {
                    let mut line_split = line.trim_start_matches("- ").split(": ");
                    Some(CodeOverhaulSigner {
                        name: line_split.next().unwrap().to_string(),
                        description: line_split.next().unwrap().to_string(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        self.signers = signers;
        Ok(())
    }

    // fn get_validations(&mut self) -> ParserResult<()> {
    //     let validations_section_regex = Regex::new(r"# Validations:[\s\S]*?#").unwrap();
    //     let bat_file_content = self
    //         .bat_file
    //         .read_content(true)
    //         .change_context(ParserError)?;
    //     let validations_section_content = validations_section_regex
    //         .find(&bat_file_content)
    //         .ok_or(ParserError)
    //         .into_report()?
    //         .as_str()
    //         .to_string();
    //     self.signers = signers;
    //     Ok(())
    // }

    pub async fn get_validations_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let (validations_x_position, validations_y_position) =
            MiroCodeOverhaulConfig::Validations.get_positions();
        let header = "/// Validations";
        let validations_image_content = if self.validations.is_empty() {
            CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
        } else {
            self.validations.clone().join("\n\n")
        };
        // let validations_section = CodeOverhaulSection::Validations
        //     .get_section_content(Some(entrypoint_parser.clone()))
        //     .change_context(CommandError)?;
        // let val_sec_formatted = validations_section
        //     .lines()
        //     .filter_map(|line| {
        //         if line.trim() == "# Validations:" {
        //             return Some("/// Validations".to_string());
        //         };
        //         if line.trim() == "- ```rust" || line.trim() == "```" {
        //             return Some("".to_string());
        //         }
        //         Some(line.to_string())
        //     })
        //     .collect::<Vec<_>>()
        //     .join("\n");
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

    pub async fn get_context_accounts_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let (context_accounts_x_position, context_accounts_y_position) =
            MiroCodeOverhaulConfig::ContextAccount.get_positions();
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
        println!(
            "Updating the position of {}{}\n",
            file_name.green(),
            ".png".green(),
        );

        miro_item.update_item_parent_and_position().await;

        Ok(miro_image)
    }
}
