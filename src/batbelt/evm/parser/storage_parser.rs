use quote::ToTokens;
use syn_solidity::Spanned;

use crate::batbelt::evm::types::{EvmVisibility, StorageVariable};

use super::evm_file_parser::span_to_line;

/// Parse a VariableDefinition into a StorageVariable.
pub fn parse_variable_definition(
    var: &syn_solidity::VariableDefinition,
    _source: &str,
) -> Option<StorageVariable> {
    let name = var.name.to_string();
    if name.is_empty() {
        return None;
    }

    let type_name = var.ty.to_string();

    let visibility = var
        .attributes
        .0
        .iter()
        .find_map(|attr| {
            if let syn_solidity::VariableAttribute::Visibility(vis) = attr {
                Some(match vis {
                    syn_solidity::Visibility::External(_) => EvmVisibility::External,
                    syn_solidity::Visibility::Public(_) => EvmVisibility::Public,
                    syn_solidity::Visibility::Internal(_) => EvmVisibility::Internal,
                    syn_solidity::Visibility::Private(_) => EvmVisibility::Private,
                })
            } else {
                None
            }
        })
        .unwrap_or(EvmVisibility::Internal);

    let is_constant = var
        .attributes
        .0
        .iter()
        .any(|attr| matches!(attr, syn_solidity::VariableAttribute::Constant(_)));

    let is_immutable = var
        .attributes
        .0
        .iter()
        .any(|attr| matches!(attr, syn_solidity::VariableAttribute::Immutable(_)));

    let line = span_to_line(var.name.span());

    Some(StorageVariable {
        name,
        type_name,
        visibility,
        is_constant,
        is_immutable,
        line,
    })
}
