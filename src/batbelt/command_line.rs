use colored::Colorize;
use std::io::Read;
use std::process::{Command, Stdio};
use std::str::from_utf8;

use error_stack::{IntoReport, ResultExt};
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

impl BatEnumerator for CodeEditor {}

impl CodeEditor {
    pub fn open_file_in_editor(path: &str, line_index: Option<usize>) -> CommandResult<()> {
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(CommandError)?;
        if !bat_auditor_config.use_code_editor {
            log::warn!("Code editor disabled");
            println!("Path to file: {:#?}:{}", path, line_index.unwrap_or(0));
            return Ok(());
        }
        let starting_line = line_index.unwrap_or(0);
        println!(
            "Opening {} on {}!",
            path.trim_start_matches("../").green(),
            bat_auditor_config.code_editor.get_colored_name(false)
        );
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

pub fn execute_command(command: &str, args: &[&str], print_output: bool) -> CommandResult<String> {
    let message = format!(
        "Error executing a process for parameters: \n command: {} \n args: {:#?}",
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

    log::debug!(target:"execute_command",  "command: {}\n args: {:#?}\noutput: \n{}", command, args, output_string);

    if print_output {
        println!("{}", output_string);
    }

    Ok(output_string)
}

pub fn execute_command_with_child_process(command: &str, args: &[&str]) -> CommandResult<String> {
    let message = format!(
        "Error spawning a child process for parameters: \n command: {} \n args: {:#?}",
        command, args
    );

    let mut output = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!("Error waiting child process: \n {:#?}", output.stdout);

    output
        .wait()
        .into_report()
        .change_context(CommandError)
        .attach_printable(message)?;

    let message = format!("Error waiting child process: \n {:#?}", output.stdout);

    let mut output_string = String::new();
    output
        .stdout
        .ok_or(CommandError)
        .into_report()
        .attach_printable(message)?
        .read_to_string(&mut output_string)
        .into_report()
        .change_context(CommandError)?;

    log::debug!(target:"execute_command_with_child_process",  "command: {}\n args: {:#?}\noutput: \n{}", command, args, output_string);

    Ok(output_string)
    // Ok("output_string".to_string())
}

#[cfg(test)]
mod command_line_tester {
    use crate::batbelt::command_line::execute_command_with_child_process;

    #[test]
    fn test_executed_piped() {
        env_logger::init();
        let ls_result = execute_command_with_child_process("gflfs", &["2.0.0"]).unwrap();
        // let ls_result = execute_child_process("cargo", &["install"]).unwrap();
        println!("ls_rrsuylt {}", ls_result)
        // let ls_result = execute_piped_process("ls", &["-la"], true).unwrap();
    }
}
