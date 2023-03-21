use crate::batbelt::analytics::{AnalyticsError, AnalyticsResult, BatAnalytics};
use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::metadata::{BatMetadata, SourceCodeMetadata};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use colored::Colorize;
use error_stack::{Report, ResultExt};
use lazy_regex::regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ConstraintsAnalytics {
    pub constraints_count: usize,
    pub invariants_count: usize,
    // #[serde(default)]
    pub non_invariants_count: usize,
    pub invariants: Vec<ConstraintInfo>,
    pub non_invariants: Vec<ConstraintInfo>,
    pub to_review: Vec<ConstraintInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ConstraintInfo {
    pub invariant: bool,
    pub reviewed: bool,
    pub constraint: String,
    pub entry_points: Vec<String>,
}

impl ConstraintsAnalytics {
    pub fn generate_analytics_data() -> AnalyticsResult<()> {
        let mut bat_analytics = BatAnalytics::read_analytics().change_context(AnalyticsError)?;
        let bat_metadata = BatMetadata::read_metadata().change_context(AnalyticsError)?;
        let entry_points_metadata = bat_metadata.clone().entry_points;
        let mut constraints_hashmap: HashMap<String, Vec<String>> = HashMap::new();
        let constraint_regex = regex!(r#"constraint[\s]*=[\s]*"#);
        let to_replace_regex = regex!(r#"(@[\w:()]*)|(\.?(load|as_ref|key)\(?\)?\??)"#);
        // let ops_regex = regex!(r#"(.load\(\)\?)|(.key\(\))"#);
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
                    let validation = to_replace_regex
                        .replace_all(&validation, "")
                        .trim()
                        .to_string();
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
            constraints_analytics_vec.push(ConstraintInfo {
                invariant: true,
                reviewed: false,
                constraint: constraint.clone(),
                entry_points: constraints_hashmap.get(constraint).unwrap().clone(),
            })
        }
        let new_analytics = ConstraintsAnalytics {
            constraints_count: constraints_analytics_vec.len(),
            invariants_count: 0,
            non_invariants_count: 0,
            invariants: vec![],
            non_invariants: vec![],
            to_review: constraints_analytics_vec,
        };
        bat_analytics.constraints = new_analytics;
        bat_analytics.save_analytics()?;
        Ok(())
    }

    pub fn update_analytics_data() -> AnalyticsResult<()> {
        let mut bat_analytics = BatAnalytics::read_analytics().change_context(AnalyticsError)?;
        let mut analytics_total = vec![];
        let ConstraintsAnalytics {
            constraints_count,
            mut invariants_count,
            mut non_invariants_count,
            mut invariants,
            mut non_invariants,
            mut to_review,
        } = bat_analytics.constraints;
        analytics_total.append(&mut invariants);
        analytics_total.append(&mut non_invariants);
        analytics_total.append(&mut to_review);

        let (reviewed, not_reviewed): (Vec<ConstraintInfo>, Vec<ConstraintInfo>) = analytics_total
            .into_iter()
            .partition(|analytics| analytics.reviewed);

        let (mut invariant_reviewed, mut non_invariant_reviewed): (
            Vec<ConstraintInfo>,
            Vec<ConstraintInfo>,
        ) = not_reviewed
            .clone()
            .into_iter()
            .map(|analytics| {
                let mut cloned = analytics.clone();
                let prompt_text = format!(
                    "is this constraint invariant?:\n{}\n entry points:\n{:#?}",
                    analytics.constraint.bright_green(),
                    analytics.entry_points
                );
                let is_invariant = BatDialoguer::select_yes_or_no(prompt_text).unwrap();
                cloned.invariant = is_invariant;
                cloned.reviewed = true;
                cloned
            })
            .partition(|analytics| analytics.invariant);

        let (mut invariant_vec, mut non_invariant_vec): (Vec<ConstraintInfo>, Vec<ConstraintInfo>) =
            reviewed
                .into_iter()
                .partition(|analytic| analytic.invariant);
        let mut invariants_total = vec![];
        invariants_total.append(&mut invariant_vec);
        invariants_total.append(&mut invariant_reviewed);

        let mut non_invariants_total = vec![];
        non_invariants_total.append(&mut non_invariant_reviewed);
        non_invariants_total.append(&mut non_invariant_vec);

        bat_analytics.constraints.invariants_count = invariants_total.len();
        bat_analytics.constraints.invariants = invariants_total;
        bat_analytics.constraints.non_invariants_count = non_invariants_total.len();
        bat_analytics.constraints.non_invariants = non_invariants_total;
        bat_analytics.constraints.to_review = vec![];
        bat_analytics.save_analytics()?;
        Ok(())
    }
}
