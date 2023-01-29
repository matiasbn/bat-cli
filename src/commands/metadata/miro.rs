use crate::commands;
use crate::commands::metadata::metadata_helpers;
use crate::commands::metadata::structs::structs_helpers;
use crate::commands::miro::{MiroColors, MiroConfig};

use crate::constants::{
    MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS, MIRO_INITIAL_X_ACCOUNTS_STICKY_NOTE,
    MIRO_INITIAL_Y_ACCOUNTS_STICKY_NOTE, MIRO_OFFSET_X_ACCOUNTS_STICKY_NOTE,
    MIRO_OFFSET_Y_ACCOUNTS_STICKY_NOTE,
};
use crate::markdown::{MardkownFile, MarkdownSection, MarkdownSectionLevel};
use crate::structs::FileInfo;
use crate::utils::git::GitCommit;

use crate::utils::path::{FilePathType, FolderPathType};
use crate::{
    commands::metadata::structs::structs_helpers::get_structs_metadata_from_program, utils,
};
use colored::Colorize;

use std::fs;
use std::vec;

pub const MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER: &str = "### Accounts frame url";
pub const MIRO_SUBSECTIONS_HEADERS: &[&str] = &["## Entrypoints", "## Accounts"];
pub const METADATA_CONTENT_STICKY_NOTE_COLOR_SECTION: &str = "- sticky_note_color:";
pub const METADATA_CONTENT_MIRO_ITEM_ID_SECTION: &str = "- miro_item_id:";

#[derive(Debug, Clone)]
pub struct MiroAccountMetadata {
    sticky_note_color: String,
    account_name: String,
    miro_item_id: String,
}

pub async fn update_miro() -> Result<(), String> {
    assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
    let metadata_markdown = MardkownFile::new(&metadata_path);
    let miro_section = metadata_markdown.clone().get_section_by_title("Miro");
    let prompt_text = "Please select the Miro metadata section to update";
    let sections = MIRO_SUBSECTIONS_HEADERS
        .into_iter()
        .enumerate()
        .map(|section| {
            if section.0 == 0 {
                section.1.green()
            } else {
                section.1.yellow()
            }
        })
        .collect::<Vec<_>>();
    let selection = utils::cli_inputs::select(prompt_text, sections, None)?;

    if selection == 1 {
        let miro_accounts_subsection_initialized = !miro_section.subsections.is_empty();
        if miro_accounts_subsection_initialized {
            metadata_helpers::prompt_user_update_section("Miro accounts")?;
        };
        let structs_section = metadata_markdown.clone().get_section_by_title("Structs");
        let account_subsection = structs_section.get_subsection_by_title("Account");
        let accounts_structs_names: Vec<String> = account_subsection
            .clone()
            .subsections
            .into_iter()
            .map(|subsection| subsection.title)
            .collect();
        // get colors
        let mut miro_stickynote_colors = MiroColors::get_colors_vec();
        let mut miro_metadata_vec: Vec<MiroAccountMetadata> = vec![];
        for struct_name in accounts_structs_names {
            let prompt_text = format!("Please select the color for {}:", struct_name.yellow());
            let selection =
                utils::cli_inputs::select(&prompt_text, miro_stickynote_colors.clone(), None)?;
            let selected_color = miro_stickynote_colors[selection].clone();
            miro_stickynote_colors.remove(selection);
            let miro_metadata = MiroAccountMetadata {
                sticky_note_color: selected_color.clone(),
                account_name: struct_name,
                miro_item_id: "".to_string(),
            };
            miro_metadata_vec.push(miro_metadata);
        }
        let mut accounts_frame_url = "".to_string();
        if !miro_accounts_subsection_initialized {
            // // create frame and parse om accounts frame url
            // let frame_id = commands::miro::api::frame::create_frame(
            //     "Accounts",
            //     MIRO_INITIAL_X_ACCOUNTS_FRAME,
            //     MIRO_INITIAL_Y_ACCOUNTS_FRAME,
            //     MIRO_WIDTH_ACCOUNTS_FRAME,
            //     MIRO_HEIGHT_ACCOUNTS_FRAME,
            // )
            // .await?
            // .id;
            // accounts_frame_url = MiroConfig::new().get_frame_url(&frame_id);
            let prompt_text = format!(
                "Please provide the {} frame url for accounts:",
                "Miro".yellow()
            );

            accounts_frame_url = utils::cli_inputs::input(&prompt_text).unwrap();
            let frame_id = accounts_frame_url
                .split("?moveToWidget=")
                .last()
                .unwrap()
                .split("&cot")
                .next()
                .unwrap();

            for (account_metadata_index, account_metadata) in
                miro_metadata_vec.clone().into_iter().enumerate()
            {
                let x_modifier = account_metadata_index as i32 % MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS;
                let y_modifier =
                    account_metadata_index as i32 / MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS + 1;
                let x_position = MIRO_INITIAL_X_ACCOUNTS_STICKY_NOTE
                    + MIRO_OFFSET_X_ACCOUNTS_STICKY_NOTE * x_modifier;
                let y_position = MIRO_INITIAL_Y_ACCOUNTS_STICKY_NOTE
                    + MIRO_OFFSET_Y_ACCOUNTS_STICKY_NOTE * y_modifier;
                let sticky_note_id = commands::miro::api::sticky_note::create_sticky_note(
                    account_metadata.account_name,
                    account_metadata.sticky_note_color,
                    frame_id.to_string(),
                    x_position as i32,
                    y_position,
                )
                .await;
                miro_metadata_vec[account_metadata_index].miro_item_id = sticky_note_id;
            }
        }

        // let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
        let miro_accounts_string = miro_helpers::get_format_miro_accounts_to_result_string(
            miro_metadata_vec.clone(),
            MIRO_SUBSECTIONS_HEADERS[selection],
        );

        // parse into metadata.md
        let miro_section_content = miro_section.clone().content;
        let miro_accounts_subsection_content = miro_section
            .get_subsection_by_title("Accounts")
            .content
            .clone();
        let mut new_content = metadata_markdown.clone().content.replace(
            miro_section_content.as_str(),
            miro_section_content
                .replace(
                    miro_accounts_subsection_content.as_str(),
                    &miro_accounts_string,
                )
                .as_str(),
        );

        if !miro_accounts_subsection_initialized {
            let replace_text_frame_url = format!(
                "{}\n\n{}",
                MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER, accounts_frame_url
            );
            new_content = new_content.replace(
                MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER,
                &replace_text_frame_url,
            );
        }

        fs::write(metadata_path, new_content).unwrap();
        // create commit

        utils::git::create_git_commit(GitCommit::UpdateMetadata, None)?;
        return Ok(());
    } else {
        unimplemented!("Entrypoints section not implemented yet")
    }
}

mod miro_helpers {

    #[allow(unused_imports)]
    use super::*;

    pub fn get_format_miro_accounts_to_result_string(
        miro_accounts_vec: Vec<MiroAccountMetadata>,
        subsection_header: &str,
    ) -> String {
        let mut sorted_vec = miro_accounts_vec.clone();
        sorted_vec.sort_by(|miro_a, miro_b| miro_a.account_name.cmp(&miro_b.account_name));
        let mut initial_vec = vec![format!("{}\n\n", subsection_header.to_string())];
        let mut result_vec = sorted_vec
            .iter()
            .enumerate()
            .map(|(miro_result_index, miro_result)| {
                format!(
                    "{}{}{}",
                    format!("### {}\n\n", miro_result.account_name),
                    format!(
                        "{} {}\n",
                        METADATA_CONTENT_STICKY_NOTE_COLOR_SECTION,
                        miro_result.sticky_note_color.to_string()
                    ),
                    if miro_result_index == sorted_vec.len() - 1 {
                        // last
                        format!(
                            "{} {}",
                            METADATA_CONTENT_MIRO_ITEM_ID_SECTION, miro_result.miro_item_id
                        )
                    } else {
                        format!(
                            "{} {}\n\n",
                            METADATA_CONTENT_MIRO_ITEM_ID_SECTION, miro_result.miro_item_id
                        )
                    }
                )
            })
            .collect::<Vec<_>>();
        initial_vec.append(&mut result_vec);
        initial_vec.join("")
    }
}
