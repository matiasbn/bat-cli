pub mod entrypoint_metadata;
pub mod functions_metadata;
pub mod miro_metadata;
pub mod source_code_metadata;
pub mod structs_metadata;

use std::error::Error;
use std::fmt;

use crate::batbelt::{self};

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::path::BatFile;

use inflector::Inflector;

use error_stack::{Report, Result, ResultExt};

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sonar error")
    }
}

impl Error for MetadataError {}

pub struct BatMetadata;

impl BatMetadata {
    pub fn metadata_is_initialized() -> Result<bool, MetadataError> {
        Ok(StructMetadata::structs_metadata_is_initialized()?
            && FunctionMetadata::functions_metadata_is_initialized()?)
    }

    pub fn check_metadata_is_initialized() -> Result<(), MetadataError> {
        StructMetadata::check_structs_metadata_is_initialized()?;
        FunctionMetadata::check_functions_metadata_is_initialized()?;
        Ok(())
    }

    pub fn get_metadata_markdown() -> Result<MarkdownFile, MetadataError> {
        let metadata_path =
            batbelt::path::get_file_path(BatFile::Metadata, false).change_context(MetadataError)?;
        Ok(MarkdownFile::new(&metadata_path))
    }
}

#[derive(strum_macros::Display)]
pub enum BatMetadataSection {
    Structs,
    Functions,
    Entrypoints,
    Miro,
}

impl BatMetadataSection {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }
}
