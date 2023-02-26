use colored::Colorize;
use std::error::Error;
use std::fmt;

pub mod context_accounts_parser;
pub mod entrypoint_parser;
pub mod function_parser;
pub mod solana_account_parser;
pub mod source_code_parser;
pub mod trait_impl_parser;

#[derive(Debug)]
pub struct ParserError;

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntrypointParser error")
    }
}

impl Error for ParserError {}

pub fn parse_formatted_path(name: String, path: String, start_line_index: usize) -> String {
    format!(
        "{}: {}:{}",
        name.blue(),
        path.trim_start_matches("../"),
        start_line_index
    )
}
