use error_stack::{FutureExt, IntoReport, Result, ResultExt};
use log::Level;

use serde_json::Map;
use serde_json::{json, Value};

use crate::batbelt::path::BatFile;
use crate::batbelt::templates::{TemplateError, TemplateResult};

use crate::commands::{BatPackageJsonCommand, BatPackageJsonCommandOptions};
use crate::BatCommands;

pub struct PackageJsonTemplate;

impl PackageJsonTemplate {
    pub fn create_package_json(log_level: Option<Level>) -> Result<(), TemplateError> {
        BatFile::PackageJson
            .write_content(false, &Self::get_package_json_content(log_level)?)
            .change_context(TemplateError)?;
        Ok(())
    }

    fn get_package_json_content(log_level: Option<Level>) -> TemplateResult<String> {
        let scripts_value = Self::get_scripts_serde_value(log_level)?;
        let package_json = json!({
        "name": "bat_project",
        "version": "1.0.0",
        "description": "Bat project",
        "main": "index.js",
        "scripts": scripts_value,
        "author": "",
        "license": "ISC"
        });

        Ok(serde_json::to_string_pretty(&package_json).unwrap())
    }

    fn get_scripts_serde_value(log_level: Option<Level>) -> TemplateResult<Value> {
        let (verbosity_flag, verbosity_level_name) = if let Some(level) = log_level {
            match level {
                Level::Warn => ("v".to_string(), level.to_string()),
                Level::Info => ("vv".to_string(), level.to_string()),
                Level::Debug => ("vvv".to_string(), level.to_string()),
                Level::Trace => ("vvvv".to_string(), level.to_string()),
                _ => ("".to_string(), "".to_string()),
            }
        } else {
            ("".to_string(), "".to_string())
        };
        let (script_key_prefix, script_value_prefix) = if cfg!(debug_assertions) {
            if verbosity_flag.is_empty() {
                ("".to_string(), "cargo run".to_string())
            } else {
                (
                    format!("{}::", verbosity_level_name),
                    format!("cargo run -- -{}", verbosity_flag),
                )
            }
        } else if verbosity_flag.is_empty() {
            ("".to_string(), "bat-cli".to_string())
        } else {
            (
                format!("{}::", verbosity_level_name),
                format!("bat-cli -{}", verbosity_flag),
            )
        };
        let bat_package_json_commands_vec = BatCommands::get_bat_package_json_commands();
        let mut scripts_map = Map::new();
        for bat_command in bat_package_json_commands_vec {
            let BatPackageJsonCommand {
                command_name,
                command_options,
            } = bat_command;
            if command_options.is_empty() {
                let script_key = format!("{}{}", script_key_prefix, command_name);
                let script_value = format!("{} {}", script_value_prefix, command_name);
                scripts_map.insert(script_key, script_value.into());
                continue;
            }
            for command_option in command_options {
                let command_option_clone = command_option.clone();

                let BatPackageJsonCommandOptions {
                    command_option_name,
                    command_option_flags,
                } = command_option;
                if command_name == "sonar" {
                    let script_key = format!("{}{}", script_key_prefix, command_name);
                    let script_value = format!("{} {}", script_value_prefix, command_name);
                    scripts_map.insert(script_key.clone(), script_value.clone().into());
                } else {
                    let script_key = format!(
                        "{}{}::{}",
                        script_key_prefix, command_name, command_option_name
                    );
                    let script_value = format!(
                        "{} {} {}",
                        script_value_prefix, command_name, command_option_name
                    );
                    scripts_map.insert(script_key.clone(), script_value.clone().into());
                };

                if !command_option_flags.is_empty() {
                    let combinations_vec = command_option_clone
                        .clone()
                        .get_combinations_vec(&command_name);
                    for combination in combinations_vec {
                        let key_string = combination
                            .clone()
                            .into_iter()
                            .fold("".to_string(), |result, current| {
                                format!("{}::{}", result, current)
                            });
                        let value_string = combination
                            .clone()
                            .into_iter()
                            .fold("".to_string(), |result, current| {
                                format!("{} --{}", result, current)
                            });
                        if command_name == "sonar" {
                            let script_key =
                                format!("{}{}{}", script_key_prefix, command_name, key_string);
                            let script_value = format!(
                                "{} {} {}",
                                script_value_prefix, command_name, value_string
                            );
                            scripts_map.insert(script_key.clone(), script_value.clone().into());
                        } else {
                            let script_key = format!(
                                "{}{}::{}{}",
                                script_key_prefix, command_name, command_option_name, key_string
                            );
                            let script_value = format!(
                                "{} {} {} {}",
                                script_value_prefix,
                                command_name,
                                command_option_name,
                                value_string
                            );
                            scripts_map.insert(script_key, script_value.into());
                        }
                    }
                }
            }
        }
        let serde_value: Value = scripts_map.into();
        Ok(serde_value)
    }
}

#[cfg(test)]
mod template_test {
    use crate::batbelt::templates::package_json_template::PackageJsonTemplate;

    #[test]
    fn test_get_package_json_content() {
        let json_content = PackageJsonTemplate::get_package_json_content(None).unwrap();
        println!("{}", json_content);
    }
}
