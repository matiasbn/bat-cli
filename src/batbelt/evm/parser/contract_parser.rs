use solar_parse::{ast, interface::Session};

use crate::batbelt::evm::types::{
    EvmContract, EvmContractType, EvmEvent, EvmFunction, EvmModifierDef, StorageVariable,
};

use super::event_parser::parse_event_definition;
use super::evm_file_parser::span_to_line;
use super::function_parser::parse_function_definition;
use super::modifier_parser::parse_modifier_definition;
use super::storage_parser::parse_variable_definition;

/// Parse an ItemContract AST node into our EvmContract type.
pub fn parse_contract_definition(
    sess: &Session,
    contract: &ast::ItemContract<'_>,
    file_path: &str,
    source: &str,
) -> EvmContract {
    let name = contract.name.as_str().to_string();

    let contract_type = match contract.kind {
        ast::ContractKind::Contract => EvmContractType::Contract,
        ast::ContractKind::AbstractContract => EvmContractType::Abstract,
        ast::ContractKind::Interface => EvmContractType::Interface,
        ast::ContractKind::Library => EvmContractType::Library,
    };

    let base_contracts: Vec<String> = contract
        .bases
        .iter()
        .map(|base| base.name.last().as_str().to_string())
        .collect();

    let mut functions: Vec<EvmFunction> = Vec::new();
    let mut modifiers: Vec<EvmModifierDef> = Vec::new();
    let mut storage_variables: Vec<StorageVariable> = Vec::new();
    let mut events: Vec<EvmEvent> = Vec::new();

    for item in contract.body.iter() {
        match &item.kind {
            ast::ItemKind::Function(func) => {
                if func.kind == ast::FunctionKind::Modifier {
                    modifiers.push(parse_modifier_definition(sess, func, &name, source));
                } else {
                    functions.push(parse_function_definition(sess, func, &name, source));
                }
            }
            ast::ItemKind::Variable(var) => {
                if let Some(sv) = parse_variable_definition(sess, var) {
                    storage_variables.push(sv);
                }
            }
            ast::ItemKind::Event(event) => {
                events.push(parse_event_definition(sess, event));
            }
            _ => {}
        }
    }

    let line = span_to_line(sess, contract.name.span);

    let external = file_path.contains("/lib/");

    EvmContract {
        name,
        contract_type,
        base_contracts,
        functions,
        modifiers,
        storage_variables,
        events,
        file_path: file_path.to_string(),
        line,
        external,
    }
}
