use crate::batbelt::analytics::{AnalyticsError, AnalyticsResult, BatAnalytics};
use crate::batbelt::metadata::{BatMetadata, SourceCodeMetadata};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use error_stack::ResultExt;
use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConstraintAnalytics {
    pub is_constant: bool,
    #[serde(default)]
    pub reviewed: bool,
    pub constraint: String,
    pub entry_points: Vec<String>,
}

impl ConstraintAnalytics {
    pub fn generate_analytics_data() -> AnalyticsResult<()> {
        let bat_metadata = BatMetadata::read_metadata().change_context(AnalyticsError)?;
        let entry_points_metadata = bat_metadata.clone().entry_points;
        let mut constraints_hashmap: HashMap<String, Vec<String>> = HashMap::new();
        let constraint_regex = regex!(r#"constraint[\s]*=[\s]*"#);
        let constraint_error_regex = regex!(r#"@[\s\S]*"#);
        for entry_point in entry_points_metadata {
            let context_accounts = bat_metadata
                .clone()
                .get_context_accounts_metadata_by_struct_source_code_metadata_id(
                    entry_point.context_accounts_id,
                )
                .change_context(AnalyticsError)?;
            for ca_info in context_accounts.context_accounts_info {
                for validation in ca_info.validations {
                    if !constraint_regex.is_match(&validation) {
                        continue;
                    }
                    let validation = constraint_error_regex.replace(&validation, "").to_string();
                    match constraints_hashmap.get_mut(&validation) {
                        None => {
                            constraints_hashmap.insert(validation, vec![entry_point.name.clone()]);
                        }
                        Some(value) => value.push(entry_point.name.clone()),
                    };
                }
            }
        }
        let mut constraints_analytics_vec = vec![];
        for constraint in constraints_hashmap.keys() {
            constraints_analytics_vec.push(ConstraintAnalytics {
                is_constant: false,
                reviewed: false,
                constraint: constraint.clone(),
                entry_points: constraints_hashmap.get(constraint).unwrap().clone(),
            })
        }
        let mut bat_analytics = BatAnalytics::read_analytics().change_context(AnalyticsError)?;
        bat_analytics.constraints = constraints_analytics_vec;
        bat_analytics.save_analytics()?;
        bat_analytics.commit_file()?;
        Ok(())
    }
}
