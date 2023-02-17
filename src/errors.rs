use std::{error::Error, fmt};

use error_stack::Report;

#[derive(Debug)]
pub struct BatError;

impl fmt::Display for BatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("General Bat error")
    }
}

impl Error for BatError {}

#[derive(Debug)]
pub enum BatErrorType {
    ReadToString { path: &'static str },
    ReadDir { path: &'static str },
    Write { path: &'static str },
    Other { error: String },
}

impl BatErrorType {
    pub fn parse_error(&self) -> Report<BatError> {
        match self {
            BatErrorType::ReadToString { path } => {
                let message = format!("Error reading file to string:\n path: {} ", path);
                Report::new(BatError).attach_printable(message)
            }
            BatErrorType::ReadDir { path } => {
                let message = format!("Error reading dir:\n path: {} ", path);
                Report::new(BatError).attach_printable(message)
            }
            BatErrorType::Write { path } => {
                let message = format!("Error write to file:\npath: {} ", path);
                Report::new(BatError).attach_printable(message)
            }
            BatErrorType::Other { error } => {
                let message = format!("Error detected:\nerror: \n{} ", error);
                Report::new(BatError).attach_printable(message)
            }
        }
    }
}

// pub fn execute_command(command: &str, args: &[&str]) -> Result<(), BashError> {
//     let mut output = Command::new(command).args(args).spawn().or_else(|err| {
//         let message = format!(
//             "Error spawning a child process for paramenters: \n command: {} \n args: {:#?}",
//             command, args
//         );
//         Err(Report::new(BashError).attach_printable(message))
//     });
//     output?.wait().or_else(|err| {
//         let message = format!(
//             "Error waiting a child process for paramenters: \n command: {} \n args: {:#?}",
//             command, args
//         );
//         Err(Report::new(BashError).attach_printable(message))
//     });
//     Ok(())
// }
