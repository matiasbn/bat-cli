use std::error::Error;
use std::fmt;

pub mod solana_account_parser;
pub mod context_accounts_parser;
pub mod entrypoint_parser;
pub mod function_parser;
pub mod source_code_parser;

#[derive(Debug)]
pub struct ParserError;

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EntrypointParser error")
    }
}

impl Error for ParserError {}
