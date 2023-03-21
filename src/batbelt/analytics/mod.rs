use crate::batbelt::analytics::constraints::{ConstraintInfo, ConstraintsAnalytics};
use crate::batbelt::analytics::entry_points_flow::EntryPointFlowAnalytics;
use crate::batbelt::path::BatFile;
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;

pub mod constraints;
pub mod entry_points_flow;
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
    #[serde(default)]
    pub initialized: bool,
    pub entry_points_flow: Vec<EntryPointFlowAnalytics>,
    pub constraints: ConstraintsAnalytics,
}

impl BatAnalytics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_analytics() -> AnalyticsResult<()> {
        let bat_analytics = BatAnalytics::read_analytics()?;
        if !bat_analytics.initialized {
            ConstraintsAnalytics::init_analytics_data()?;
        } else {
            println!(
                "Analytics already initialized, skipping {}",
                "constraints analytics".bright_green()
            );
        }
        EntryPointFlowAnalytics::init_analytics_data()?;
        let mut bat_analytics = BatAnalytics::read_analytics()?;
        bat_analytics.initialized = true;
        bat_analytics.save_analytics()?;
        bat_analytics.commit_file()?;
        Ok(())
    }

    pub fn update_analytics() -> AnalyticsResult<()> {
        let bat_analytics = BatAnalytics::read_analytics()?;
        if !bat_analytics.initialized {
            return Err(
                Report::new(AnalyticsError).attach_printable(format!("Analytics not initialized"))
            );
        }
        ConstraintsAnalytics::update_analytics_data()?;
        bat_analytics.commit_file()?;
        Ok(())
    }

    pub fn read_analytics() -> AnalyticsResult<Self> {
        let cache_json_bat_file = BatFile::BatAnalyticsFile;
        if !cache_json_bat_file
            .file_exists()
            .change_context(AnalyticsError)?
        {
            let bat_cache = Self::new();
            bat_cache.save_analytics()?;
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

    pub fn save_analytics(&self) -> AnalyticsResult<()> {
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

    pub fn commit_file(&self) -> AnalyticsResult<()> {
        let analytics_bat_file = BatFile::BatAnalyticsFile;
        analytics_bat_file
            .commit_file(None)
            .change_context(AnalyticsError)?;
        Ok(())
    }
}
