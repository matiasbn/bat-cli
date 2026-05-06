use solar_parse::{
    ast,
    interface::Session,
};

use crate::batbelt::evm::types::{EvmFunction, EvmMutability, EvmParam, EvmVisibility};

use super::evm_file_parser::{extract_source_by_lines, span_to_end_line, span_to_line, type_to_string};

/// Parse an ItemFunction AST node into our EvmFunction type.
pub fn parse_function_definition(
    sess: &Session,
    func: &ast::ItemFunction<'_>,
    contract_name: &str,
    source: &str,
) -> EvmFunction {
    let name = func
        .header
        .name
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| match func.kind {
            ast::FunctionKind::Constructor => "constructor".to_string(),
            ast::FunctionKind::Fallback => "fallback".to_string(),
            ast::FunctionKind::Receive => "receive".to_string(),
            ast::FunctionKind::Modifier => "modifier".to_string(),
            ast::FunctionKind::Function => "unnamed".to_string(),
        });

    let visibility = func
        .header
        .visibility
        .as_ref()
        .map(|v| match v.data {
            ast::Visibility::External => EvmVisibility::External,
            ast::Visibility::Public => EvmVisibility::Public,
            ast::Visibility::Internal => EvmVisibility::Internal,
            ast::Visibility::Private => EvmVisibility::Private,
        })
        .unwrap_or(EvmVisibility::Internal);

    let mutability = func
        .header
        .state_mutability
        .as_ref()
        .map(|m| match m.data {
            ast::StateMutability::Pure => EvmMutability::Pure,
            ast::StateMutability::View => EvmMutability::View,
            ast::StateMutability::Payable => EvmMutability::Payable,
            ast::StateMutability::NonPayable => EvmMutability::NonPayable,
        })
        .unwrap_or(EvmMutability::NonPayable);

    let modifiers: Vec<String> = func
        .header
        .modifiers
        .iter()
        .map(|m| m.name.last().as_str().to_string())
        .collect();

    let params: Vec<EvmParam> = func
        .header
        .parameters
        .iter()
        .map(|p| parse_parameter(sess, p))
        .collect();

    let returns: Vec<EvmParam> = func
        .header
        .returns
        .as_ref()
        .map(|r| r.iter().map(|p| parse_parameter(sess, p)).collect())
        .unwrap_or_default();

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

    // Use full function span: from header start to body end
    let full_span = func.header.span.to(func.body_span);
    let line = span_to_line(sess, full_span);
    let end_line = span_to_end_line(sess, full_span);

    EvmFunction {
        name,
        contract_name: contract_name.to_string(),
        visibility,
        mutability,
        modifiers,
        params,
        returns,
        body_source,
        line,
        end_line,
        is_constructor: func.kind == ast::FunctionKind::Constructor,
        is_fallback: func.kind == ast::FunctionKind::Fallback,
        is_receive: func.kind == ast::FunctionKind::Receive,
    }
}

fn parse_parameter(sess: &Session, p: &ast::VariableDefinition<'_>) -> EvmParam {
    let name = p
        .name
        .map(|n| n.as_str().to_string())
        .unwrap_or_default();
    let type_name = type_to_string(sess, &p.ty);
    let storage_location = p.data_location.map(|s| s.to_str().to_string());

    EvmParam {
        name,
        type_name,
        storage_location,
    }
}
