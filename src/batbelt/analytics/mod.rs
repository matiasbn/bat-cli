use crate::batbelt::analytics::code_overhaul_interactive::CodeOverhaulInteractiveCache;
use crate::batbelt::path::BatFile;
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;

pub mod code_overhaul_interactive;
pub mod state_changes;

#[derive(Debug)]
pub struct AnalyticsError;

impl fmt::Display for AnalyticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cache error")
    }
}

impl Error for AnalyticsError {}

pub type AnalyticsResult<T> = Result<T, AnalyticsError>;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BatAnalytics {
    pub co_interactive: Vec<CodeOverhaulInteractiveCache>,
}

impl BatAnalytics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_cache() -> AnalyticsResult<Self> {
        let cache_json_bat_file = BatFile::BatAnalyticsFile;
        if !cache_json_bat_file
            .file_exists()
            .change_context(AnalyticsError)?
        {
            let bat_cache = Self::new();
            bat_cache.save_metadata()?;
            return Ok(bat_cache);
        }
        let bat_cache_value: Value = serde_json::from_str(
            &cache_json_bat_file
                .read_content(true)
                .change_context(AnalyticsError)?,
        )
        .into_report()
        .change_context(AnalyticsError)?;
        let mut bat_cache: BatAnalytics = serde_json::from_value(bat_cache_value)
            .into_report()
            .change_context(AnalyticsError)?;
        Ok(bat_cache)
    }

    pub fn save_metadata(&self) -> AnalyticsResult<()> {
        let metadata_json_bat_file = BatFile::BatAnalyticsFile;
        let metadata_json = json!(&self);
        let metadata_json_pretty = serde_json::to_string_pretty(&metadata_json)
            .into_report()
            .change_context(AnalyticsError)?;
        metadata_json_bat_file
            .write_content(false, &metadata_json_pretty)
            .change_context(AnalyticsError)?;
        Ok(())
    }

    pub fn commit_cache(&self) -> AnalyticsResult<()> {
        let cache_bat_file = BatFile::BatAnalyticsFile;
        cache_bat_file
            .commit_file(None)
            .change_context(AnalyticsError)?;
        Ok(())
    }
}
