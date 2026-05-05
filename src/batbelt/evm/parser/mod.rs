pub mod call_resolver;
pub mod contract_parser;
pub mod evm_file_parser;
pub mod event_parser;
pub mod function_parser;
pub mod import_resolver;
pub mod inheritance_resolver;
pub mod modifier_parser;
pub mod storage_parser;

use std::{error::Error, fmt};

#[derive(Debug)]
pub struct EvmParserError;

impl fmt::Display for EvmParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvmParser error")
    }
}

impl Error for EvmParserError {}

pub type EvmParserResult<T> = error_stack::Result<T, EvmParserError>;
