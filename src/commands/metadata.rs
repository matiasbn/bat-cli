use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;

use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::BatMetadataType;

use crate::batbelt::path::BatFile;
use crate::{batbelt, GitCommit};
use colored::Colorize;

use crate::batbelt::metadata::BatMetadataType::Functions;
use error_stack::{Report, Result, ResultExt};

use super::CommandError;

pub fn functions() -> Result<(), CommandError> {
    let mut functions_metadata_markdown = BatMetadataType::Functions
        .get_markdown()
        .change_context(CommandError)?;
    let is_initialized = BatMetadataType::Functions
        .is_initialized()
        .change_context(CommandError)?;
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
            return Err(Report::new(CommandError).attach_printable(format!(
                "User decided not to continue with the update process for functions metadata"
            )));
        }
    }
    let functions_metadata =
        FunctionMetadata::get_functions_metadata_from_program().change_context(CommandError)?;
    let functions_markdown_content = functions_metadata
        .into_iter()
        .map(|function_metadata| function_metadata.get_markdown_section_content_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    functions_metadata_markdown.content = functions_markdown_content;
    functions_metadata_markdown
        .save()
        .change_context(CommandError)?;
    batbelt::git::create_git_commit(
        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Functions,
        },
        None,
    )
    .unwrap();
    Ok(())
}

pub fn structs() -> Result<(), CommandError> {
    let is_initialized = BatMetadataType::Structs
        .is_initialized()
        .change_context(CommandError)?;
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
            return Err(Report::new(CommandError).attach_printable(format!(
                "User decided not to continue with the update process for structs metadata"
            )));
        }
    }
    let mut structs_metadata_markdown = BatMetadataType::Structs
        .get_markdown()
        .change_context(CommandError)?;
    let structs_metadata =
        StructMetadata::get_structs_metadata_from_program().change_context(CommandError)?;
    let structs_markdown_content = structs_metadata
        .into_iter()
        .map(|struct_metadata| struct_metadata.get_markdown_section_content_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    structs_metadata_markdown.content = structs_markdown_content;
    structs_metadata_markdown
        .save()
        .change_context(CommandError)?;
    batbelt::git::create_git_commit(
        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Structs,
        },
        None,
    )
    .unwrap();
    Ok(())
}
