use crate::markdown::{MarkdownFile, MarkdownSection, MarkdownSectionLevel};
use crate::structs::FileInfo;
use crate::utils::git::GitCommit;

use crate::utils;
use crate::utils::path::{FilePathType, FolderPathType};
use colored::Colorize;

use std::vec;

use super::metadata_helpers;

const FUNCTION_TYPE_SECTION: &str = "- type:";
const FUNCTION_PATH_SECTION: &str = "- path:";
const FUNCTION_START_LINE_INDEX_SECTION: &str = "- start_line_index:";
const FUNCTION_END_LINE_INDEX_SECTION: &str = "- end_line_index:";
const FUNCTIONS_SECTION_TITLE: &str = "Functions";
const HANDLERS_SUBSECTION_TITLE: &str = "Handlers";
const ENTRYPOINTS_SUBSECTION_TITLE: &str = "Entrypoints";
const HELPERS_SUBSECTION_TITLE: &str = "Helpers";
const VALIDATORS_SUBSECTION_TITLE: &str = "Validators";
const OTHERS_SUBSECTION_TITLE: &str = "Other";
pub const FUNCTION_TYPES_STRING: &[&str] =
    &["handler", "entry_point", "helper", "validator", "other"];

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub path: String,
    pub name: String,
    pub function_type: FunctionMetadataType,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl FunctionMetadata {
    fn new(
        path: String,
        name: String,
        function_type: FunctionMetadataType,
        start_line_index: usize,
        end_line_index: usize,
    ) -> Self {
        FunctionMetadata {
            path,
            name,
            function_type,
            start_line_index,
            end_line_index,
        }
    }

    fn new_from_metadata_name(function_name: &str) -> Self {
        let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
        let metadata_markdown = MarkdownFile::new(&metadata_path);
        let function_section = metadata_markdown
            .clone()
            .get_section_by_title(FUNCTIONS_SECTION_TITLE);
        let path = metadata_helpers::parse_metadata_info_section(
            &function_section.content,
            FUNCTION_PATH_SECTION,
        );
        let function_type_string = metadata_helpers::parse_metadata_info_section(
            &function_section.content,
            FUNCTION_TYPE_SECTION,
        );
        let function_type_index = FUNCTION_TYPES_STRING
            .to_vec()
            .into_iter()
            .position(|function_type| function_type == function_type_string)
            .unwrap();
        let function_type = FunctionMetadataType::from_index(function_type_index);
        let start_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &function_section.content,
            FUNCTION_START_LINE_INDEX_SECTION,
        )
        .parse()
        .unwrap();
        let end_line_index: usize = metadata_helpers::parse_metadata_info_section(
            &function_section.content,
            FUNCTION_END_LINE_INDEX_SECTION,
        )
        .parse()
        .unwrap();
        FunctionMetadata::new(
            path,
            function_name.to_string(),
            function_type,
            start_line_index,
            end_line_index,
        )
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FunctionMetadataType {
    Handler,
    EntryPoint,
    Helper,
    Validator,
    Other,
}

impl FunctionMetadataType {
    fn get_function_metadata_index(&self) -> usize {
        match self {
            FunctionMetadataType::Handler => 0,
            FunctionMetadataType::EntryPoint => 1,
            FunctionMetadataType::Helper => 2,
            FunctionMetadataType::Validator => 3,
            FunctionMetadataType::Other => 4,
        }
    }

    fn to_string(&self) -> &str {
        let index = self.get_function_metadata_index();
        FUNCTION_TYPES_STRING[index]
    }

    fn from_index(index: usize) -> FunctionMetadataType {
        match index {
            0 => FunctionMetadataType::Handler,
            1 => FunctionMetadataType::EntryPoint,
            2 => FunctionMetadataType::Helper,
            3 => FunctionMetadataType::Validator,
            4 => FunctionMetadataType::Other,
            _ => todo!(),
        }
    }
    fn get_function_types<'a>() -> Vec<&'a str> {
        let mut result_vec = vec![""; FUNCTION_TYPES_STRING.len()];
        result_vec.copy_from_slice(FUNCTION_TYPES_STRING);
        result_vec
    }
}

pub fn update_functions() -> Result<(), String> {
    let metadata_path = utils::path::get_file_path(FilePathType::Metadata, false);
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let mut functions_section = metadata_markdown
        .clone()
        .get_section_by_title(FUNCTIONS_SECTION_TITLE)
        .clone();
    // check if empty
    let is_initialized = !functions_section
        .subsections
        .iter()
        .all(|subsection| subsection.subsections.is_empty());
    // prompt the user if he wants to replace
    if is_initialized {
        let user_decided_to_continue = utils::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("functions in metadata.md are arealready initialized").bright_red()
            )
            .as_str(),
        )?;
        if !user_decided_to_continue {
            panic!("User decided not to continue with the update process for functions metada")
        }
    }
    // get functions in all files
    let (
        handlers_metadata_vec,
        entry_poins_metadata_vec,
        helpers_metadata_vec,
        validators_metadata_vec,
        other_metadata_vec,
    ) = get_functions_metadata_from_program()?;

    let handlers_subsections: Vec<MarkdownSection> = handlers_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_functions_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let entry_points_subsections: Vec<MarkdownSection> = entry_poins_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_functions_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let helpers_subsections: Vec<MarkdownSection> = helpers_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_functions_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();
    let validators_subsections: Vec<MarkdownSection> = validators_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_functions_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();

    let other_subsections: Vec<MarkdownSection> = other_metadata_vec
        .into_iter()
        .map(|metadata| {
            MarkdownSection::new_from_content(&get_functions_section_content(
                &MarkdownSectionLevel::H3.get_header(&metadata.name),
                metadata,
            ))
        })
        .collect();
    functions_section
        .update_subsection_subsections_by_title(HANDLERS_SUBSECTION_TITLE, handlers_subsections)
        .unwrap();
    functions_section
        .update_subsection_subsections_by_title(
            ENTRYPOINTS_SUBSECTION_TITLE,
            entry_points_subsections,
        )
        .unwrap();
    functions_section
        .update_subsection_subsections_by_title(HELPERS_SUBSECTION_TITLE, helpers_subsections)
        .unwrap();
    functions_section
        .update_subsection_subsections_by_title(VALIDATORS_SUBSECTION_TITLE, validators_subsections)
        .unwrap();
    functions_section
        .update_subsection_subsections_by_title(OTHERS_SUBSECTION_TITLE, other_subsections)
        .unwrap();
    metadata_markdown
        .replace_section(
            metadata_markdown
                .clone()
                .get_section_by_title(FUNCTIONS_SECTION_TITLE),
            functions_section,
        )
        .unwrap();
    metadata_markdown.clone().save()?;
    utils::git::create_git_commit(GitCommit::UpdateMetadata, None)?;
    Ok(())
}

pub fn get_functions_metadata_from_program() -> Result<
    (
        Vec<FunctionMetadata>,
        Vec<FunctionMetadata>,
        Vec<FunctionMetadata>,
        Vec<FunctionMetadata>,
        Vec<FunctionMetadata>,
    ),
    String,
> {
    let program_path = utils::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info = utils::helpers::get::get_only_files_from_folder(program_path)?;
    let mut functions_metadata: Vec<FunctionMetadata> = vec![];
    for file_info in program_folder_files_info {
        let mut function_metadata_result = get_function_metadata_from_file_info(file_info)?;
        functions_metadata.append(&mut function_metadata_result);
    }
    let mut handlers_metadata_vec = functions_metadata
        .iter()
        .filter(|metadata| metadata.function_type == FunctionMetadataType::Handler)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut entry_points_metadata_vec = functions_metadata
        .iter()
        .filter(|metadata| metadata.function_type == FunctionMetadataType::EntryPoint)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut helpers_metadata_vec = functions_metadata
        .iter()
        .filter(|metadata| metadata.function_type == FunctionMetadataType::Helper)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut validators_metadata_vec = functions_metadata
        .iter()
        .filter(|metadata| metadata.function_type == FunctionMetadataType::Validator)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    let mut others_metadata_vec = functions_metadata
        .iter()
        .filter(|metadata| metadata.function_type == FunctionMetadataType::Other)
        .map(|f| f.clone())
        .collect::<Vec<_>>();
    handlers_metadata_vec
        .sort_by(|functions_a, functions_b| functions_a.name.cmp(&functions_b.name));
    entry_points_metadata_vec
        .sort_by(|functions_a, functions_b| functions_a.name.cmp(&functions_b.name));
    helpers_metadata_vec
        .sort_by(|functions_a, functions_b| functions_a.name.cmp(&functions_b.name));
    validators_metadata_vec
        .sort_by(|functions_a, functions_b| functions_a.name.cmp(&functions_b.name));
    others_metadata_vec.sort_by(|functions_a, functions_b| functions_a.name.cmp(&functions_b.name));
    Ok((
        handlers_metadata_vec,
        entry_points_metadata_vec,
        helpers_metadata_vec,
        validators_metadata_vec,
        others_metadata_vec,
    ))
}

fn get_function_metadata_from_file_info(
    function_file_info: FileInfo,
) -> Result<Vec<FunctionMetadata>, String> {
    let mut function_metadata_vec: Vec<FunctionMetadata> = vec![];
    println!(
        "starting the review of the {} file",
        function_file_info.path.clone().blue()
    );
    let file_info_content = function_file_info.read_content().unwrap();
    let file_info_content_lines = file_info_content.lines();
    let function_types_colored = FunctionMetadataType::get_function_types()
        .iter()
        .enumerate()
        .map(|f| match f.0 {
            0 => f.1.red(),
            1 => f.1.yellow(),
            2 => f.1.green(),
            3 => f.1.purple(),
            4 => f.1.blue(),
            _ => todo!(),
        })
        .collect::<Vec<_>>();
    for (content_line_index, content_line) in file_info_content_lines.enumerate() {
        if content_line.contains("pub") && content_line.contains("fn") && content_line.contains("(")
        {
            let start_line_index = content_line_index;
            let trailing_whitespaces: usize = content_line
                .chars()
                .take_while(|ch| ch.is_whitespace() && *ch != '\n')
                .map(|ch| ch.len_utf8())
                .sum();
            let end_of_function = format!("{}}}", " ".repeat(trailing_whitespaces));
            let end_line_index =
                file_info_content
                    .lines()
                    .enumerate()
                    .find(|(line_index, line)| {
                        line.to_string() == end_of_function && line_index > &start_line_index
                    });
            if let Some((closing_brace_index, _)) = end_line_index {
                let end_line_index = closing_brace_index;
                let function_metadata_content = file_info_content.lines().collect::<Vec<_>>()
                    [start_line_index..=end_line_index]
                    .to_vec()
                    .join("\n");
                println!(
                    "possible function found at {}",
                    format!(
                        "{}:{}",
                        function_file_info.path.clone(),
                        content_line_index + 1
                    )
                    .magenta()
                );
                let prompt_text = format!(
                    "Are these lines a {}?:\n{}",
                    "function".red(),
                    function_metadata_content.green()
                );
                let is_function = utils::cli_inputs::select_yes_or_no(&prompt_text)?;
                if is_function {
                    let prompt_text = "Select the type of function:";
                    let selection = utils::cli_inputs::select(
                        prompt_text,
                        function_types_colored.clone(),
                        None,
                    )?;
                    let selection_type_enum = FunctionMetadataType::from_index(selection);
                    let function_first_line = function_metadata_content.split("\n").next().unwrap();
                    let function_name = get_function_name(function_first_line);
                    let function_metadata = FunctionMetadata::new(
                        function_file_info.path.clone(),
                        function_name.to_string(),
                        selection_type_enum,
                        start_line_index + 1,
                        end_line_index + 1,
                    );
                    function_metadata_vec.push(function_metadata);
                }
            };
        }
    }
    println!(
        "finishing the review of the {} file",
        function_file_info.path.clone().blue()
    );
    Ok(function_metadata_vec)
}

fn get_function_name(function_line: &str) -> String {
    function_line.trim().split("fn ").collect::<Vec<_>>()[1]
        .split("(")
        .next()
        .unwrap()
        .split("<")
        .next()
        .unwrap()
        .to_string()
        .clone()
}

fn get_functions_section_content(header: &str, function_metadata: FunctionMetadata) -> String {
    format!(
        "{header}\n\n{}{}{}{}",
        format!(
            "{} {}\n",
            FUNCTION_TYPE_SECTION,
            function_metadata.function_type.to_string()
        ),
        format!("{} {}\n", FUNCTION_PATH_SECTION, function_metadata.path),
        format!(
            "{} {}\n",
            FUNCTION_START_LINE_INDEX_SECTION, function_metadata.start_line_index
        ),
        format!(
            "{} {}\n",
            FUNCTION_END_LINE_INDEX_SECTION, function_metadata.end_line_index
        ),
    )
}
