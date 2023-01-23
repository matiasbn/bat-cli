use crate::{
    commands::metadata::structs::helpers::get_structs_metadata_from_program,
    utils::{self, helpers::get::get_string_between_two_str_from_path},
};
use colored::Colorize;
use std::borrow::BorrowMut;
use std::fs;

pub const STRUCTS_SECTION_HEADER: &str = "## Structs";
pub const FUNCTIONS_SECTION_HEADER: &str = "## Functions";
pub const STRUCT_TYPES_STRING: &[&str] = &["context_accounts", "account", "input", "other"];
pub const STRUCT_SUBSECTIONS_HEADERS: &[&str] = &[
    "### Context Accounts",
    "### Accounts",
    "### Inputs",
    "### Others",
];

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum StructType {
    ContextAccounts,
    Account,
    Input,
    Other,
}

impl StructType {
    fn get_struct_index(&self) -> usize {
        match self {
            StructType::ContextAccounts => 0,
            StructType::Account => 1,
            StructType::Input => 2,
            StructType::Other => 3,
        }
    }

    fn to_string(&self) -> &str {
        let index = self.get_struct_index();
        STRUCT_TYPES_STRING[index]
    }

    fn from_index(index: usize) -> StructType {
        match index {
            0 => StructType::ContextAccounts,
            1 => StructType::Account,
            2 => StructType::Input,
            3 => StructType::Other,
            _ => todo!(),
        }
    }
    fn get_struct_types<'a>() -> Vec<&'a str> {
        let mut result_vec = vec![""; STRUCT_TYPES_STRING.len()];
        result_vec.copy_from_slice(STRUCT_TYPES_STRING);
        result_vec
    }

    fn get_subsection_header(&self) -> &str {
        let index = self.get_struct_index();
        STRUCT_SUBSECTIONS_HEADERS[index]
    }
}
#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl StructMetadata {
    fn new(
        path: String,
        name: String,
        struct_type: StructType,
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
    fn get_content(&self) -> Result<String, String> {
        let content = utils::helpers::get::get_string_between_two_index_from_path(
            self.path.clone(),
            self.start_line_index,
            self.end_line_index,
        )
        .unwrap()
        .clone();
        Ok(content)
    }
}

pub mod structs {
    use super::*;

    pub fn update_structs() -> Result<(), String> {
        let metadata_path = utils::path::get_audit_folder_path(Some("metadata.md".to_string()))?;
        let metadata_structs_content_string = get_string_between_two_str_from_path(
            metadata_path.clone(),
            STRUCTS_SECTION_HEADER,
            FUNCTIONS_SECTION_HEADER,
        )?;
        // check if empty
        let is_initialized =
            self::helpers::check_structs_initialized(&metadata_structs_content_string)?;
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
        let context_accounts_result_string = self::helpers::get_format_structs_to_result_string(
            context_accounts_metadata_vec.clone(),
            StructType::ContextAccounts.get_subsection_header(),
        );
        let accounts_result_string = self::helpers::get_format_structs_to_result_string(
            accounts_metadata_vec.clone(),
            StructType::Account.get_subsection_header(),
        );
        let input_result_string = self::helpers::get_format_structs_to_result_string(
            input_metadata_vec.clone(),
            StructType::Input.get_subsection_header(),
        );
        let other_result_string = self::helpers::get_format_structs_to_result_string(
            other_metadata_vec.clone(),
            StructType::Other.get_subsection_header(),
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
        Ok(())
    }

    pub mod helpers {
        use std::vec;

        use crate::structs::FileInfo;

        use super::*;
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
                .filter(|metadata| metadata.struct_type == StructType::ContextAccounts)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let accounts_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructType::Account)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let input_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructType::Input)
                .map(|f| f.clone())
                .collect::<Vec<_>>();
            let other_metadata_vec = structs_metadata
                .iter()
                .filter(|metadata| metadata.struct_type == StructType::Other)
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
            let struct_types_colored = StructType::get_struct_types()
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
                            let selection_type_enum = StructType::from_index(selection);
                            let struct_first_line =
                                struct_metadata_content.split("\n").next().unwrap();
                            let struct_name = get_struct_name(struct_first_line);
                            let struct_metadata = StructMetadata::new(
                                struct_file_info.path.clone(),
                                struct_name.to_string(),
                                selection_type_enum,
                                start_line_index,
                                end_line_index,
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
                .to_string()
                .clone()
        }

        pub fn get_format_structs_to_result_string(
            structs_vec: Vec<StructMetadata>,
            subsection_header: &str,
        ) -> String {
            let mut sorted_vec = structs_vec.clone();
            sorted_vec.sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
            println!("sorted {:#?}", sorted_vec);
            let mut initial_vec = vec![format!("{}\n", subsection_header.to_string())];
            let mut result_vec = sorted_vec
                .iter()
                .map(|struct_result| {
                    format!(
                        "{}{}{}{}{}",
                        format!("#### {}\n\n", struct_result.name),
                        format!("- type: {}\n", struct_result.struct_type.to_string()),
                        format!("- path: {}\n", struct_result.path),
                        format!("- start_line_index: {}\n", struct_result.start_line_index),
                        format!("- end_line_index: {}\n", struct_result.end_line_index),
                    )
                })
                .collect::<Vec<_>>();
            initial_vec.append(&mut result_vec);
            initial_vec.join("\n")
        }
    }
}
