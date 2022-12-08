use std::process::Command;

use crate::{config::BatConfig, constants::BASE_REPOSTORY_NAME, git::clone_base_repository};

pub fn update_templates() {
    let BatConfig { required: _, .. } = BatConfig::get_validated_config();

    // clone base repository
    clone_base_repository();

    // delete templates folder
    let templates_path = BatConfig::get_templates_path();
    let output = Command::new("rm")
        .args(["-rf", templates_path.as_str()])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    // move template to now location
    let output = Command::new("mv")
        .args([
            BASE_REPOSTORY_NAME.to_string() + "/templates",
            templates_path,
        ])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };
    // delete base_repository cloned
    let output = Command::new("rm")
        .args(["-rf", BASE_REPOSTORY_NAME])
        .output()
        .unwrap();
    if !output.stderr.is_empty() {
        panic!(
            "update templates failed with error: {:?}",
            std::str::from_utf8(output.stderr.as_slice()).unwrap()
        )
    };

    println!("Templates folder successfully updated");
}
