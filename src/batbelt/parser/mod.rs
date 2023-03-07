use colored::Colorize;
use error_stack::Result;
use std::error::Error;
use std::fmt;

pub mod context_accounts_parser;
pub mod entrypoint_parser;
pub mod function_parser;
pub mod solana_account_parser;
pub mod source_code_parser;
pub mod trait_parser;
pub mod code_overhaul_parser;

#[derive(Debug)]
pub struct ParserError;

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntrypointParser error")
    }
}

impl Error for ParserError {}

pub type ParserResult<T> = Result<T, ParserError>;

pub fn parse_formatted_path(name: String, path: String, start_line_index: usize) -> String {
    format!(
        "{}: {}:{}",
        name.blue(),
        path.trim_start_matches("../"),
        start_line_index
    )
}
