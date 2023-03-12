use crate::batbelt::path::prettify_source_code_path;
use colored::Colorize;
use error_stack::Result;
use std::error::Error;
use std::fmt;

pub mod code_overhaul_parser;
pub mod context_accounts_parser;
pub mod entrypoint_parser;
pub mod function_parser;
pub mod solana_account_parser;
pub mod source_code_parser;
pub mod trait_parser;

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
        prettify_source_code_path(path.trim_start_matches("../")).unwrap(),
        start_line_index
    )
}
