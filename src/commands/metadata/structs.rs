use crate::markdown::{MardkownFile, MarkdownSection, MarkdownSectionLevel};
use crate::structs::FileInfo;
use crate::utils::git::GitCommit;

use crate::utils;
use crate::utils::path::{FilePathType, FolderPathType};
use colored::Colorize;

use std::vec;

use super::metadata_helpers;

const METADATA_CONTENT_TYPE_SECTION: &str = "- type:";
const METADATA_CONTENT_PATH_SECTION: &str = "- path:";
const METADATA_CONTENT_START_LINE_INDEX_SECTION: &str = "- start_line_index:";
const METADATA_CONTENT_END_LINE_INDEX_SECTION: &str = "- end_line_index:";
const STRUCT_TYPES_STRING: &[&str] = &["context_accounts", "account", "input", "other"];

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
        let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
        let metadata_markdown = MardkownFile::new(&metadata_path);
        let struct_section = metadata_markdown.clone().get_section_by_title("Structs");
        let path = metadata_helpers::parse_metadata_info_section(
            &struct_section.content,
            METADATA_CONTENT_PATH_SECTION,
        );
        let struct_type_string = metadata_helpers::parse_metadata_info_section(
            &struct_section.content,
            METADATA_CONTENT_TYPE_SECTION,
        );
        let struct_type_index = STRUCT_TYPES_STRING
            .to_vec()
            .into_iter()
            .position(|struct_type| struct_type == struct_type_string)
            .unwrap();
        let struct_type = StructMetadataType::from_index(struct_type_index);
        let start_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &struct_section.content,
            METADATA_CONTENT_START_LINE_INDEX_SECTION,
        )
        .parse()
        .unwrap();
        let end_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &struct_section.content,
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
}

pub fn update_structs() -> Result<(), String> {
    let metadata_path = utils::path::get_file_path(FilePathType::Metadata, false);
    let mut metadata_markdown = MardkownFile::new(&metadata_path);
    let mut structs_section = metadata_markdown
        .clone()
        .get_section_by_title("Structs")
        .clone();
    // check if empty
    let is_initialized = !structs_section.subsections.is_empty();
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

    let context_account_subsections: Vec<MarkdownSection> = context_accounts_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_structs_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let account_subsections: Vec<MarkdownSection> = accounts_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_structs_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let input_subsections: Vec<MarkdownSection> = input_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_structs_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let other_subsections: Vec<MarkdownSection> = other_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_structs_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();
    structs_section
        .update_subsection_subsections_by_title("Context Accounts", context_account_subsections)
        .unwrap();
    structs_section
        .update_subsection_subsections_by_title("Account", account_subsections)
        .unwrap();
    structs_section
        .update_subsection_subsections_by_title("Input", input_subsections)
        .unwrap();
    structs_section
        .update_subsection_subsections_by_title("Other", other_subsections)
        .unwrap();
    metadata_markdown
        .replace_section(
            metadata_markdown.clone().get_section_by_title("Structs"),
            structs_section,
        )
        .unwrap();
    metadata_markdown.clone().save()?;
    utils::git::create_git_commit(GitCommit::UpdateMetadata, None)?;
    Ok(())
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
    let program_path = utils::path::get_folder_path(FolderPathType::ProgramPath, true);
    let program_folder_files_info = utils::helpers::get::get_only_files_from_folder(program_path)?;
    let mut structs_metadata: Vec<StructMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut struct_metadata_result = get_struct_metadata_from_file_info(file_info)?;
        structs_metadata.append(&mut struct_metadata_result);
    }
    let mut context_accounts_metadata_vec = structs_metadata
        .iter()
        .filter(|metadata| metadata.struct_type == StructMetadataType::ContextAccounts)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut accounts_metadata_vec = structs_metadata
        .iter()
        .filter(|metadata| metadata.struct_type == StructMetadataType::Account)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut input_metadata_vec = structs_metadata
        .iter()
        .filter(|metadata| metadata.struct_type == StructMetadataType::Input)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut other_metadata_vec = structs_metadata
        .iter()
        .filter(|metadata| metadata.struct_type == StructMetadataType::Other)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    context_accounts_metadata_vec
        .sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
    accounts_metadata_vec.sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
    input_metadata_vec.sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
    other_metadata_vec.sort_by(|structs_a, structs_b| structs_a.name.cmp(&structs_b.name));
    Ok((
        context_accounts_metadata_vec,
        accounts_metadata_vec,
        input_metadata_vec,
        other_metadata_vec,
    ))
}

fn get_struct_metadata_from_file_info(
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
            let end_line_index = file_info_content
                .lines()
                .enumerate()
                .find(|(line_index, line)| line.trim() == "}" && line_index > &start_line_index);
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
                    let selection =
                        utils::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
                    let selection_type_enum = StructMetadataType::from_index(selection);
                    let struct_first_line = struct_metadata_content.split("\n").next().unwrap();
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

fn get_structs_section_content(header: &str, struct_metadata: StructMetadata) -> String {
    format!(
        "{header}\n\n{}{}{}{}",
        format!(
            "{} {}\n",
            METADATA_CONTENT_TYPE_SECTION,
            struct_metadata.struct_type.to_string()
        ),
        format!(
            "{} {}\n",
            METADATA_CONTENT_PATH_SECTION, struct_metadata.path
        ),
        format!(
            "{} {}\n",
            METADATA_CONTENT_START_LINE_INDEX_SECTION, struct_metadata.start_line_index
        ),
        format!(
            "{} {}\n",
            METADATA_CONTENT_END_LINE_INDEX_SECTION, struct_metadata.end_line_index
        ),
    )
}
