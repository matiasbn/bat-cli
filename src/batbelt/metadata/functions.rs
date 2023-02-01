use crate::batbelt::git::GitCommit;
use crate::batbelt::markdown::{MarkdownFile, MarkdownSection, MarkdownSectionLevel};
use crate::batbelt::structs::FileInfo;

use crate::batbelt;
use crate::batbelt::path::{FilePathType, FolderPathType};
use colored::Colorize;

use std::vec;

use super::MetadataContent;

pub const FUNCTIONS_SECTION_TITLE: &str = "Functions";
pub const HANDLERS_SUBSECTION_TITLE: &str = "Handlers";
pub const ENTRYPOINTS_SUBSECTION_TITLE: &str = "Entrypoints";
pub const HELPERS_SUBSECTION_TITLE: &str = "Helpers";
pub const VALIDATORS_SUBSECTION_TITLE: &str = "Validators";
pub const OTHERS_SUBSECTION_TITLE: &str = "Other";
pub const FUNCTION_TYPES_STRING: &[&str] =
    &["handler", "entry_point", "helper", "validator", "other"];

#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub path: String,
    pub name: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
    pub function_type: FunctionMetadataType,
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
    let program_path = batbelt::path::get_folder_path(FolderPathType::ProgramPath, false);
    let program_folder_files_info =
        batbelt::helpers::get::get_only_files_from_folder(program_path)?;
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

pub fn get_function_metadata_from_file_info(
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
                let is_function = batbelt::cli_inputs::select_yes_or_no(&prompt_text)?;
                if is_function {
                    let prompt_text = "Select the type of function:";
                    let selection = batbelt::cli_inputs::select(
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

pub fn get_function_name(function_line: &str) -> String {
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

pub fn get_functions_section_content(header: &str, function_metadata: FunctionMetadata) -> String {
    format!(
        "{header}\n\n{}{}{}",
        format!(
            "{} {}\n",
            MetadataContent::Path.get_prefix(),
            function_metadata.path
        ),
        format!(
            "{} {}\n",
            MetadataContent::StartLineIndex.get_prefix(),
            function_metadata.start_line_index
        ),
        format!(
            "{} {}\n",
            MetadataContent::EndLineIndex.get_prefix(),
            function_metadata.end_line_index
        ),
    )
}
