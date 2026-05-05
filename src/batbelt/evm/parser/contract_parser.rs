use solang_parser::helpers::CodeLocation;
use solang_parser::pt::{self, ContractPart, ContractTy};

use crate::batbelt::evm::types::{
    EvmContract, EvmContractType, EvmEvent, EvmFunction, EvmModifierDef, StorageVariable,
};

use super::event_parser::parse_event_definition;
use super::function_parser::parse_function_definition;
use super::modifier_parser::parse_modifier_definition;
use super::evm_file_parser::offset_to_line;
use super::storage_parser::parse_variable_definition;

/// Parse a ContractDefinition AST node into our EvmContract type.
pub fn parse_contract_definition(
    contract: &pt::ContractDefinition,
    file_path: &str,
    source: &str,
) -> EvmContract {
    let name = contract
        .name
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_default();

    let contract_type = match &contract.ty {
        ContractTy::Contract(_) => {
            if contract.base.is_empty()
                && contract
                    .parts
                    .iter()
                    .all(|p| matches!(p, ContractPart::FunctionDefinition(f) if f.body.is_none()))
            {
                // All functions without body = likely interface, but declared as contract
                EvmContractType::Contract
            } else {
                EvmContractType::Contract
            }
        }
        ContractTy::Interface(_) => EvmContractType::Interface,
        ContractTy::Abstract(_) => EvmContractType::Abstract,
        ContractTy::Library(_) => EvmContractType::Library,
    };

    let base_contracts: Vec<String> = contract
        .base
        .iter()
        .map(|base| {
            base.name
                .identifiers
                .iter()
                .map(|id| id.name.clone())
                .collect::<Vec<_>>()
                .join(".")
        })
        .collect();

    let mut functions: Vec<EvmFunction> = Vec::new();
    let mut modifiers: Vec<EvmModifierDef> = Vec::new();
    let mut storage_variables: Vec<StorageVariable> = Vec::new();
    let mut events: Vec<EvmEvent> = Vec::new();

    for part in &contract.parts {
        match part {
            ContractPart::FunctionDefinition(func) => {
                functions.push(parse_function_definition(func, &name, source));
            }
            ContractPart::VariableDefinition(var) => {
                if let Some(sv) = parse_variable_definition(var, source) {
                    storage_variables.push(sv);
                }
            }
            ContractPart::EventDefinition(event) => {
                events.push(parse_event_definition(event, source));
            }
            ContractPart::ErrorDefinition(_) => {}
            ContractPart::StructDefinition(_) => {}
            ContractPart::EnumDefinition(_) => {}
            ContractPart::TypeDefinition(_) => {}
            ContractPart::Using(_) => {}
            ContractPart::StraySemicolon(_) => {}
            ContractPart::Annotation(_) => {}
        }
    }

    // Extract modifiers from function definitions that are actually modifiers
    for part in &contract.parts {
        if let ContractPart::FunctionDefinition(func) = part {
            if func.ty == pt::FunctionTy::Modifier {
                modifiers.push(parse_modifier_definition(func, &name, source));
            }
        }
    }

    let line = match &contract.loc {
        pt::Loc::File(_, start, _) => offset_to_line(source, *start),
        _ => 0,
    };

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
