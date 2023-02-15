use crate::batbelt::entrypoint::Entrypoint;
use crate::batbelt::structs::FileInfo;
use std::fmt::Debug;

use crate::batbelt;
use crate::batbelt::path::{FilePathType, FolderPathType};
use colored::{ColoredString, Colorize};

use crate::batbelt::markdown::{
    MarkdownFile, MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel,
};
use crate::batbelt::metadata::source_code::SourceCodeMetadata;
use crate::batbelt::metadata::{get_metadata_markdown, MetadataSection};
use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use inflector::Inflector;
use std::vec;
use strum::IntoEnumIterator;

#[derive(Debug, Clone)]
pub struct StructMetadata {
    pub path: String,
    pub name: String,
    pub struct_type: StructMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
enum StructMetadataInfoSection {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
}

impl StructMetadataInfoSection {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }
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

    pub fn get_markdown_section_content_string(&self) -> String {
        format!(
            "- type: {}\n- path: {}\n- start_line_index: {}\n- end_line_index: {}",
            self.struct_type.to_snake_case(),
            self.path,
            self.start_line_index,
            self.end_line_index
        )
    }

    pub fn structs_metadata_is_initialized() -> bool {
        let mut metadata_markdown = get_metadata_markdown();
        let structs_section = metadata_markdown
            .get_section(&MetadataSection::Structs.to_string())
            .unwrap();
        // // check if empty
        let structs_subsections =
            metadata_markdown.get_section_subsections(structs_section.clone());
        let is_initialized = !structs_section.content.is_empty() || structs_subsections.len() > 0;
        is_initialized
    }

    pub fn get_structs_metadata_by_type(struct_type: StructMetadataType) -> Vec<StructMetadata> {
        let structs_metadata = Self::get_structs_metadata_from_metadata_file();
        structs_metadata
            .into_iter()
            .filter(|struct_metadata| struct_metadata.struct_type == struct_type)
            .collect::<Vec<_>>()
    }

    pub fn get_structs_metadata_names_by_type(struct_type: StructMetadataType) -> Vec<String> {
        let structs_metadata = Self::get_structs_metadata_from_metadata_file();
        structs_metadata
            .into_iter()
            .filter_map(|struct_metadata| {
                if struct_metadata.struct_type == struct_type {
                    Some(struct_metadata.name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }

    pub fn get_structs_markdown_subsections_from_metadata_file() -> Vec<MarkdownSection> {
        let metadata_markdown = get_metadata_markdown();
        let structs_section = metadata_markdown
            .get_section(&MetadataSection::Structs.to_sentence_case())
            .unwrap();
        let structs_subsections = metadata_markdown.get_section_subsections(structs_section);
        structs_subsections
    }

    pub fn get_structs_metadata_from_metadata_file() -> Vec<StructMetadata> {
        let structs_subsections = Self::get_structs_markdown_subsections_from_metadata_file();
        let structs_sourcecodes = structs_subsections
            .into_iter()
            .map(|subsection| StructMetadata::from_markdown_section(subsection))
            .collect::<Vec<StructMetadata>>();
        structs_sourcecodes
    }

    pub fn get_structs_sourcecodes() -> Vec<SourceCodeMetadata> {
        let structs_subsections = Self::get_structs_markdown_subsections_from_metadata_file();
        let structs_sourcecodes = structs_subsections
            .into_iter()
            .map(|subsection| StructMetadata::from_markdown_section(subsection))
            .map(|struct_metadata| {
                SourceCodeMetadata::new(
                    struct_metadata.name,
                    struct_metadata.path,
                    struct_metadata.start_line_index,
                    struct_metadata.end_line_index,
                )
            })
            .collect::<Vec<SourceCodeMetadata>>();
        structs_sourcecodes
    }

    pub fn to_markdown_section(&self, section_hash: &str) -> MarkdownSection {
        let section_level_header = MarkdownSectionLevel::H2.get_header(&self.name);
        let section_header = MarkdownSectionHeader::new_from_header_and_hash(
            section_level_header,
            section_hash.to_string(),
            0,
        );
        let md_section = MarkdownSection::new(
            section_header,
            self.get_markdown_section_content_string(),
            0,
            0,
        );
        md_section
    }

    pub fn to_source_code_metadata(&self) -> SourceCodeMetadata {
        SourceCodeMetadata::new(
            self.name.clone(),
            self.path.clone(),
            self.start_line_index,
            self.end_line_index,
        )
    }

    pub fn from_markdown_section(md_section: MarkdownSection) -> Self {
        let name = md_section.section_header.title;
        let path =
            Self::parse_metadata_info_section(&md_section.content, StructMetadataInfoSection::Path);
        let struct_type_string =
            Self::parse_metadata_info_section(&md_section.content, StructMetadataInfoSection::Type);
        let start_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            StructMetadataInfoSection::StartLineIndex,
        );
        let end_line_index = Self::parse_metadata_info_section(
            &md_section.content,
            StructMetadataInfoSection::EndLineIndex,
        );
        StructMetadata::new(
            path,
            name,
            StructMetadataType::from_str(&struct_type_string),
            start_line_index.parse::<usize>().unwrap(),
            end_line_index.parse::<usize>().unwrap(),
        )
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        struct_section: StructMetadataInfoSection,
    ) -> String {
        let section_prefix = struct_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .unwrap()
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        data
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum StructMetadataType {
    ContextAccounts,
    SolanaAccount,
    Event,
    Input,
    Other,
}

impl StructMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn from_str(type_str: &str) -> StructMetadataType {
        let structs_type_vec = Self::get_structs_type_vec();
        let struct_type = structs_type_vec
            .iter()
            .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
            .unwrap();
        struct_type.clone()
    }

    pub fn get_structs_type_vec() -> Vec<StructMetadataType> {
        StructMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn get_colorized_structs_type_vec() -> Vec<ColoredString> {
        let struct_type_vec = Self::get_structs_type_vec();
        let structs_type_colorized = struct_type_vec
            .iter()
            .map(|struct_type| match struct_type {
                StructMetadataType::ContextAccounts => struct_type.to_sentence_case().red(),
                StructMetadataType::SolanaAccount => struct_type.to_sentence_case().yellow(),
                StructMetadataType::Event => struct_type.to_sentence_case().green(),
                StructMetadataType::Input => struct_type.to_sentence_case().blue(),
                StructMetadataType::Other => struct_type.to_sentence_case().magenta(),
                _ => unimplemented!("color no implemented for given type"),
            })
            .collect::<Vec<_>>();
        structs_type_colorized
    }
}

pub fn get_structs_metadata_from_program() -> Result<Vec<StructMetadata>, String> {
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
    let mut structs_metadata: Vec<StructMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut struct_metadata_result = get_struct_metadata_from_file_info(file_info)?;
        structs_metadata.append(&mut struct_metadata_result);
    }
    structs_metadata.sort_by(|struct_a, struct_b| struct_a.name.cmp(&struct_b.name));
    Ok(structs_metadata)
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
    let struct_types_colored = StructMetadataType::get_colorized_structs_type_vec();
    let bat_sonar = BatSonar::new_scanned(&file_info_content, SonarResultType::Struct);
    for result in bat_sonar.results {
        println!(
            "Struct found at {}\n{}",
            format!(
                "{}:{}",
                struct_file_info.path.clone(),
                result.start_line_index + 1,
            )
            .magenta(),
            result.content.clone().green()
        );
        if assert_struct_is_context_accounts(&file_info_content, result.clone()) {
            println!("{}", "Struct found is ContextAccounts type!".yellow());
            let struct_type = StructMetadataType::ContextAccounts;
            let struct_metadata = StructMetadata::new(
                struct_file_info.path.clone(),
                result.name.to_string(),
                struct_type,
                result.start_line_index + 1,
                result.end_line_index + 1,
            );
            struct_metadata_vec.push(struct_metadata);
            continue;
        }
        let prompt_text = "Select the struct type:";
        let selection =
            batbelt::cli_inputs::select(prompt_text, struct_types_colored.clone(), None)?;
        let struct_type = StructMetadataType::get_structs_type_vec()[selection];
        let struct_metadata = StructMetadata::new(
            struct_file_info.path.clone(),
            result.name.to_string(),
            struct_type,
            result.start_line_index + 1,
            result.end_line_index + 1,
        );
        struct_metadata_vec.push(struct_metadata);
    }
    println!(
        "finishing the review of the {} file",
        struct_file_info.path.clone().blue()
    );
    Ok(struct_metadata_vec)
}

fn assert_struct_is_context_accounts(file_info_content: &str, sonar_result: SonarResult) -> bool {
    if sonar_result.start_line_index > 0 {
        let previous_line =
            file_info_content.lines().collect::<Vec<_>>()[sonar_result.start_line_index - 1];
        let filtered_previous_line = previous_line
            .trim()
            .trim_end_matches(")]")
            .trim_start_matches("#[derive(");
        let mut tokenized = filtered_previous_line.split(", ");
        if tokenized.any(|token| token == "Acccounts") {
            return true;
        }
    }
    let context_accounts_content = vec![
        "Signer<",
        "AccountLoader<",
        "UncheckedAccount<",
        "#[account(",
    ];
    if context_accounts_content
        .iter()
        .any(|content| sonar_result.content.contains(content))
    {
        return true;
    }
    let lib_file_path = batbelt::path::get_file_path(FilePathType::ProgramLib, false);
    let entrypoints = BatSonar::new_from_path(
        &lib_file_path,
        Some("#[program]"),
        SonarResultType::Function,
    );
    let mut entrypoints_context_accounts_names = entrypoints
        .results
        .iter()
        .map(|result| Entrypoint::get_context_name(&result.name).unwrap());
    if entrypoints_context_accounts_names.any(|name| name == sonar_result.name) {
        return true;
    }
    return false;
}
