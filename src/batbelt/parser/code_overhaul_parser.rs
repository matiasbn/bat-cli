use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::image::{MiroImage, MiroImageType};
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::MiroItemType;

use crate::batbelt::metadata::MiroMetadata;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::silicon;
use crate::batbelt::sonar::BatSonar;
use crate::batbelt::templates::code_overhaul_template::CoderOverhaulTemplatePlaceholders;
use crate::commands::miro_commands::MiroCommand;
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
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
    pub co_started_bat_file: BatFile,
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
                entry_point_name
            )));
        }
        let mut new_co_parser = CodeOverhaulParser {
            entry_point_name,
            co_started_bat_file: bat_file,
            validations: vec![],
            signers: vec![],
            context_accounts_content: "".to_string(),
        };
        new_co_parser.get_signers()?;
        new_co_parser.get_validations()?;
        new_co_parser.get_context_accounts_content()?;
        Ok(new_co_parser)
    }

    pub async fn deploy_new_validations_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let content = self.get_validations_image_content();
        let (validations_x_position, validations_y_position) =
            MiroCodeOverhaulConfig::Validations.get_positions();
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

    pub async fn deploy_new_context_accounts_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let content = self.get_context_accounts_image_content();
        let (context_accounts_x_position, context_accounts_y_position) =
            MiroCodeOverhaulConfig::ContextAccount.get_positions();
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

    async fn deploy_image_and_update_position(
        &self,
        content: String,
        title: &str,
        miro_frame: MiroFrame,
        x_position: i64,
        y_position: i64,
    ) -> ParserResult<MiroImage> {
        let file_name = MiroCommand::parse_screenshot_name(title, &miro_frame.title);

        let sc_path = self.create_screenshot_with_silicon(content.clone(), &file_name)?;

        println!(
            "\nCreating {}{} in {} frame",
            file_name.green(),
            ".png".green(),
            miro_frame.title.green()
        );

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

    pub async fn update_validations_screenshot(&self) -> ParserResult<()> {
        let miro_co_metadata =
            MiroMetadata::get_co_metadata_by_entrypoint_name(self.entry_point_name.clone())
                .change_context(ParserError)?;
        let validations_image_id = miro_co_metadata.validations_image_id;
        let miro_frame_id = miro_co_metadata.miro_frame_id;
        let miro_frame = MiroFrame::new_from_item_id(&miro_frame_id)
            .await
            .change_context(ParserError)?;
        let mut validations_image =
            MiroImage::new_from_item_id(&validations_image_id, MiroImageType::FromPath)
                .await
                .change_context(ParserError)?;

        let validations_content = self.get_validations_image_content();
        let file_name = MiroCommand::parse_screenshot_name("validations", &miro_frame.title);

        let validations_path =
            self.create_screenshot_with_silicon(validations_content, &file_name)?;
        println!(
            "\nUpdating validations screenshot in {} frame",
            miro_frame.title.green()
        );

        validations_image
            .update_from_path(&validations_path)
            .await
            .change_context(ParserError)?;

        fs::remove_file(&validations_path)
            .into_report()
            .change_context(ParserError)?;
        Ok(())
    }

    pub async fn update_context_accounts_screenshot(&self) -> ParserResult<()> {
        let miro_co_metadata =
            MiroMetadata::get_co_metadata_by_entrypoint_name(self.entry_point_name.clone())
                .change_context(ParserError)?;
        let context_accounts_image_id = miro_co_metadata.context_accounts_image_id;
        let miro_frame_id = miro_co_metadata.miro_frame_id;
        let miro_frame = MiroFrame::new_from_item_id(&miro_frame_id)
            .await
            .change_context(ParserError)?;

        let mut ca_image =
            MiroImage::new_from_item_id(&context_accounts_image_id, MiroImageType::FromPath)
                .await
                .change_context(ParserError)?;

        let ca_content = self.get_context_accounts_image_content();
        let file_name = MiroCommand::parse_screenshot_name("context_accounts", &miro_frame.title);

        let ca_path = self.create_screenshot_with_silicon(ca_content, &file_name)?;
        println!(
            "\nUpdating context accounts screenshot in {} frame",
            miro_frame.title.green()
        );

        ca_image
            .update_from_path(&ca_path)
            .await
            .change_context(ParserError)?;

        fs::remove_file(&ca_path)
            .into_report()
            .change_context(ParserError)?;
        Ok(())
    }

    fn create_screenshot_with_silicon(
        &self,
        content: String,
        file_name: &str,
    ) -> ParserResult<String> {
        let auditor_figures_path = BatFolder::AuditorFigures
            .get_path(false)
            .change_context(ParserError)?;

        let sc_path =
            silicon::create_figure(&content, &auditor_figures_path, file_name, 0, None, false);
        Ok(sc_path)
    }

    fn get_validations_image_content(&self) -> String {
        let header = "/// Validations";
        let validations_image_content = if self.validations.is_empty() {
            CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
        } else {
            self.validations
                .clone()
                .into_iter()
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        let content = format!("{}\n\n{}", header, validations_image_content);
        content
    }

    fn get_context_accounts_image_content(&self) -> String {
        let header = "/// Context accounts";
        let context_accounts_image_content = self.context_accounts_content.clone();
        let ca_formatted = self.format_trailing_whitespaces(&context_accounts_image_content);
        let content = format!("{}\n\n{}", header, ca_formatted);
        content
    }

    fn get_signers(&mut self) -> ParserResult<()> {
        let signers_section_regex = Regex::new(r"# Signers:[\s\S]*?#").unwrap();
        let bat_file_content = self
            .co_started_bat_file
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
                if line.starts_with("- ")
                    && !line.contains(
                        &CoderOverhaulTemplatePlaceholders::NoSignersDetected.to_placeholder(),
                    )
                {
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

    fn get_validations(&mut self) -> ParserResult<()> {
        let validations_section_regex = Regex::new(r"# Validations:[\s\S]*?\n# Miro")
            .into_report()
            .change_context(ParserError)?;
        let bat_file_content = self
            .co_started_bat_file
            .read_content(true)
            .change_context(ParserError)?;
        let validations_section_content = validations_section_regex
            .find(&bat_file_content)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .to_string();
        let validations = self.rust_subsection_matcher(&validations_section_content, true)?;
        self.validations = validations;
        Ok(())
    }

    fn get_context_accounts_content(&mut self) -> ParserResult<()> {
        let bat_file_content = self
            .co_started_bat_file
            .read_content(true)
            .change_context(ParserError)?;
        let ca_section_regex = Regex::new(r"# Context accounts:[\s\S]*?\n# Validations")
            .into_report()
            .change_context(ParserError)?;
        let ca_section_content = ca_section_regex
            .find(&bat_file_content)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .to_string();
        self.context_accounts_content =
            self.rust_subsection_matcher(&ca_section_content, false)?[0].clone();
        Ok(())
    }

    fn format_trailing_whitespaces(&self, content: &str) -> String {
        let content_lines = content.lines();
        let trailing_ws_first_line =
            BatSonar::get_trailing_whitespaces(content_lines.clone().next().unwrap());

        content_lines
            .map(|line| {
                let trailing_ws = BatSonar::get_trailing_whitespaces(line);
                format!(
                    "{}{}",
                    " ".repeat(trailing_ws - trailing_ws_first_line),
                    line.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rust_subsection_matcher(
        &self,
        content: &str,
        use_separator: bool,
    ) -> ParserResult<Vec<String>> {
        let rust_regex =
            Regex::new(r"- ```rust[\s]+[\s 'A-Za-z0-9−()?._=@:><!&{}^;/+#\[\],]+[\s]+```")
                .into_report()
                .change_context(ParserError)?;
        if rust_regex.is_match(content) {
            return Ok(rust_regex
                .find_iter(content)
                .map(|regex_match| {
                    regex_match
                        .as_str()
                        .to_string()
                        .lines()
                        .filter_map(|line| {
                            if !line.contains("```") {
                                Some(line.to_string())
                            } else if use_separator {
                                Some("-".repeat(20))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .collect::<Vec<_>>());
        }
        Ok(vec![])
    }
}
