//! Workspace scaffolding.
//!
//! bat-cli produces Miro diagrams, so the workspace it needs is small: a config
//! file, an empty metadata cache and a place to drop the rendered screenshots.
//! The audit-report scaffolding this module used to generate — code-overhaul
//! files, finding folders, auditor notes, a README, a generated `package.json` —
//! belonged to a workflow bat-cli no longer owns.

use std::error::Error;
use std::{fmt, fs};

use error_stack::{IntoReport, Result, ResultExt};

use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::path::BatFile;

#[derive(Debug)]
pub struct TemplateError;

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Template error")
    }
}

impl Error for TemplateError {}

pub type TemplateResult<T> = Result<T, TemplateError>;

pub struct TemplateGenerator;

impl TemplateGenerator {
    /// Write an empty `BatMetadata.json`, which `sonar` then fills in.
    pub fn create_metadata_json(&self) -> TemplateResult<()> {
        BatFile::BatMetadataFile
            .create_empty(false)
            .change_context(TemplateError)?;
        BatMetadata::new_empty()
            .save_metadata()
            .change_context(TemplateError)?;
        Ok(())
    }

}
