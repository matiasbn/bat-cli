use quote::ToTokens;
use syn_solidity::Spanned;

use crate::batbelt::evm::types::{EvmModifierDef, EvmParam};

use super::evm_file_parser::{extract_source_by_lines, span_to_end_line, span_to_line};

/// Parse a modifier (ItemFunction with kind == Modifier) into EvmModifierDef.
pub fn parse_modifier_definition(
    func: &syn_solidity::ItemFunction,
    contract_name: &str,
    source: &str,
) -> EvmModifierDef {
    let name = func
        .name
        .as_ref()
        .map(|n| n.to_string())
        .unwrap_or_default();

    let params: Vec<EvmParam> = func
        .parameters
        .iter()
        .map(|p| parse_parameter(p))
        .collect();

    let body_source = match &func.body {
        syn_solidity::FunctionBody::Block(block) => {
            extract_source_by_lines(source, span_to_line(block.brace_token.span.open()), span_to_end_line(block.brace_token.span.close()))
        }
        _ => String::new(),
    };

    let line = span_to_line(func.span());

    EvmModifierDef {
        name,
        params,
        body_source,
        line,
        contract_name: contract_name.to_string(),
    }
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
