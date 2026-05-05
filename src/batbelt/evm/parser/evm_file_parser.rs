use error_stack::Report;
use solang_parser::pt::{self, SourceUnitPart};
use std::fs;
use std::io;

use super::{EvmParserError, EvmParserResult};
use crate::batbelt::evm::types::{ImportSymbol, EvmFile, EvmImport};

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

    let (source_unit, _comments) = solang_parser::parse(&source, 0).map_err(|diags| {
        let msg = diags
            .iter()
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
            .join("; ");
        Report::new(EvmParserError)
            .attach_printable(format!("Parse error in {}: {}", file_path, msg))
    })?;

    let mut sol_file = EvmFile {
        path: file_path.to_string(),
        imports: Vec::new(),
        contracts: Vec::new(),
        pragma: None,
    };

    for part in &source_unit.0 {
        match part {
            SourceUnitPart::PragmaDirective(pragma) => {
                sol_file.pragma = Some(format_pragma(pragma));
            }
            SourceUnitPart::ImportDirective(import) => {
                let sol_import = parse_import(import, &source);
                sol_file.imports.push(sol_import);
            }
            SourceUnitPart::ContractDefinition(contract_def) => {
                let contract = parse_contract_definition(contract_def, file_path, &source);
                sol_file.contracts.push(contract);
            }
            _ => {}
        }
    }

    Ok(sol_file)
}

fn format_pragma(pragma: &pt::PragmaDirective) -> String {
    match pragma {
        pt::PragmaDirective::Identifier(_, ident, value) => {
            let id = ident.as_ref().map(|i| i.name.as_str()).unwrap_or("");
            let val = value.as_ref().map(|v| v.name.as_str()).unwrap_or("");
            format!("{} {}", id, val).trim().to_string()
        }
        pt::PragmaDirective::StringLiteral(_, ident, lit) => {
            format!("{} \"{}\"", ident.name, lit.string)
        }
        pt::PragmaDirective::Version(_, ident, comparators) => {
            let version_str = comparators
                .iter()
                .map(|c| format!("{}", c))
                .collect::<Vec<_>>()
                .join(" ");
            format!("{} {}", ident.name, version_str)
        }
    }
}

/// Parse an import directive into our EvmImport type.
fn parse_import(import: &pt::Import, _source: &str) -> EvmImport {
    match import {
        pt::Import::Plain(path, loc) => EvmImport {
            path: import_path_to_string(path),
            symbols: vec![],
            line: loc_to_line(loc),
        },
        pt::Import::GlobalSymbol(path, alias, loc) => EvmImport {
            path: import_path_to_string(path),
            symbols: vec![ImportSymbol {
                name: "*".to_string(),
                alias: Some(alias.name.clone()),
            }],
            line: loc_to_line(loc),
        },
        pt::Import::Rename(path, renames, loc) => {
            let symbols = renames
                .iter()
                .map(|(ident, alias)| ImportSymbol {
                    name: ident.name.clone(),
                    alias: alias.as_ref().map(|a| a.name.clone()),
                })
                .collect();
            EvmImport {
                path: import_path_to_string(path),
                symbols,
                line: loc_to_line(loc),
            }
        }
    }
}

fn import_path_to_string(path: &pt::ImportPath) -> String {
    match path {
        pt::ImportPath::Filename(lit) => lit.string.clone(),
        pt::ImportPath::Path(ident_path) => ident_path
            .identifiers
            .iter()
            .map(|id| id.name.clone())
            .collect::<Vec<_>>()
            .join("."),
    }
}

fn loc_to_line(loc: &pt::Loc) -> usize {
    match loc {
        pt::Loc::File(_, start, _) => *start,
        _ => 0,
    }
}

/// Convert a byte offset to a 1-based line number.
pub fn offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())]
        .chars()
        .filter(|&c| c == '\n')
        .count()
        + 1
}
