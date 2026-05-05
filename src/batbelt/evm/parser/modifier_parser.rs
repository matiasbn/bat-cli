use solang_parser::helpers::CodeLocation;
use solang_parser::pt;

use crate::batbelt::evm::types::{EvmModifierDef, EvmParam};

use super::evm_file_parser::offset_to_line;

/// Parse a modifier (FunctionDefinition with ty == Modifier) into EvmModifierDef.
pub fn parse_modifier_definition(
    func: &pt::FunctionDefinition,
    contract_name: &str,
    source: &str,
) -> EvmModifierDef {
    let name = func
        .name
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_default();

    let params: Vec<EvmParam> = func
        .params
        .iter()
        .filter_map(|(_, param)| {
            param.as_ref().map(|p| {
                let param_name = p.name.as_ref().map(|n| n.name.clone()).unwrap_or_default();
                let type_name = extract_source(source, &p.ty.loc());
                EvmParam {
                    name: param_name,
                    type_name,
                    storage_location: p.storage.as_ref().map(|s| match s {
                        pt::StorageLocation::Memory(_) => "memory".to_string(),
                        pt::StorageLocation::Storage(_) => "storage".to_string(),
                        pt::StorageLocation::Calldata(_) => "calldata".to_string(),
                    }),
                }
            })
        })
        .collect();

    let body_source = func
        .body
        .as_ref()
        .map(|body| extract_source(source, &body.loc()))
        .unwrap_or_default();

    let line = match &func.loc {
        pt::Loc::File(_, start, _) => offset_to_line(source, *start),
        _ => 0,
    };

    EvmModifierDef {
        name,
        params,
        body_source,
        line,
        contract_name: contract_name.to_string(),
    }
}

fn extract_source(source: &str, loc: &pt::Loc) -> String {
    match loc {
        pt::Loc::File(_, start, end) => {
            let start = (*start).min(source.len());
            let end = (*end).min(source.len());
            source[start..end].to_string()
        }
        _ => String::new(),
    }
}
