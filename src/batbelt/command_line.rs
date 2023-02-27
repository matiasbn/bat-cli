use std::io::Read;
use std::process::{ChildStdout, Command, Stdio};
use std::str::from_utf8;

use error_stack::{IntoReport, Report, Result, ResultExt};
use serde::{Deserialize, Serialize};

use crate::batbelt::BatEnumerator;
use crate::commands::{CommandError, CommandResult};
use crate::config::BatAuditorConfig;

#[derive(
    Default,
    Debug,
    Serialize,
    Deserialize,
    Clone,
    strum_macros::EnumIter,
    strum_macros::Display,
    PartialOrd,
    PartialEq,
)]
pub enum CodeEditor {
    #[default]
    CLion,
    VSCode,
    None,
}

impl CodeEditor {
    pub fn open_file_in_editor(path: &str, line_index: Option<usize>) -> CommandResult<()> {
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
        if !bat_auditor_config.use_code_editor {
            log::warn!("Code editor disabled");
            println!("Path to file: {:#?}:{}", path, line_index.unwrap_or(0));
            return Ok(());
        }
        let starting_line = line_index.unwrap_or(0);
        match bat_auditor_config.code_editor {
            CodeEditor::CLion => {
                execute_command(
                    "clion",
                    &["--line", &format!("{}", starting_line), path],
                    false,
                )
                .change_context(CommandError)?;
            }
            CodeEditor::VSCode => {
                let formatted_path = if starting_line == 0 {
                    path.to_string()
                } else {
                    format!("{};{}", path, starting_line)
                };
                execute_command("code", &["-a", &formatted_path], false)
                    .change_context(CommandError)?;
            }
            _ => {
                println!("Path to file: {:#?}:{}", path, starting_line);
            }
        }
        Ok(())
    }
}

impl BatEnumerator for CodeEditor {}

pub fn execute_command(command: &str, args: &[&str], print_output: bool) -> CommandResult<String> {
    let message = format!(
        "Error spawning a child process for parameters: \n command: {} \n args: {:#?}",
        command, args
    );

    let output = Command::new(command)
        .args(args)
        .output()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!(
        "Error reading parsing output to string: \n {:#?}",
        output.stdout
    );

    let output_string = from_utf8(output.stdout.as_slice())
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?
        .to_string();

    log::debug!("output_string: \n{}", output_string);

    if print_output {
        println!("{}", output_string);
    }

    Ok(output_string)
}
