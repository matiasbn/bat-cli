use solang_parser::helpers::CodeLocation;
use solang_parser::pt;

use crate::batbelt::evm::types::{EvmEvent, EvmParam};

use super::evm_file_parser::offset_to_line;

/// Parse an EventDefinition into a EvmEvent.
pub fn parse_event_definition(event: &pt::EventDefinition, source: &str) -> EvmEvent {
    let name = event
        .name
        .as_ref()
        .map(|n| n.name.clone())
        .unwrap_or_default();

    let params: Vec<EvmParam> = event
        .fields
        .iter()
        .map(|field| {
            let param_name = field
                .name
                .as_ref()
                .map(|n| n.name.clone())
                .unwrap_or_default();
            let type_name = extract_source(source, &field.ty.loc());
            EvmParam {
                name: param_name,
                type_name,
                storage_location: None,
            }
        })
        .collect();

    let line = match &event.loc {
        pt::Loc::File(_, start, _) => offset_to_line(source, *start),
        _ => 0,
    };

    EvmEvent {
        name,
        params,
        is_anonymous: event.anonymous,
        line,
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
