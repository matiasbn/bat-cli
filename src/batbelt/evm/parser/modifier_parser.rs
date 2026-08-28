use solar_parse::{ast, interface::Session};

use crate::batbelt::evm::types::{EvmModifierDef, EvmParam};

use super::evm_file_parser::{
    extract_source_by_lines, span_to_end_line, span_to_line, type_to_string,
};

/// Parse a modifier (ItemFunction with kind == Modifier) into EvmModifierDef.
pub fn parse_modifier_definition(
    sess: &Session,
    func: &ast::ItemFunction<'_>,
    contract_name: &str,
    source: &str,
) -> EvmModifierDef {
    let name = func
        .header
        .name
        .map(|n| n.as_str().to_string())
        .unwrap_or_default();

    let params: Vec<EvmParam> = func
        .header
        .parameters
        .iter()
        .map(|p| parse_parameter(sess, p))
        .collect();

    let full_span = func.header.span.to(func.body_span);
    let line = span_to_line(sess, full_span);
    let end_line = span_to_end_line(sess, full_span);

    // The WHOLE modifier (header + body), like a function — so a multi-line
    // signature parses and a write's line maps back to the file by `line`.
    let body_source = if func.body.is_some() {
        extract_source_by_lines(source, line, end_line)
    } else {
        String::new()
    };

    EvmModifierDef {
        name,
        params,
        body_source,
        line,
        end_line,
        contract_name: contract_name.to_string(),
        storage_writes: Vec::new(),
        storage_write_sites: Vec::new(),
    }
}

fn parse_parameter(sess: &Session, p: &ast::VariableDefinition<'_>) -> EvmParam {
    let name = p.name.map(|n| n.as_str().to_string()).unwrap_or_default();
    let type_name = type_to_string(sess, &p.ty);
    let storage_location = p.data_location.map(|s| s.to_str().to_string());

    EvmParam {
        name,
        type_name,
        storage_location,
    }
}
