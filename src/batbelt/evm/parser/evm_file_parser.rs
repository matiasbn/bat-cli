use error_stack::Report;
use std::fs;
use std::io;

use syn_solidity::{self, Item};

use super::{EvmParserError, EvmParserResult};
use crate::batbelt::evm::types::{EvmFile, EvmImport, ImportSymbol};

use super::contract_parser::parse_contract_definition;

/// Parse a single .sol file into a `EvmFile` structure.
pub fn parse_sol_file(file_path: &str) -> EvmParserResult<EvmFile> {
    let source = fs::read_to_string(file_path).map_err(|e: io::Error| {
        Report::new(EvmParserError)
            .attach_printable(format!("Cannot read {}: {}", file_path, e))
    })?;

    if source.is_empty() {
        return Err(Report::new(EvmParserError)
            .attach_printable(format!("File is empty or unreadable: {}", file_path)));
    }

    let tokens: proc_macro2::TokenStream = source.parse().map_err(|e: proc_macro2::LexError| {
        Report::new(EvmParserError)
            .attach_printable(format!("Lex error in {}: {}", file_path, e))
    })?;

    let sol_file_ast: syn_solidity::File = syn_solidity::parse2(tokens).map_err(|e| {
        Report::new(EvmParserError)
            .attach_printable(format!("Parse error in {}: {}", file_path, e))
    })?;

    let mut sol_file = EvmFile {
        path: file_path.to_string(),
        imports: Vec::new(),
        contracts: Vec::new(),
        pragma: None,
    };

    for item in &sol_file_ast.items {
        match item {
            Item::Pragma(pragma) => {
                // Extract pragma tokens as string (e.g. "solidity ^0.8.23")
                sol_file.pragma = Some(pragma.tokens.to_string());
            }
            Item::Import(import_dir) => {
                let sol_import = parse_import(import_dir);
                sol_file.imports.push(sol_import);
            }
            Item::Contract(contract_def) => {
                let contract = parse_contract_definition(contract_def, file_path, &source);
                sol_file.contracts.push(contract);
            }
            _ => {}
        }
    }

    Ok(sol_file)
}

/// Parse an import directive into our EvmImport type.
fn parse_import(import: &syn_solidity::ImportDirective) -> EvmImport {
    use syn_solidity::{ImportPath, Spanned};

    match &import.path {
        ImportPath::Plain(plain) => EvmImport {
            path: plain.path.value(),
            symbols: if let Some(alias) = &plain.alias {
                vec![ImportSymbol {
                    name: "*".to_string(),
                    alias: Some(alias.alias.to_string()),
                }]
            } else {
                vec![]
            },
            line: span_to_line(import.span()),
        },
        ImportPath::Aliases(aliases) => {
            let symbols = aliases
                .imports
                .iter()
                .map(|(ident, alias)| ImportSymbol {
                    name: ident.to_string(),
                    alias: alias.as_ref().map(|a| a.alias.to_string()),
                })
                .collect();
            EvmImport {
                path: aliases.path.value(),
                symbols,
                line: span_to_line(import.span()),
            }
        }
        ImportPath::Glob(glob) => EvmImport {
            path: glob.path.value(),
            symbols: vec![ImportSymbol {
                name: "*".to_string(),
                alias: glob.alias.as_ref().map(|a| a.alias.to_string()),
            }],
            line: span_to_line(import.span()),
        },
    }
}

/// Get 1-based line number from span start.
/// Requires proc-macro2 "span_locations" feature.
pub fn span_to_line(span: proc_macro2::Span) -> usize {
    span.start().line
}

/// Get 1-based line number from span end.
pub fn span_to_end_line(span: proc_macro2::Span) -> usize {
    span.end().line
}

/// Extract source text between two 1-based line numbers (inclusive).
pub fn extract_source_by_lines(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = if start_line > 0 { start_line - 1 } else { 0 };
    let end = end_line.min(lines.len());
    lines[start..end].join("\n")
}
