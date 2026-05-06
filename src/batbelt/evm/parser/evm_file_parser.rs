use error_stack::Report;
use std::fs;
use std::io;
use std::path::Path;

use solar_parse::{
    ast,
    interface::{Session, Span},
    Parser,
};

use super::{EvmParserError, EvmParserResult};
use crate::batbelt::evm::types::{EvmFile, EvmImport, ImportSymbol};

use super::contract_parser::parse_contract_definition;

/// Parse a single .sol file into a `EvmFile` structure.
pub fn parse_sol_file(file_path: &str) -> EvmParserResult<EvmFile> {
    let source = fs::read_to_string(file_path).map_err(|e: io::Error| {
        Report::new(EvmParserError).attach_printable(format!("Cannot read {}: {}", file_path, e))
    })?;

    if source.is_empty() {
        return Err(Report::new(EvmParserError)
            .attach_printable(format!("File is empty or unreadable: {}", file_path)));
    }

    let sess = Session::builder()
        .with_buffer_emitter(solar_parse::interface::ColorChoice::Auto)
        .build();

    let result = sess.enter(|| -> Result<EvmFile, Report<EvmParserError>> {
        let arena = ast::Arena::new();
        let mut parser = Parser::from_file(&sess, &arena, Path::new(file_path)).map_err(|_| {
            Report::new(EvmParserError).attach_printable(format!("Cannot open {}", file_path))
        })?;

        let ast = parser.parse_file().map_err(|e| {
            e.emit();
            Report::new(EvmParserError).attach_printable(format!("Parse error in {}", file_path))
        })?;

        let mut sol_file = EvmFile {
            path: file_path.to_string(),
            imports: Vec::new(),
            contracts: Vec::new(),
            pragma: None,
        };

        for item in ast.items.iter() {
            match &item.kind {
                ast::ItemKind::Pragma(pragma) => {
                    sol_file.pragma = Some(match &pragma.tokens {
                        ast::PragmaTokens::Version(name, req) => {
                            format!("{} {}", name.as_str(), req)
                        }
                        ast::PragmaTokens::Custom(name, value) => {
                            if let Some(val) = value {
                                format!("{} {}", name.as_str(), val.as_str())
                            } else {
                                name.as_str().to_string()
                            }
                        }
                        other => format!("{:?}", other),
                    });
                }
                ast::ItemKind::Import(import_dir) => {
                    let sol_import = parse_import(&sess, import_dir, item.span);
                    sol_file.imports.push(sol_import);
                }
                ast::ItemKind::Contract(contract_def) => {
                    let contract =
                        parse_contract_definition(&sess, contract_def, file_path, &source);
                    sol_file.contracts.push(contract);
                }
                _ => {}
            }
        }

        Ok(sol_file)
    });

    result
}

/// Parse an import directive into our EvmImport type.
fn parse_import(sess: &Session, import: &ast::ImportDirective<'_>, item_span: Span) -> EvmImport {
    let path = import.path.value.as_str().to_string();
    let line = span_to_line(sess, item_span);

    match &import.items {
        ast::ImportItems::Plain(alias) => EvmImport {
            path,
            symbols: if let Some(alias_ident) = alias {
                vec![ImportSymbol {
                    name: "*".to_string(),
                    alias: Some(alias_ident.as_str().to_string()),
                }]
            } else {
                vec![]
            },
            line,
        },
        ast::ImportItems::Aliases(aliases) => {
            let symbols = aliases
                .iter()
                .map(|(ident, alias)| ImportSymbol {
                    name: ident.as_str().to_string(),
                    alias: alias.as_ref().map(|a| a.as_str().to_string()),
                })
                .collect();
            EvmImport {
                path,
                symbols,
                line,
            }
        }
        ast::ImportItems::Glob(alias_ident) => EvmImport {
            path,
            symbols: vec![ImportSymbol {
                name: "*".to_string(),
                alias: Some(alias_ident.as_str().to_string()),
            }],
            line,
        },
    }
}

/// Get 1-based line number from span start using solar-parse source map.
pub fn span_to_line(sess: &Session, span: Span) -> usize {
    sess.source_map().lookup_char_pos(span.lo()).line
}

/// Get 1-based line number from span end using solar-parse source map.
pub fn span_to_end_line(sess: &Session, span: Span) -> usize {
    sess.source_map().lookup_char_pos(span.hi()).line
}

/// Convert a solar-parse Type to a human-readable string using source map span text.
pub fn type_to_string(sess: &Session, ty: &ast::Type<'_>) -> String {
    sess.source_map()
        .span_to_snippet(ty.span)
        .unwrap_or_else(|_| format!("{:?}", ty.kind))
}

/// Extract source text between two 1-based line numbers (inclusive).
pub fn extract_source_by_lines(source: &str, start_line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = if start_line > 0 { start_line - 1 } else { 0 };
    let end = end_line.min(lines.len());
    lines[start..end].join("\n")
}
