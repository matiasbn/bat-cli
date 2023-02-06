use crate::batbelt::structs::FileInfo;

use crate::batbelt;
use crate::batbelt::path::FolderPathType;
use colored::Colorize;

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionLevel};
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use inflector::Inflector;
use std::vec;

use super::MetadataContent;

pub const STRUCT_TYPES_STRING: &[&str] = &["context_accounts", "account", "input", "other"];

#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl StructMetadata {
    pub fn new(
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

    pub fn to_markdown_string(&self) -> String {
        format!(
            "{}\n\n- type: {}\n- path:{}\n- start_line_index:{}\n- end_line_index:{}",
            MarkdownSectionLevel::H2.get_header(&self.name),
            self.struct_type.to_camel_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display)]
pub enum StructMetadataType {
    ContextAccounts,
    SolanaAccount,
    Input,
    Other,
}

impl StructMetadataType {
    pub fn to_camel_case(&self) -> String {
        self.to_string().to_camel_case()
    }

    pub fn get_structs_type_vec() -> Vec<StructMetadataType> {
        vec![
            StructMetadataType::ContextAccounts,
            StructMetadataType::SolanaAccount,
            StructMetadataType::Input,
            StructMetadataType::Other,
        ]
    }
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
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
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
        .filter(|metadata| metadata.struct_type == StructMetadataType::SolanaAccount)
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
    let struct_types_colored = StructMetadataType::get_structs_type_vec()
        .iter()
        .map(|struct_type| struct_type.to_camel_case())
        .enumerate()
        .map(|f| match f.0 {
            0 => f.1.red(),
            1 => f.1.yellow(),
            2 => f.1.green(),
            3 => f.1.blue(),
            _ => f.1.magenta(),
        })
        .collect::<Vec<_>>();
    let bat_sonar = BatSonar::new(&file_info_content, SonarResultType::Struct);
    println!("{}", bat_sonar.content);
    println!("{:#?}", bat_sonar.result);
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
                let is_struct = batbelt::cli_inputs::select_yes_or_no(&prompt_text)?;
                if is_struct {
                    let prompt_text = "Select the type of struct:";
                    let selection = batbelt::cli_inputs::select(
                        prompt_text,
                        struct_types_colored.clone(),
                        None,
                    )?;
                    let selection_type_enum = StructMetadataType::get_structs_type_vec()[selection];
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

pub fn get_struct_name(struct_line: &str) -> String {
    struct_line.split_whitespace().collect::<Vec<_>>()[2]
        .split("<")
        .next()
        .unwrap()
        .replace(":", "")
        .to_string()
        .clone()
}

pub fn get_structs_section_content(header: &str, struct_metadata: StructMetadata) -> String {
    format!(
        "{header}\n{}{}{}",
        format!(
            "{} {}\n",
            MetadataContent::Path.get_prefix(),
            struct_metadata.path
        ),
        format!(
            "{} {}\n",
            MetadataContent::StartLineIndex.get_prefix(),
            struct_metadata.start_line_index
        ),
        format!(
            "{} {}\n",
            MetadataContent::EndLineIndex.get_prefix(),
            struct_metadata.end_line_index
        ),
    )
}
