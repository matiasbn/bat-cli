pub mod entrypoint;
pub mod functions;
pub mod miro;
pub mod source_code;
pub mod structs;

use crate::batbelt::{self};

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions::FunctionMetadata;
use crate::batbelt::metadata::structs::StructMetadata;
use crate::batbelt::path::FilePathType;
use colored::Colorize;
use inflector::Inflector;

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

pub fn get_metadata_markdown() -> MarkdownFile {
    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, false);
    MarkdownFile::new(&metadata_path)
}

pub fn metadata_is_initialized() -> bool {
    StructMetadata::structs_metadata_is_initialized()
        && FunctionMetadata::functions_metadata_is_initialized()
}

pub mod metadata_helpers {
    #[allow(unused_imports)]
    use super::*;

    pub fn prompt_user_update_section(section_name: &str) -> Result<(), String> {
        let user_decided_to_continue = batbelt::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("{} in metadata.md is already initialized", section_name).bright_red()
            )
            .as_str(),
        )?;
        if !user_decided_to_continue {
            panic!(
                "User decided not to continue with the update process for {} metada",
                section_name.red()
            )
        }
        Ok(())
    }
}
