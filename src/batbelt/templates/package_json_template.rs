use crate::batbelt::path::BatFile;
use crate::batbelt::templates::TemplateError;
use crate::config::BatConfig;
use error_stack::{IntoReport, Result, ResultExt};
use serde_json::json;
use std::fs;

pub struct PackageJsonTemplate;

impl PackageJsonTemplate {
    pub fn update_package_json() -> Result<(), TemplateError> {
        let content = Self::get_package_json_content();
        let package_path = BatFile::PackageJson
            .get_path(false)
            .change_context(TemplateError)?;
        fs::write(&package_path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    pub fn create_package_json() -> Result<(), TemplateError> {
        let content = Self::get_package_json_content();
        let package_path = format!(
            "{}/package.json",
            BatConfig::get_config()
                .change_context(TemplateError)?
                .project_name
        );
        fs::write(&package_path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    pub fn get_package_json_content() -> String {
        let package_json = json!({
        "name": "bat_project",
        "version": "1.0.0",
        "description": "Bat project",
        "main": "index.js",
        "scripts": {
          "cargo::run::co::start": " cargo run co start",
          "cargo::run::co::finish": " cargo run co finish",
          "cargo::run::co::open": " cargo run co open",
          "cargo::run::update": " cargo run update",
          "cargo::run::notes": " cargo run notes",
          "bat-cli::co::start": " bat-cli co start",
          "bat-cli::co::finish": " bat-cli co finish",
          "bat-cli::co::open": " bat-cli co open",
          "bat-cli::notes": " bat-cli notes",
          "bat-cli::update": " bat-cli update"
        },
        "author": "",
        "license": "ISC"
        });
        let content = serde_json::to_string_pretty(&package_json).unwrap();
        content
    }
}

#[test]
fn test_get_package_json_content() {
    let json_content = PackageJsonTemplate::get_package_json_content();
    println!("{}", json_content);
}

#[test]
fn test_update_package_json_content() {
    let json_content = PackageJsonTemplate::get_package_json_content();
    println!("{}", json_content);
    fs::write("./package_test.json", json_content).unwrap();
}
