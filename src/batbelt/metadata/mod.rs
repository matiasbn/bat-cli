pub mod entrypoint;
pub mod functions;
pub mod miro;
pub mod source_code;
pub mod structs;

use std::error::Error;
use std::fmt;

use crate::batbelt::{self};

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions::FunctionMetadata;
use crate::batbelt::metadata::structs::StructMetadata;
use crate::batbelt::path::FilePathType;
use colored::Colorize;
use inflector::Inflector;

use error_stack::{Result, ResultExt};

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sonar error")
    }
}

impl Error for MetadataError {}

#[derive(strum_macros::Display)]
pub enum MetadataSection {
    Structs,
    Functions,
    Entrypoints,
    Miro,
}

impl MetadataSection {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }
}

pub fn get_metadata_markdown() -> Result<MarkdownFile, MetadataError> {
    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, false)
        .change_context(MetadataError)?;
    Ok(MarkdownFile::new(&metadata_path))
}

pub fn metadata_is_initialized() -> Result<bool, MetadataError> {
    Ok(StructMetadata::structs_metadata_is_initialized()?
        && FunctionMetadata::functions_metadata_is_initialized()?)
}

pub mod metadata_helpers {
    #[allow(unused_imports)]
    use super::*;

    pub fn prompt_user_update_section(section_name: &str) -> Result<(), MetadataError> {
        let user_decided_to_continue = batbelt::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("{} in metadata.md is already initialized", section_name).bright_red()
            )
            .as_str(),
        )
        .change_context(MetadataError)?;
        if !user_decided_to_continue {
            panic!(
                "User decided not to continue with the update process for {} metada",
                section_name.red()
            )
        }
        Ok(())
    }
}
