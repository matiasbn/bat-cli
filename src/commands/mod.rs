pub mod co_commands;
pub mod finding_commands;
pub mod miro_commands;
pub mod project_commands;
pub mod repository_commands;
pub mod sonar_commands;
pub mod tools_commands;

use crate::batbelt::BatEnumerator;
use inflector::Inflector;
use regex::Regex;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct CommandError;

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command error")
    }
}

impl Error for CommandError {}

pub type CommandResult<T> = error_stack::Result<T, CommandError>;

pub trait BatCommandEnumerator
where
    Self: BatEnumerator,
{
    fn execute_command(&self) -> CommandResult<()>;
    fn check_metadata_is_initialized(&self) -> bool;
    fn check_correct_branch(&self) -> bool;
    fn get_bat_package_json_commands(command_name: String) -> BatPackageJsonCommand {
        let command_with_options_regex = Regex::new(r"\w+ \{\s*([\s\w]+: false,\n)+\}").unwrap();
        let boolean_flag_regex = Regex::new(r"\w+: false,").unwrap();

        let commands_vec = Self::iter()
            .map(|command| format!("{command:#?}"))
            .collect::<Vec<String>>();

        let mut command_options: Vec<BatPackageJsonCommandOptions> = vec![];

        for command in commands_vec.clone() {
            let mut json_command_options = BatPackageJsonCommandOptions {
                command_option_name: command
                    .split(" ")
                    .next()
                    .unwrap()
                    .to_string()
                    .to_kebab_case(),
                command_option_flags: vec![],
            };
            if command_with_options_regex.is_match(&command) {
                let option_flags = boolean_flag_regex
                    .find_iter(&command)
                    .map(|flag_match| {
                        flag_match
                            .as_str()
                            .split(": ")
                            .next()
                            .unwrap()
                            .to_string()
                            .to_kebab_case()
                    })
                    .collect::<Vec<_>>();
                json_command_options.command_option_flags = option_flags;
            }
            command_options.push(json_command_options);
        }
        let bat_package_json_command = BatPackageJsonCommand {
            command_name,
            command_options,
        };
        bat_package_json_command
    }
}

#[derive(Debug, Clone)]
pub struct BatPackageJsonCommand {
    pub command_name: String,
    pub command_options: Vec<BatPackageJsonCommandOptions>,
}

#[derive(Debug, Clone)]
pub struct BatPackageJsonCommandOptions {
    pub command_option_name: String,
    pub command_option_flags: Vec<String>,
}

impl BatPackageJsonCommandOptions {
    pub fn get_combinations_vec(&self) -> Vec<Vec<String>> {
        let mut result = vec![];
        for (option_flag_index, option_flag) in
            self.command_option_flags.clone().into_iter().enumerate()
        {
            let mut inner_vec = vec![];
            inner_vec.push(option_flag.clone());
            result.push(inner_vec.clone());
            let mut idx = option_flag_index + 1;
            while idx < self.command_option_flags.len() {
                inner_vec.push(self.command_option_flags[idx].clone());
                result.push(inner_vec.clone());
                idx += 1;
            }
        }
        result.sort_by(|vec_a, vec_b| vec_a.len().cmp(&vec_b.len()));
        result
    }
}

// {
// "author": "",
// "description": "Bat project",
// "license": "ISC",
// "main": "index.js",
// "name": "bat_project",
// "scripts": {
// "cargo::run::co::finish": "cargo run co finish",
// "cargo::run::co::start": "cargo run co start",
// "cargo::run::finding::accept-all": "cargo run finding accept-all",
// "cargo::run::finding::create": "cargo run finding create",
// "cargo::run::finding::finish": "cargo run finding finish",
// "cargo::run::finding::reject": "cargo run finding reject",
// "cargo::run::finding::update": "cargo run finding update",
// "cargo::run::miro::code-overhaul-frames": "cargo run miro code-overhaul-frames",
// "cargo::run::miro::code-overhaul-screenshots": "cargo run miro code-overhaul-screenshots",
// "cargo::run::miro::entrypoint-screenshots": "cargo run miro entrypoint-screenshots",
// "cargo::run::miro::function-dependencies": "cargo run miro function-dependencies",
// "cargo::run::miro::metadata": "cargo run miro metadata",
// "cargo::run::reload": "cargo run reload",
// "cargo::run::repo::delete-local-branches": "cargo run repo delete-local-branches",
// "cargo::run::repo::fetch-remote-branches": "cargo run repo fetch-remote-branches",
// "cargo::run::repo::update-branches": "cargo run repo update-branches",
// "cargo::run::repo::update-code-overhaul": "cargo run repo update-code-overhaul",
// "cargo::run::repo::update-notes": "cargo run repo update-notes",
// "cargo::run::sonar": "cargo run sonar",
// "cargo::run::tools::count-code-overhaul": "cargo run tools count-code-overhaul",
// "cargo::run::tools::customize-package-json": "cargo run tools customize-package-json",
// "cargo::run::tools::open-code-overhaul-files": "cargo run tools open-code-overhaul-files",
// "cargo::run::tools::open-metadata": "cargo run tools open-metadata",
// "cargo::run::tools::open-metadata-by-id": "cargo run tools open-metadata-by-id"
// },
// "version": "1.0.0"
// }
