use syn_solidity::{Item, Spanned};

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
    contract: &syn_solidity::ItemContract,
    file_path: &str,
    source: &str,
) -> EvmContract {
    let name = contract.name.to_string();

    let contract_type = if contract.kind.is_interface() {
        EvmContractType::Interface
    } else if contract.kind.is_abstract_contract() {
        EvmContractType::Abstract
    } else if contract.kind.is_library() {
        EvmContractType::Library
    } else {
        EvmContractType::Contract
    };

    let base_contracts: Vec<String> = contract
        .inheritance
        .as_ref()
        .map(|inh| {
            inh.inheritance
                .iter()
                .map(|base| base.name.to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut functions: Vec<EvmFunction> = Vec::new();
    let mut modifiers: Vec<EvmModifierDef> = Vec::new();
    let mut storage_variables: Vec<StorageVariable> = Vec::new();
    let mut events: Vec<EvmEvent> = Vec::new();

    for item in &contract.body {
        match item {
            Item::Function(func) => {
                if matches!(func.kind, syn_solidity::FunctionKind::Modifier(_)) {
                    modifiers.push(parse_modifier_definition(func, &name, source));
                } else {
                    functions.push(parse_function_definition(func, &name, source));
                }
            }
            Item::Variable(var) => {
                if let Some(sv) = parse_variable_definition(var, source) {
                    storage_variables.push(sv);
                }
            }
            Item::Event(event) => {
                events.push(parse_event_definition(event, source));
            }
            _ => {}
        }
    }

    let line = span_to_line(contract.name.span());

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
