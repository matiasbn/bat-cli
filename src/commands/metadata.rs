use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions::{get_functions_metadata_from_program, FunctionMetadata};

use crate::batbelt::metadata::structs::{get_structs_metadata_from_program, StructMetadata};
use crate::batbelt::metadata::MetadataSection;

use crate::batbelt::path::FilePathType;
use crate::{batbelt, GitCommit};
use colored::Colorize;

use error_stack::{Result, ResultExt};

use super::CommandError;

pub fn functions() -> Result<(), CommandError> {
    let metadata_path =
        batbelt::path::get_file_path(FilePathType::Metadata, false).change_context(CommandError)?;
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let functions_section = metadata_markdown
        .get_section(&MetadataSection::Functions.to_string())
        .unwrap();
    let is_initialized =
        FunctionMetadata::functions_metadata_is_initialized().change_context(CommandError)?;
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

pub fn structs() -> Result<(), CommandError> {
    let metadata_path =
        batbelt::path::get_file_path(FilePathType::Metadata, false).change_context(CommandError)?;
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let structs_section = metadata_markdown
        .get_section(&MetadataSection::Structs.to_string())
        .unwrap();
    // // check if empty
    let is_initialized =
        StructMetadata::structs_metadata_is_initialized().change_context(CommandError)?;
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
                .to_markdown_section(&structs_section.section_header.section_hash.clone())
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
