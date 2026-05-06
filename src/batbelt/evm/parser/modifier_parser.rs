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

    let body_source = func
        .body
        .as_ref()
        .map(|block| {
            extract_source_by_lines(
                source,
                span_to_line(sess, block.span),
                span_to_end_line(sess, block.span),
            )
        })
        .unwrap_or_default();

    let line = span_to_line(sess, func.header.span.to(func.body_span));

    EvmModifierDef {
        name,
        params,
        body_source,
        line,
        contract_name: contract_name.to_string(),
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
