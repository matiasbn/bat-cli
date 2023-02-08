use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions::get_functions_metadata_from_program;

use crate::batbelt::metadata::structs::get_structs_metadata_from_program;
use crate::batbelt::metadata::MetadataSection;

use crate::batbelt::path::FilePathType;
use crate::{batbelt, GitCommit};
use colored::Colorize;

use std::io;

pub fn functions() -> Result<(), String> {
    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, false);
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let functions_section = metadata_markdown
        .get_section(&MetadataSection::Functions.to_string())
        .unwrap();
    // // check if empty
    let functions_subsections =
        metadata_markdown.get_section_subsections(functions_section.clone());
    let is_initialized = !functions_section.content.is_empty() || functions_subsections.len() > 0;
    if is_initialized {
        let user_decided_to_continue = batbelt::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("Functions section in metadata.md is already initialized").bright_red()
            )
            .as_str(),
        )
        .unwrap();
        if !user_decided_to_continue {
            panic!("User decided not to continue with the update process for functions metadata")
        }
    }
    let functions_metadata = get_functions_metadata_from_program().unwrap();
    let functions_metadata_sections_vec = functions_metadata
        .iter()
        .map(|function_metadata| {
            function_metadata
                .get_markdown_section(&functions_section.section_header.section_hash.clone())
        })
        .collect::<Vec<_>>();
    metadata_markdown
        .replace_section(
            functions_section.clone(),
            functions_section.clone(),
            functions_metadata_sections_vec,
        )
        .unwrap();
    metadata_markdown.save().unwrap();
    batbelt::git::create_git_commit(GitCommit::UpdateMetadata, None).unwrap();
    Ok(())
}

pub async fn miro() -> Result<(), String> {
    // assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    // let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, true);
    // let metadata_markdown = MarkdownFile::new(&metadata_path);
    // let miro_section = metadata_markdown.clone().get_section_by_title("Miro");
    // let prompt_text = "Please select the Miro metadata section to update";
    // let sections = MIRO_SUBSECTIONS_HEADERS
    //     .into_iter()
    //     .enumerate()
    //     .map(|section| {
    //         if section.0 == 0 {
    //             section.1.green()
    //         } else {
    //             section.1.yellow()
    //         }
    //     })
    //     .collect::<Vec<_>>();
    // let selection = batbelt::cli_inputs::select(prompt_text, sections, None)?;

    // if selection == 1 {
    //     let miro_accounts_subsection_initialized = !miro_section.subsections.is_empty();
    //     if miro_accounts_subsection_initialized {
    //         metadata_helpers::prompt_user_update_section("Miro accounts")?;
    //     };
    //     let structs_section = metadata_markdown.get_section_by_title("Structs");
    //     let account_subsection = structs_section.get_subsection_by_title("Account");
    //     let accounts_structs_names: Vec<String> = account_subsection
    //         .clone()
    //         .subsections
    //         .into_iter()
    //         .map(|subsection| subsection.title)
    //         .collect();
    //     // get colors
    //     let mut miro_stickynote_colors = MiroColor::get_colors_vec();
    //     let mut miro_metadata_vec: Vec<MiroAccountMetadata> = vec![];
    //     for struct_name in accounts_structs_names {
    //         let prompt_text = format!("Please select the color for {}:", struct_name.yellow());
    //         let selection =
    //             batbelt::cli_inputs::select(&prompt_text, miro_stickynote_colors.clone(), None)?;
    //         let selected_color = miro_stickynote_colors[selection].clone();
    //         miro_stickynote_colors.remove(selection);
    //         let miro_metadata = MiroAccountMetadata {
    //             sticky_note_color: selected_color.clone(),
    //             account_name: struct_name,
    //             miro_item_id: "".to_string(),
    //         };
    //         miro_metadata_vec.push(miro_metadata);
    //     }
    //     let mut accounts_frame_url = "".to_string();
    //     if !miro_accounts_subsection_initialized {
    //         // // create frame and parse om accounts frame url
    //         // let frame_id = commands::miro::api::frame::create_frame(
    //         //     "Accounts",
    //         //     MIRO_INITIAL_X_ACCOUNTS_FRAME,
    //         //     MIRO_INITIAL_Y_ACCOUNTS_FRAME,
    //         //     MIRO_WIDTH_ACCOUNTS_FRAME,
    //         //     MIRO_HEIGHT_ACCOUNTS_FRAME,
    //         // )
    //         // .await?
    //         // .id;
    //         // accounts_frame_url = MiroConfig::new().get_frame_url(&frame_id);
    //         let prompt_text = format!(
    //             "Please provide the {} frame url for accounts:",
    //             "Miro".yellow()
    //         );

    //         accounts_frame_url = batbelt::cli_inputs::input(&prompt_text).unwrap();
    //         let frame_id = accounts_frame_url
    //             .split("?moveToWidget=")
    //             .last()
    //             .unwrap()
    //             .split("&cot")
    //             .next()
    //             .unwrap();

    //         for (account_metadata_index, account_metadata) in
    //             miro_metadata_vec.clone().into_iter().enumerate()
    //         {
    //             let x_modifier = account_metadata_index as u64 % MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS;
    //             let y_modifier =
    //                 account_metadata_index as u64 / MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS + 1;
    //             let x_position = MIRO_INITIAL_X_ACCOUNTS_STICKY_NOTE
    //                 + MIRO_OFFSET_X_ACCOUNTS_STICKY_NOTE * x_modifier as i64;
    //             let y_position = MIRO_INITIAL_Y_ACCOUNTS_STICKY_NOTE
    //                 + MIRO_OFFSET_Y_ACCOUNTS_STICKY_NOTE * y_modifier as i64;
    //             let mut sticky_note = MiroStickyNote::new(
    //                 &account_metadata.account_name,
    //                 MiroColor::from_str(&account_metadata.sticky_note_color),
    //                 frame_id,
    //                 x_position,
    //                 y_position,
    //                 MIRO_WIDTH_ACCOUNTS_STICKY_NOTE,
    //             );
    //             sticky_note.deploy();
    //             miro_metadata_vec[account_metadata_index].miro_item_id = sticky_note.item_id;
    //         }
    //     }

    //     // let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
    //     let miro_accounts_string = get_format_miro_accounts_to_result_string(
    //         miro_metadata_vec.clone(),
    //         MIRO_SUBSECTIONS_HEADERS[selection],
    //     );

    //     // parse into metadata.md
    //     let miro_section_content = miro_section.clone().content;
    //     let miro_accounts_subsection_content = miro_section
    //         .get_subsection_by_title("Accounts")
    //         .content
    //         .clone();
    //     let mut new_content = metadata_markdown.clone().content.replace(
    //         miro_section_content.as_str(),
    //         miro_section_content
    //             .replace(
    //                 miro_accounts_subsection_content.as_str(),
    //                 &miro_accounts_string,
    //             )
    //             .as_str(),
    //     );

    //     if !miro_accounts_subsection_initialized {
    //         let replace_text_frame_url = format!(
    //             "{}\n\n{}",
    //             MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER, accounts_frame_url
    //         );
    //         new_content = new_content.replace(
    //             MIRO_ACCOUNTS_SUBSECTION_FRAME_URL_HEADER,
    //             &replace_text_frame_url,
    //         );
    //     }

    //     fs::write(metadata_path, new_content).unwrap();
    //     // create commit

    //     batbelt::git::create_git_commit(GitCommit::UpdateMetadata, None)?;
    return Ok(());
    // } else {
    //     unimplemented!("Entrypoints section not implemented yet")
    // }
}

pub fn structs() -> Result<(), io::Error> {
    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, false);
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let structs_section = metadata_markdown
        .get_section(&MetadataSection::Structs.to_string())
        .unwrap();
    // // check if empty
    let structs_subsections = metadata_markdown.get_section_subsections(structs_section.clone());
    let is_initialized = !structs_section.content.is_empty() || structs_subsections.len() > 0;
    if is_initialized {
        let user_decided_to_continue = batbelt::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("Structs section in metadata.md is already initialized").bright_red()
            )
            .as_str(),
        )
        .unwrap();
        if !user_decided_to_continue {
            panic!("User decided not to continue with the update process for structs metadata")
        }
    }
    let structs_metadata = get_structs_metadata_from_program().unwrap();
    let structs_metadata_sections_vec = structs_metadata
        .iter()
        .map(|struct_metadata| {
            struct_metadata
                .get_markdown_section(&structs_section.section_header.section_hash.clone())
        })
        .collect::<Vec<_>>();
    metadata_markdown
        .replace_section(
            structs_section.clone(),
            structs_section.clone(),
            structs_metadata_sections_vec,
        )
        .unwrap();
    metadata_markdown.save().unwrap();
    batbelt::git::create_git_commit(GitCommit::UpdateMetadata, None).unwrap();
    Ok(())
}
