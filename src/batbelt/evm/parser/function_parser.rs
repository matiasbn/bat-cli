use solang_parser::helpers::CodeLocation;
use solang_parser::pt::{self, FunctionTy, Visibility};

use crate::batbelt::evm::types::{EvmFunction, EvmMutability, EvmParam, EvmVisibility};

use super::evm_file_parser::offset_to_line;

/// Parse a FunctionDefinition AST node into our EvmFunction type.
pub fn parse_function_definition(
    func: &pt::FunctionDefinition,
    contract_name: &str,
    source: &str,
) -> EvmFunction {
    let name = func
        .name
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_else(|| match func.ty {
            FunctionTy::Constructor => "constructor".to_string(),
            FunctionTy::Fallback => "fallback".to_string(),
            FunctionTy::Receive => "receive".to_string(),
            FunctionTy::Modifier => "modifier".to_string(),
            _ => "unnamed".to_string(),
        });

    let visibility = extract_visibility(&func.attributes);
    let mutability = extract_mutability(&func.attributes);

    let modifiers: Vec<String> = func
        .attributes
        .iter()
        .filter_map(|attr| {
            if let pt::FunctionAttribute::BaseOrModifier(_, base) = attr {
                Some(
                    base.name
                        .identifiers
                        .iter()
                        .map(|id| id.name.clone())
                        .collect::<Vec<_>>()
                        .join("."),
                )
            } else {
                None
            }
        })
        .collect();

    let params: Vec<EvmParam> = func
        .params
        .iter()
        .filter_map(|(_, param)| param.as_ref().map(|p| parse_parameter(p, source)))
        .collect();

    let returns: Vec<EvmParam> = func
        .returns
        .iter()
        .filter_map(|(_, param)| param.as_ref().map(|p| parse_parameter(p, source)))
        .collect();

    let body_source = func
        .body
        .as_ref()
        .map(|body| extract_source_range(source, &body.loc()))
        .unwrap_or_default();

    let line = match &func.loc {
        pt::Loc::File(_, start, _) => offset_to_line(source, *start),
        _ => 0,
    };

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
        is_constructor: func.ty == FunctionTy::Constructor,
        is_fallback: func.ty == FunctionTy::Fallback,
        is_receive: func.ty == FunctionTy::Receive,
    }
}

fn extract_visibility(attrs: &[pt::FunctionAttribute]) -> EvmVisibility {
    for attr in attrs {
        if let pt::FunctionAttribute::Visibility(vis) = attr {
            return match vis {
                Visibility::External(_) => EvmVisibility::External,
                Visibility::Public(_) => EvmVisibility::Public,
                Visibility::Internal(_) => EvmVisibility::Internal,
                Visibility::Private(_) => EvmVisibility::Private,
            };
        }
    }
    // Default visibility for functions is internal
    EvmVisibility::Internal
}

fn extract_mutability(attrs: &[pt::FunctionAttribute]) -> EvmMutability {
    for attr in attrs {
        if let pt::FunctionAttribute::Mutability(m) = attr {
            return match m {
                pt::Mutability::Pure(_) => EvmMutability::Pure,
                pt::Mutability::View(_) => EvmMutability::View,
                pt::Mutability::Payable(_) => EvmMutability::Payable,
                pt::Mutability::Constant(_) => EvmMutability::View,
            };
        }
    }
    EvmMutability::NonPayable
}

fn parse_parameter(param: &pt::Parameter, source: &str) -> EvmParam {
    let name = param
        .name
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_default();

    let type_name = extract_source_range(source, &param.ty.loc());

    let storage_location = param.storage.as_ref().map(|s| match s {
        pt::StorageLocation::Memory(_) => "memory".to_string(),
        pt::StorageLocation::Storage(_) => "storage".to_string(),
        pt::StorageLocation::Calldata(_) => "calldata".to_string(),
    });

    EvmParam {
        name,
        type_name,
        storage_location,
    }
}

/// Extract source code text for a given location.
fn extract_source_range(source: &str, loc: &pt::Loc) -> String {
    match loc {
        pt::Loc::File(_, start, end) => {
            let start = (*start).min(source.len());
            let end = (*end).min(source.len());
            source[start..end].to_string()
        }
        _ => String::new(),
    }
}
