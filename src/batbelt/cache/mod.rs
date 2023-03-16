use crate::batbelt::cache::code_overhaul_interactive_cache::CodeOverhaulInteractiveCache;
use crate::batbelt::path::BatFile;
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;

pub mod code_overhaul_interactive_cache;
pub mod state_changes_cache;

#[derive(Debug)]
pub struct CacheError;

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cache error")
    }
}

impl Error for CacheError {}

pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct BatCache {
    pub co_interactive: Vec<CodeOverhaulInteractiveCache>,
}

impl BatCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn read_cache() -> CacheResult<Self> {
        let cache_json_bat_file = BatFile::BatCacheFile;
        if !cache_json_bat_file
            .file_exists()
            .change_context(CacheError)?
        {
            let bat_cache = Self::new();
            bat_cache.save_metadata()?;
            return Ok(bat_cache);
        }
        let bat_cache_value: Value = serde_json::from_str(
            &cache_json_bat_file
                .read_content(true)
                .change_context(CacheError)?,
        )
        .into_report()
        .change_context(CacheError)?;
        let mut bat_cache: BatCache = serde_json::from_value(bat_cache_value)
            .into_report()
            .change_context(CacheError)?;
        Ok(bat_cache)
    }

    pub fn save_metadata(&self) -> CacheResult<()> {
        let metadata_json_bat_file = BatFile::BatCacheFile;
        let metadata_json = json!(&self);
        let metadata_json_pretty = serde_json::to_string_pretty(&metadata_json)
            .into_report()
            .change_context(CacheError)?;
        metadata_json_bat_file
            .write_content(false, &metadata_json_pretty)
            .change_context(CacheError)?;
        Ok(())
    }

    pub fn commit_cache(&self) -> CacheResult<()> {
        let cache_bat_file = BatFile::BatCacheFile;
        cache_bat_file
            .commit_file(None)
            .change_context(CacheError)?;
        Ok(())
    }
}
