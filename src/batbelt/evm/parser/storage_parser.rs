use solar_parse::{ast, interface::Session};

use crate::batbelt::evm::types::{EvmVisibility, StorageVariable};

use super::evm_file_parser::{span_to_line, type_to_string};

/// Parse a VariableDefinition into a StorageVariable.
pub fn parse_variable_definition(
    sess: &Session,
    var: &ast::VariableDefinition<'_>,
) -> Option<StorageVariable> {
    let name = var.name.map(|n| n.as_str().to_string()).unwrap_or_default();
    if name.is_empty() {
        return None;
    }

    let type_name = type_to_string(sess, &var.ty);

    let visibility = var
        .visibility
        .map(|vis| match vis {
            ast::Visibility::External => EvmVisibility::External,
            ast::Visibility::Public => EvmVisibility::Public,
            ast::Visibility::Internal => EvmVisibility::Internal,
            ast::Visibility::Private => EvmVisibility::Private,
        })
        .unwrap_or(EvmVisibility::Internal);

    let is_constant = var
        .mutability
        .map(|m| m == ast::VarMut::Constant)
        .unwrap_or(false);

    let is_immutable = var
        .mutability
        .map(|m| m == ast::VarMut::Immutable)
        .unwrap_or(false);

    let line = var.name.map(|n| span_to_line(sess, n.span)).unwrap_or(0);

    Some(StorageVariable {
        name,
        type_name,
        visibility,
        is_constant,
        is_immutable,
        line,
    })
}
