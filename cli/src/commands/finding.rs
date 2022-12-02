use std::{process::Command, string::String};

use crate::config::{BatConfig, BatConfigValidation, FindingConfigValidation};

pub fn create_finding_file(finding_name: String, informational: bool) {
    BatConfig::validate_bat_config();
    BatConfig::validate_create_finding_config(finding_name.clone());
    copy_template_to_findings_to_review(finding_name, informational)
}

fn copy_template_to_findings_to_review(finding_name: String, informational: bool) {
    let template_path = if informational {
        BatConfig::get_informational_template_path()
    } else {
        BatConfig::get_finding_template_path()
    };
    let new_file_path = BatConfig::get_auditor_findings_to_review_path(Some(finding_name.clone()));
    let output = Command::new("cp")
        .args([template_path, new_file_path])
        .output()
        .unwrap()
        .status
        .exit_ok();
    if let Err(output) = output {
        panic!("Finding creation failed with reason: {:#?}", output)
    };
}
