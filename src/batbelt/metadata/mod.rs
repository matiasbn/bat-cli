pub mod entrypoint_metadata;
pub mod functions_metadata;
pub mod miro_metadata;
pub mod source_code_metadata;
pub mod structs_metadata;

use colored::{ColoredString, Colorize};
use std::error::Error;
use std::fmt;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSection};

use crate::batbelt::path::BatFile;

use inflector::Inflector;

use error_stack::{Report, Result, ResultExt};
use strum::IntoEnumIterator;

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Metadata error")
    }
}

impl Error for MetadataError {}

pub struct BatMetadata;

impl BatMetadata {
    pub fn metadata_is_initialized() -> Result<bool, MetadataError> {
        let mut metadata_initialized = true;
        for metadata_type in BatMetadataType::get_metadata_type_vec() {
            let section_initialized = metadata_type.is_initialized()?;
            if !section_initialized {
                metadata_initialized = false;
            }
        }
        Ok(metadata_initialized)
    }

    pub fn check_metadata_is_initialized() -> Result<(), MetadataError> {
        let metadata_types = BatMetadataType::get_metadata_type_vec();
        for metadata_type in metadata_types {
            metadata_type.check_is_initialized()?;
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatMetadataType {
    Structs,
    Functions,
    Entrypoints,
}

impl BatMetadataType {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    pub fn get_metadata_type_vec() -> Vec<BatMetadataType> {
        BatMetadataType::iter().collect::<Vec<_>>()
    }

    pub fn get_colorized_metadata_type_vec() -> Vec<ColoredString> {
        let struct_type_vec = Self::get_metadata_type_vec();
        let structs_type_colorized = struct_type_vec
            .iter()
            .map(|metadata_type| match metadata_type {
                Self::Structs => metadata_type.to_sentence_case().red(),
                Self::Functions => metadata_type.to_sentence_case().yellow(),
                Self::Entrypoints => metadata_type.to_sentence_case().magenta(),
            })
            .collect::<Vec<_>>();
        structs_type_colorized
    }

    pub fn get_path(&self) -> Result<String, MetadataError> {
        let path = match self {
            BatMetadataType::Structs => BatFile::StructsMetadata
                .get_path(true)
                .change_context(MetadataError)?,
            BatMetadataType::Functions => BatFile::FunctionsMetadata
                .get_path(true)
                .change_context(MetadataError)?,
            BatMetadataType::Entrypoints => BatFile::FunctionsMetadata
                .get_path(true)
                .change_context(MetadataError)?,
        };
        Ok(path)
    }

    pub fn get_markdown(&self) -> Result<MarkdownFile, MetadataError> {
        let file_path = self.get_path()?;
        let markdown_file = MarkdownFile::new(&file_path);
        Ok(markdown_file)
    }

    pub fn get_markdown_sections_from_metadata_file(
        &self,
    ) -> Result<Vec<MarkdownSection>, MetadataError> {
        let markdown_file = self.get_markdown()?;
        Ok(markdown_file.sections)
    }

    pub fn is_initialized(&self) -> Result<bool, MetadataError> {
        let markdown = self.get_markdown()?;
        Ok(!markdown.sections.is_empty())
    }

    pub fn check_is_initialized(&self) -> Result<(), MetadataError> {
        if !self.is_initialized()? {
            return Err(Report::new(MetadataError).attach_printable(format!(
                "{} metadata is required to be initialized to execute this action",
                self.to_string().red()
            )));
        }
        Ok(())
    }
}
