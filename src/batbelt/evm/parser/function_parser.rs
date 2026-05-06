use quote::ToTokens;
use syn_solidity::Spanned;

use crate::batbelt::evm::types::{EvmFunction, EvmMutability, EvmParam, EvmVisibility};

use super::evm_file_parser::{extract_source_by_lines, span_to_end_line, span_to_line};

/// Parse an ItemFunction AST node into our EvmFunction type.
pub fn parse_function_definition(
    func: &syn_solidity::ItemFunction,
    contract_name: &str,
    source: &str,
) -> EvmFunction {
    let name = func
        .name
        .as_ref()
        .map(|n| n.to_string())
        .unwrap_or_else(|| match &func.kind {
            syn_solidity::FunctionKind::Constructor(_) => "constructor".to_string(),
            syn_solidity::FunctionKind::Fallback(_) => "fallback".to_string(),
            syn_solidity::FunctionKind::Receive(_) => "receive".to_string(),
            syn_solidity::FunctionKind::Modifier(_) => "modifier".to_string(),
            syn_solidity::FunctionKind::Function(_) => "unnamed".to_string(),
        });

    let visibility = extract_visibility(&func.attributes);
    let mutability = extract_mutability(&func.attributes);

    let modifiers: Vec<String> = func
        .attributes
        .iter()
        .filter_map(|attr| {
            if let syn_solidity::FunctionAttribute::Modifier(m) = attr {
                Some(m.name.to_string())
            } else {
                None
            }
        })
        .collect();

    let params: Vec<EvmParam> = func
        .parameters
        .iter()
        .map(parse_parameter)
        .collect();

    let returns: Vec<EvmParam> = func
        .returns
        .as_ref()
        .map(|r| r.returns.iter().map(parse_parameter).collect())
        .unwrap_or_default();

    let body_source = match &func.body {
        syn_solidity::FunctionBody::Block(block) => {
            // Extract body from original source using span lines
            extract_source_by_lines(source, span_to_line(block.brace_token.span.open()), span_to_end_line(block.brace_token.span.close()))
        }
        _ => String::new(),
    };

    let line = span_to_line(func.span());
    let end_line = span_to_end_line(func.span());

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
        is_constructor: matches!(func.kind, syn_solidity::FunctionKind::Constructor(_)),
        is_fallback: matches!(func.kind, syn_solidity::FunctionKind::Fallback(_)),
        is_receive: matches!(func.kind, syn_solidity::FunctionKind::Receive(_)),
    }
}

fn extract_visibility(attrs: &syn_solidity::FunctionAttributes) -> EvmVisibility {
    for attr in attrs.iter() {
        if let syn_solidity::FunctionAttribute::Visibility(vis) = attr {
            return match vis {
                syn_solidity::Visibility::External(_) => EvmVisibility::External,
                syn_solidity::Visibility::Public(_) => EvmVisibility::Public,
                syn_solidity::Visibility::Internal(_) => EvmVisibility::Internal,
                syn_solidity::Visibility::Private(_) => EvmVisibility::Private,
            };
        }
    }
    EvmVisibility::Internal
}

fn extract_mutability(attrs: &syn_solidity::FunctionAttributes) -> EvmMutability {
    for attr in attrs.iter() {
        if let syn_solidity::FunctionAttribute::Mutability(m) = attr {
            return match m {
                syn_solidity::Mutability::Pure(_) => EvmMutability::Pure,
                syn_solidity::Mutability::View(_) => EvmMutability::View,
                syn_solidity::Mutability::Payable(_) => EvmMutability::Payable,
                syn_solidity::Mutability::Constant(_) => EvmMutability::View,
            };
        }
    }
    EvmMutability::NonPayable
}

fn parse_parameter(p: &syn_solidity::VariableDeclaration) -> EvmParam {
    let name = p
        .name
        .as_ref()
        .map(|n| n.to_string())
        .unwrap_or_default();
    let type_name = p.ty.to_string();
    let storage_location = p.storage.as_ref().map(|s| s.as_str().to_string());

    EvmParam {
        name,
        type_name,
        storage_location,
    }
}
