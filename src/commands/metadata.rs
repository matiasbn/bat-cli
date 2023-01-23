use crate::commands::metadata::miro_helpers::{
    get_miro_accounts_subsection_content_string, get_miro_section_content_string,
    miro_section_is_initialized,
};
use crate::commands::metadata::structs::structs_helpers;
use crate::commands::miro::{MiroConfig, MiroStickyNoteColors};
use crate::structs::FileInfo;
use crate::utils::git::GitCommit;
use crate::{
    commands::metadata::structs::structs_helpers::get_structs_metadata_from_program,
    utils::{self, helpers::get::get_string_between_two_str_from_path},
};
use colored::Colorize;

use std::fs;
use std::vec;

pub const METADATA_END_OF_FILE: &str = "<!-- Miro should be ever the last section -->";
pub const MIRO_SECTION_HEADER: &str = "## Miro";
pub const MIRO_SUBSECTIONS_HEADERS: &[&str] = &["### Entrypoints", "### Accounts"];
pub const METADATA_CONTENT_ACCOUNT_NAME_SECTION: &str = "- account_name:";
pub const METADATA_CONTENT_TYPE_SECTION: &str = "- type:";
pub const METADATA_CONTENT_PATH_SECTION: &str = "- path:";
pub const METADATA_CONTENT_START_LINE_INDEX_SECTION: &str = "- start_line_index:";
pub const METADATA_CONTENT_END_LINE_INDEX_SECTION: &str = "- end_line_index:";
pub const STRUCTS_SECTION_HEADER: &str = "## Structs";
pub const FUNCTIONS_SECTION_HEADER: &str = "## Functions";
pub const STRUCT_TYPES_STRING: &[&str] = &["context_accounts", "account", "input", "other"];
pub const STRUCT_SUBSECTIONS_HEADERS: &[&str] = &[
    "### Context Accounts",
    "### Accounts",
    "### Inputs",
    "### Others",
];

#[derive(Debug, Clone)]
pub struct MiroAccountMetadata {
    color: String,
    account_name: String,
    miro_item_id: String,
}

#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl StructMetadata {
    fn new(
        path: String,
        name: String,
        struct_type: StructMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        StructMetadata {
            path,
            name,
            struct_type,
            start_line_index,
            end_line_index,
        }
    }

    fn new_from_metadata_name(struct_name: &str) -> Self {
        let structs_section_metadata_string =
            structs::structs_helpers::get_validated_struct_metadata_from_name(struct_name).unwrap();
        let path = metadata_helpers::parse_metadata_info_section(
            &structs_section_metadata_string,
            METADATA_CONTENT_PATH_SECTION,
        );
        let struct_type_string = metadata_helpers::parse_metadata_info_section(
            &structs_section_metadata_string,
            METADATA_CONTENT_TYPE_SECTION,
        );
        let struct_type_index = STRUCT_TYPES_STRING
            .to_vec()
            .into_iter()
            .position(|struct_type| struct_type == struct_type_string)
            .unwrap();
        let struct_type = StructMetadataType::from_index(struct_type_index);
        let start_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &structs_section_metadata_string,
            METADATA_CONTENT_START_LINE_INDEX_SECTION,
        )
        .parse()
        .unwrap();
        let end_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &structs_section_metadata_string,
            METADATA_CONTENT_END_LINE_INDEX_SECTION,
        )
        .parse()
        .unwrap();
        StructMetadata::new(
            path,
            struct_name.to_string(),
            struct_type,
            start_line_index,
            end_line_index,
        )
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StructMetadataType {
    ContextAccounts,
    Account,
    Input,
    Other,
}

impl StructMetadataType {
    fn get_struct_metadata_index(&self) -> usize {
        match self {
            StructMetadataType::ContextAccounts => 0,
            StructMetadataType::Account => 1,
            StructMetadataType::Input => 2,
            StructMetadataType::Other => 3,
        }
    }

    fn to_string(&self) -> &str {
        let index = self.get_struct_metadata_index();
        STRUCT_TYPES_STRING[index]
    }

    fn from_index(index: usize) -> StructMetadataType {
        match index {
            0 => StructMetadataType::ContextAccounts,
            1 => StructMetadataType::Account,
            2 => StructMetadataType::Input,
            3 => StructMetadataType::Other,
            _ => todo!(),
        }
    }
    fn get_struct_types<'a>() -> Vec<&'a str> {
        let mut result_vec = vec![""; STRUCT_TYPES_STRING.len()];
        result_vec.copy_from_slice(STRUCT_TYPES_STRING);
        result_vec
    }

    fn get_struct_metadata_subsection_header(&self) -> &str {
        let index = self.get_struct_metadata_index();
        STRUCT_SUBSECTIONS_HEADERS[index]
    }
}

pub fn update_structs() -> Result<(), String> {
    let metadata_path = utils::path::get_audit_folder_path(Some("metadata.md".to_string()))?;
    let metadata_structs_content_string =
        structs::structs_helpers::get_structs_section_content_string()?;
    // check if empty
    let is_initialized =
        structs::structs_helpers::check_structs_initialized(&metadata_structs_content_string)?;
    // prompt the user if he wants to replace
    if is_initialized {
        let user_decided_to_continue = utils::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("Structs in metadata.md are arealready initialized").bright_red()
            )
            .as_str(),
        )?;
        if !user_decided_to_continue {
            panic!("User decided not to continue with the update process for structs metada")
        }
    }
    // get structs in all files
    let (
        context_accounts_metadata_vec,
        accounts_metadata_vec,
        input_metadata_vec,
        other_metadata_vec,
    ) = get_structs_metadata_from_program()?;

    let metadata_content_string = fs::read_to_string(metadata_path.clone()).unwrap();
    let context_accounts_result_string =
        structs::structs_helpers::get_format_structs_to_result_string(
            context_accounts_metadata_vec.clone(),
            StructMetadataType::ContextAccounts.get_struct_metadata_subsection_header(),
        );
    let accounts_result_string = structs::structs_helpers::get_format_structs_to_result_string(
        accounts_metadata_vec.clone(),
        StructMetadataType::Account.get_struct_metadata_subsection_header(),
    );
    let input_result_string = structs::structs_helpers::get_format_structs_to_result_string(
        input_metadata_vec.clone(),
        StructMetadataType::Input.get_struct_metadata_subsection_header(),
    );
    let other_result_string = structs::structs_helpers::get_format_structs_to_result_string(
        other_metadata_vec.clone(),
        StructMetadataType::Other.get_struct_metadata_subsection_header(),
    );

    // parse into metadata.md

    fs::write(
        metadata_path,
        metadata_content_string.replace(
            metadata_structs_content_string.as_str(),
            format!(
                "{}\n\n{}\n{}\n{}\n{}",
                STRUCTS_SECTION_HEADER,
                &context_accounts_result_string,
                &accounts_result_string,
                &input_result_string,
                &other_result_string,
            )
            .as_str(),
        ),
    )
    .unwrap();
    // create commit

    utils::git::create_git_commit(GitCommit::UpdateMetadata, None)?;
    Ok(())
}

pub fn update_miro() -> Result<(), String> {
    assert!(MiroConfig::new().miro_enabled(), "To enable the Miro integration, fill the miro_oauth_access_token in the BatAuditor.toml file");
    let prompt_text = "Please select the Miro metadata section to update";
    let sections = MIRO_SUBSECTIONS_HEADERS
        .into_iter()
        .enumerate()
        .map(|section| {
            if section.0 == 0 {
                section.1.replace("### ", "").green()
            } else {
                section.1.replace("### ", "").yellow()
            }
        })
        .collect::<Vec<_>>();
    let selection = utils::cli_inputs::select(prompt_text, sections, None)?;

    if selection == 1 {
        let miro_section_initialized = miro_section_is_initialized()?;
        if miro_section_initialized {
            metadata_helpers::prompt_user_update_section("Miro")?;
        };
        // get miro accounts subsection
        let miro_accounts_subsection_content = get_miro_accounts_subsection_content_string()?;
        // get Structs accounts names
        let accounts_structs_names = structs_helpers::get_structs_names_from_metadata_file(Some(
            StructMetadataType::Account,
        ))?;
        // get colors
        let mut miro_stickynote_colors = MiroStickyNoteColors::get_colors_vec();
        let mut miro_metadata_vec: Vec<MiroAccountMetadata> = vec![];
        for struct_name in accounts_structs_names {
            let prompt_text = format!("Please select the color for {}:", struct_name.yellow());
            let selection =
                utils::cli_inputs::select(&prompt_text, miro_stickynote_colors.clone(), None)?;
            let selected_color = miro_stickynote_colors[selection].clone();
            miro_stickynote_colors.remove(selection);
            let miro_metadata = MiroAccountMetadata {
                color: selected_color.clone(),
                account_name: struct_name,
                miro_item_id: "".to_string(),
            };
            miro_metadata_vec.push(miro_metadata);
        }
        return Ok(());
    }
    unimplemented!()
}

mod miro_helpers {

    #[allow(unused_imports)]
    use super::*;

    pub fn get_miro_section_content_string() -> Result<String, String> {
        let metadata_path = utils::path::get_audit_folder_path(Some("metadata.md".to_string()))?;
        let metadata_content_string = fs::read_to_string(metadata_path).unwrap();
        let start_index = metadata_content_string
            .lines()
            .position(|line| line.trim() == MIRO_SECTION_HEADER)
            .unwrap();
        let end_index = metadata_content_string
            .lines()
            .position(|line| line.trim() == METADATA_END_OF_FILE)
            .unwrap();
        let miro_section_content = metadata_content_string.lines().collect::<Vec<_>>().to_vec()
            [start_index..end_index]
            .join("\n");
        Ok(miro_section_content)
    }
    pub fn get_miro_accounts_subsection_content_string() -> Result<String, String> {
        let miro_section_content = get_miro_section_content_string()?;
        let start_index = miro_section_content
            .lines()
            .position(|line| line.trim() == MIRO_SUBSECTIONS_HEADERS[1])
            .unwrap();

        let miro_accounts_subsection_content =
            miro_section_content.lines().collect::<Vec<_>>().to_vec()[start_index..].join("\n");
        Ok(miro_accounts_subsection_content)
    }

    pub fn miro_section_is_initialized() -> Result<bool, String> {
        let miro_section_content_string = get_miro_section_content_string()?;
        let metadata_updated_structs = miro_section_content_string
            .lines()
            .into_iter()
            .filter(|l| {
                !l.is_empty()
                    && !MIRO_SUBSECTIONS_HEADERS
                        .iter()
                        .any(|section| l.contains(section))
            })
            .collect::<Vec<_>>();
        Ok(!metadata_updated_structs.is_empty())
    }
}

pub mod structs {
    use super::*;

    // pub fn get_fleetstats_metadata() -> Result<(), String> {
    //     let struct_metadata = StructMetadata::new_from_metadata_name("FleetStats");
    //     println!("struct metadata {:#?}", struct_metadata);
    //     Ok(())
    // }

    pub mod structs_helpers {
        use super::*;

        pub fn get_structs_metadata_from_metadata_file(
            struct_type: Option<StructMetadataType>,
        ) -> Result<Vec<StructMetadata>, String> {
            let structs_section_content = get_structs_section_content_string()?;
            let struct_names = structs_section_content
                .lines()
                .filter(|struct_metatda| struct_metatda.contains("####"))
                .map(|struct_name| struct_name.replace("#### ", "").trim().to_string())
                .collect::<Vec<_>>();
            let mut structs_metadata_vec: Vec<StructMetadata> = vec![];
            for struct_name in struct_names {
                let struct_metadata = StructMetadata::new_from_metadata_name(&struct_name);
                structs_metadata_vec.push(struct_metadata);
            }
            if let Some(metadata_type) = struct_type {
                let filtered_structs = structs_metadata_vec
                    .into_iter()
                    .filter(|struct_metadata| struct_metadata.struct_type == metadata_type)
                    .collect::<Vec<_>>();
                return Ok(filtered_structs);
            }
            Ok(structs_metadata_vec)
        }

        pub fn get_structs_names_from_metadata_file(
            struct_type: Option<StructMetadataType>,
        ) -> Result<Vec<String>, String> {
            let structs_section_content = get_structs_section_content_string()?;
            let struct_names = structs_section_content
                .lines()
                .filter(|struct_metatda| struct_metatda.contains("####"))
                .map(|struct_name| struct_name.replace("#### ", "").trim().to_string())
                .collect::<Vec<_>>();
            let mut structs_metadata_vec: Vec<StructMetadata> = vec![];
            for struct_name in struct_names {
                let struct_metadata = StructMetadata::new_from_metadata_name(&struct_name);
                structs_metadata_vec.push(struct_metadata);
            }
            if let Some(metadata_type) = struct_type {
                structs_metadata_vec = structs_metadata_vec
                    .into_iter()
                    .filter(|struct_metadata| struct_metadata.struct_type == metadata_type)
                    .collect::<Vec<_>>();
            }
            Ok(structs_metadata_vec
                .into_iter()
                .map(|struct_metadata| struct_metadata.name)
                .collect::<Vec<_>>())
        }

        //  fn get_structs_subcsection_from_metadata_file(struct_type:StructMetadataType) -> Result<Vec<StructMetadata>,String>{
        //     let structs_section_content = get_structs_section_content_string()?;
        //     if let
        // }

        pub fn get_structs_section_content_string() -> Result<String, String> {
            let metadata_path =
                utils::path::get_audit_folder_path(Some("metadata.md".to_string()))?;
            let metadata_structs_content_string = get_string_between_two_str_from_path(
                metadata_path.clone(),
                STRUCTS_SECTION_HEADER,
                FUNCTIONS_SECTION_HEADER,
            )?;
            Ok(metadata_structs_content_string)
        }

        pub fn get_validated_struct_metadata_from_name(
            struct_name: &str,
        ) -> Result<String, String> {
            let structs_section_content_string = get_structs_section_content_string()?;
            let struct_metadata_header = get_struct_metadata_header_from_struct_name(struct_name);
            if !structs_section_content_string.contains(&struct_metadata_header) {
                panic!(
                    "Struct {} not found in Structs section of metadata.md",
                    struct_name
                )
            };
            let structs_section_content_last_index = structs_section_content_string
                .lines()
                .collect::<Vec<_>>()
                .len()
                - 1;
            let start_index = structs_section_content_string
                .lines()
                .into_iter()
                .position(|line| line.trim() == (&struct_metadata_header))
                .unwrap();
            let end_index = structs_section_content_string
                .lines()
                .into_iter()
                .enumerate()
                .position(|line| {
                    (line.1.contains("####") && line.0 > start_index)
                        || line.0 == structs_section_content_last_index
                })
                .unwrap();
            let metadata_struct_content =
                utils::helpers::get::get_string_between_two_index_from_string(
                    structs_section_content_string,
                    start_index,
                    end_index,
                )?;
            Ok(metadata_struct_content)
        }

        pub fn get_struct_metadata_header_from_struct_name(struct_name: &str) -> String {
            let struct_metadata_header = format!("#### {}", struct_name);
            struct_metadata_header
        }

        pub fn check_structs_initialized(metadata_structs_content: &str) -> Result<bool, String> {
            let metadata_updated_structs = metadata_structs_content
                .lines()
                .into_iter()
                .filter(|l| {
                    !l.is_empty()
                        && !l.contains(STRUCTS_SECTION_HEADER)
                        && !STRUCT_SUBSECTIONS_HEADERS
                            .iter()
                            .any(|section| l.contains(section))
                })
                .collect::<Vec<_>>();
            Ok(!metadata_updated_structs.is_empty())
        }

        pub fn get_structs_metadata_from_program() -> Result<
            (
                Vec<StructMetadata>,
                Vec<StructMetadata>,
                Vec<StructMetadata>,
                Vec<StructMetadata>,
            ),
            String,
        > {
            let program_path = utils::path::get_program_path()?;
            let program_folder_files_info =
                utils::helpers::get::get_only_files_from_folder(program_path)?;
            let mut structs_metadata: Vec<StructMetadata> = vec![];
            for file_info in program_folder_files_info {
                let mut struct_metadata_result = get_struct_metadata_from_file_info(file_info)?;
                structs_metadata.append(&mut struct_metadata_result);
            }
            let context_accounts_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructMetadataType::ContextAccounts)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let accounts_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructMetadataType::Account)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let input_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructMetadataType::Input)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let other_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructMetadataType::Other)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            Ok((
                context_accounts_metadata_vec,
                accounts_metadata_vec,
                input_metadata_vec,
                other_metadata_vec,
            ))
        }

        pub fn get_struct_metadata_from_file_info(
            struct_file_info: FileInfo,
        ) -> Result<Vec<StructMetadata>, String> {
            let mut struct_metadata_vec: Vec<StructMetadata> = vec![];
            println!(
                "starting the review of the {} file",
                struct_file_info.path.clone().blue()
            );
            let file_info_content = struct_file_info.read_content().unwrap();
            let file_info_content_lines = file_info_content.lines();
            let struct_types_colored = StructMetadataType::get_struct_types()
                .iter()
                .enumerate()
                .map(|f| match f.0 {
                    0 => f.1.red(),
                    1 => f.1.yellow(),
                    2 => f.1.green(),
                    3 => f.1.blue(),
                    _ => todo!(),
                })
                .collect::<Vec<_>>();
            for (content_line_index, content_line) in file_info_content_lines.enumerate() {
                if content_line.contains("pub")
                    && content_line.contains("struct")
                    && content_line.contains("{")
                {
                    let start_line_index = content_line_index;
                    let end_line_index =
                        file_info_content
                            .lines()
                            .enumerate()
                            .find(|(line_index, line)| {
                                line.trim() == "}" && line_index > &start_line_index
                            });
                    if let Some((closing_brace_index, _)) = end_line_index {
                        let end_line_index = closing_brace_index;
                        let struct_metadata_content = file_info_content.lines().collect::<Vec<_>>()
                            [start_line_index..=end_line_index]
                            .to_vec()
                            .join("\n");
                        println!(
                            "possible struct found at {}",
                            format!(
                                "{}:{}",
                                struct_file_info.path.clone(),
                                content_line_index + 1
                            )
                            .magenta()
                        );
                        let prompt_text = format!(
                            "Are these lines a {}?:\n{}",
                            "Struct".red(),
                            struct_metadata_content.green()
                        );
                        let is_struct = utils::cli_inputs::select_yes_or_no(&prompt_text)?;
                        if is_struct {
                            let prompt_text = "Select the type of struct:";
                            let selection = utils::cli_inputs::select(
                                prompt_text,
                                struct_types_colored.clone(),
                                None,
                            )?;
                            let selection_type_enum = StructMetadataType::from_index(selection);
                            let struct_first_line =
                                struct_metadata_content.split("\n").next().unwrap();
                            let struct_name = get_struct_name(struct_first_line);
                            let struct_metadata = StructMetadata::new(
                                struct_file_info.path.clone(),
                                struct_name.to_string(),
                                selection_type_enum,
                                start_line_index + 1,
                                end_line_index + 1,
                            );
                            struct_metadata_vec.push(struct_metadata);
                        }
                    };
                }
            }
            println!(
                "finishing the review of the {} file",
                struct_file_info.path.clone().blue()
            );
            Ok(struct_metadata_vec)
        }

        fn get_struct_name(struct_line: &str) -> String {
            struct_line.split_whitespace().collect::<Vec<_>>()[2]
                .split("<")
                .next()
                .unwrap()
                .replace(":", "")
                .to_string()
                .clone()
        }

        pub fn get_format_structs_to_result_string(
            structs_vec: Vec<StructMetadata>,
            subsection_header: &str,
        ) -> String {
            let mut sorted_vec = structs_vec.clone();
            sorted_vec.sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
            let mut initial_vec = vec![format!("{}\n", subsection_header.to_string())];
            let mut result_vec = sorted_vec
                .iter()
                .map(|struct_result| {
                    format!(
                        "{}{}{}{}{}",
                        format!("#### {}\n\n", struct_result.name),
                        format!(
                            "{} {}\n",
                            METADATA_CONTENT_TYPE_SECTION,
                            struct_result.struct_type.to_string()
                        ),
                        format!("{} {}\n", METADATA_CONTENT_PATH_SECTION, struct_result.path),
                        format!(
                            "{} {}\n",
                            METADATA_CONTENT_START_LINE_INDEX_SECTION,
                            struct_result.start_line_index
                        ),
                        format!(
                            "{} {}\n",
                            METADATA_CONTENT_END_LINE_INDEX_SECTION, struct_result.end_line_index
                        ),
                    )
                })
                .collect::<Vec<_>>();
            initial_vec.append(&mut result_vec);
            initial_vec.join("\n")
        }

        // pub fn get_structs_metadata_content()-> {

        // }
    }
}

pub mod metadata_helpers {
    #[allow(unused_imports)]
    use super::*;

    pub fn parse_metadata_info_section(metadata_info_content: &str, section: &str) -> String {
        let path = metadata_info_content
            .lines()
            .find(|line| line.contains(section))
            .unwrap()
            .replace(section, "")
            .trim()
            .to_string();
        path
    }

    pub fn prompt_user_update_section(section_name: &str) -> Result<(), String> {
        let user_decided_to_continue = utils::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("{} in metadata.md are arealready initialized", section_name).bright_red()
            )
            .as_str(),
        )?;
        if !user_decided_to_continue {
            panic!(
                "User decided not to continue with the update process for {} metada",
                section_name
            )
        }
        Ok(())
    }
}
