use crate::batbelt::miro::frame::{MiroCodeOverhaulConfig, MiroFrame};
use crate::batbelt::miro::image::{MiroImage, MiroImageType};
use crate::batbelt::miro::item::MiroItem;
use crate::batbelt::miro::MiroItemType;

use crate::batbelt::metadata::MiroMetadata;
use crate::batbelt::parser::{ParserError, ParserResult};
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::sonar::BatSonar;
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CoderOverhaulTemplatePlaceholders,
};
use crate::batbelt::{silicon, BatEnumerator};
use crate::commands::miro_commands::{miro_command_functions, MiroCommand};
use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{cmp, fs};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulSigner {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulSectionsContent {
    pub state_changes: String,
    pub notes: String,
    pub signers: String,
    pub handler_function_parameters: String,
    pub context_accounts: String,
    pub validations: String,
    pub miro_frame_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeOverhaulParser {
    pub entry_point_name: String,
    pub co_bat_file: BatFile,
    pub signers: Vec<CodeOverhaulSigner>,
    pub section_content: CodeOverhaulSectionsContent,
}

impl CodeOverhaulParser {
    pub fn new_from_entry_point_name(entry_point_name: String) -> ParserResult<Self> {
        let entry_point_name = entry_point_name.trim_end_matches(".md").to_string();
        let bat_file_started = BatFile::CodeOverhaulStarted {
            file_name: entry_point_name.clone(),
        };
        let bat_file_finished = BatFile::CodeOverhaulFinished {
            file_name: entry_point_name.clone(),
        };

        let co_bat_file = if bat_file_started.file_exists().change_context(ParserError)? {
            bat_file_started
        } else if bat_file_finished
            .file_exists()
            .change_context(ParserError)?
        {
            bat_file_finished
        } else {
            return Err(Report::new(ParserError).attach_printable(format!(
                "code-overhaul file not found on to-review or finished for entrypoint: {}",
                entry_point_name
            )));
        };

        let mut new_co_parser = CodeOverhaulParser {
            entry_point_name,
            co_bat_file,
            signers: vec![],
            section_content: CodeOverhaulSectionsContent {
                state_changes: "".to_string(),
                notes: "".to_string(),
                signers: "".to_string(),
                handler_function_parameters: "".to_string(),
                context_accounts: "".to_string(),
                validations: "".to_string(),
                miro_frame_url: "".to_string(),
            },
        };

        new_co_parser.get_sections_content()?;
        new_co_parser.parse_signers()?;
        Ok(new_co_parser)
    }

    fn get_sections_content(&mut self) -> ParserResult<()> {
        self.section_content = CodeOverhaulSectionsContent {
            state_changes: self
                .extract_section_content_from_co_file(CodeOverhaulSection::StateChanges)?,
            notes: self.extract_section_content_from_co_file(CodeOverhaulSection::Notes)?,
            signers: self.extract_section_content_from_co_file(CodeOverhaulSection::Signers)?,
            handler_function_parameters: self.extract_section_content_from_co_file(
                CodeOverhaulSection::HandlerFunctionParameters,
            )?,
            context_accounts: self
                .extract_section_content_from_co_file(CodeOverhaulSection::ContextAccounts)?,
            validations: self
                .extract_section_content_from_co_file(CodeOverhaulSection::Validations)?,
            miro_frame_url: self
                .extract_section_content_from_co_file(CodeOverhaulSection::MiroFrameUrl)?,
        };
        Ok(())
    }

    pub async fn deploy_new_validations_image_for_miro_co_frame(
        &self,
        miro_frame: MiroFrame,
    ) -> ParserResult<MiroImage> {
        let content = self.get_validations_image_content()?;
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
        let content = self.get_context_accounts_image_content()?;
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
        let file_name = miro_command_functions::parse_screenshot_name(title, &miro_frame.title);

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

        let validations_content = self.get_validations_image_content()?;
        let file_name =
            miro_command_functions::parse_screenshot_name("validations", &miro_frame.title);

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

        let ca_content = self.get_context_accounts_image_content()?;
        let file_name =
            miro_command_functions::parse_screenshot_name("context_accounts", &miro_frame.title);

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

    fn get_validations_image_content(&self) -> ParserResult<String> {
        let header = "/// Validations";
        let validations_image_content =
            if self.section_content.validations.contains(
                &CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder(),
            ) {
                CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
            } else {
                self.rust_subsection_matcher(&self.section_content.validations.clone(), true)?
                    .join("\n\n")
            };
        let content = format!("{}\n\n{}", header, validations_image_content);
        Ok(content)
    }

    fn get_context_accounts_image_content(&self) -> ParserResult<String> {
        let header = "/// Context accounts";
        let context_accounts_image_content =
            self.rust_subsection_matcher(&self.section_content.context_accounts, false)?[0].clone();
        let ca_formatted = self.format_trailing_whitespaces(&context_accounts_image_content);
        let content = format!("{}\n\n{}", header, ca_formatted);
        Ok(content)
    }

    fn parse_signers(&mut self) -> ParserResult<()> {
        let signers = self
            .section_content
            .signers
            .clone()
            .lines()
            .filter_map(|line| {
                if line.starts_with("- ")
                    && line.contains(":")
                    && !line.contains(
                        &CoderOverhaulTemplatePlaceholders::PermissionlessFunction.to_placeholder(),
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

    fn extract_section_content_from_co_file(
        &self,
        code_overhaul_section: CodeOverhaulSection,
    ) -> ParserResult<String> {
        let bat_file_content = self
            .co_bat_file
            .read_content(true)
            .change_context(ParserError)?;
        let section_header = code_overhaul_section.to_markdown_header();
        let next_section_header = if code_overhaul_section.get_index_of_type_vec() + 1
            < CodeOverhaulSection::get_type_vec().len()
        {
            CodeOverhaulSection::from_index(code_overhaul_section.get_index_of_type_vec() + 1)
                .to_markdown_header()
        } else {
            "".to_string()
        };
        log::debug!("{bat_file_content}");
        log::debug!("{section_header}");
        log::debug!("{next_section_header}");
        let section_content_regex = Regex::new(&format!(
            r#"({})[\s\S]+({})"#,
            section_header, next_section_header
        ))
        .into_report()
        .change_context(ParserError)?;
        let section_content = section_content_regex
            .find(&bat_file_content)
            .ok_or(ParserError)
            .into_report()?
            .as_str()
            .trim_end_matches(&next_section_header)
            .trim()
            .to_string();
        Ok(section_content)
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
        let rust_regex = Regex::new(r#"```rust([\s\S]*?)```"#)
            .into_report()
            .change_context(ParserError)?;
        let mut max_trailing_ws = 0;
        let mut max_line_length = 0;
        for line in content.lines() {
            max_line_length = cmp::max(max_line_length, line.len());
            max_trailing_ws = cmp::max(max_trailing_ws, BatSonar::get_trailing_whitespaces(line));
        }

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
                                Some("-".repeat(max_line_length + max_trailing_ws))
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
