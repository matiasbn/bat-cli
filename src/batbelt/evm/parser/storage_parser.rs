use solang_parser::helpers::CodeLocation;
use solang_parser::pt;

use crate::batbelt::evm::types::{EvmVisibility, StorageVariable};

use super::evm_file_parser::offset_to_line;

/// Parse a VariableDefinition into a StorageVariable.
pub fn parse_variable_definition(
    var: &pt::VariableDefinition,
    source: &str,
) -> Option<StorageVariable> {
    let name = var.name.as_ref().map(|n| n.name.clone())?;

    let type_name = extract_type_source(source, &var.ty.loc());

    let visibility = var
        .attrs
        .iter()
        .find_map(|attr| {
            if let pt::VariableAttribute::Visibility(vis) = attr {
                Some(match vis {
                    pt::Visibility::External(_) => EvmVisibility::External,
                    pt::Visibility::Public(_) => EvmVisibility::Public,
                    pt::Visibility::Internal(_) => EvmVisibility::Internal,
                    pt::Visibility::Private(_) => EvmVisibility::Private,
                })
            } else {
                None
            }
        })
        .unwrap_or(EvmVisibility::Internal);

    let is_constant = var
        .attrs
        .iter()
        .any(|attr| matches!(attr, pt::VariableAttribute::Constant(_)));

    let is_immutable = var
        .attrs
        .iter()
        .any(|attr| matches!(attr, pt::VariableAttribute::Immutable(_)));

    let line = match &var.loc {
        pt::Loc::File(_, start, _) => offset_to_line(source, *start),
        _ => 0,
    };

    Some(StorageVariable {
        name,
        type_name,
        visibility,
        is_constant,
        is_immutable,
        line,
    })
}

fn extract_type_source(source: &str, loc: &pt::Loc) -> String {
    match loc {
        pt::Loc::File(_, start, end) => {
            let start = (*start).min(source.len());
            let end = (*end).min(source.len());
            source[start..end].to_string()
        }
        _ => "unknown".to_string(),
    }
}
