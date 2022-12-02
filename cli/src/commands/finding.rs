use std::string::String;

use crate::config::{BatConfig, BatConfigValidation, FindingConfigValidation};

pub fn create_finding_file(finding_name: String) {
    // create finding file
    BatConfig::validate_bat_config();
    BatConfig::validate_create_finding_config(finding_name.clone());
    println!("{:?}", finding_name)
}
