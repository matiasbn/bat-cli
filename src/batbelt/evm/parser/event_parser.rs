use solar_parse::{ast, interface::Session};

use crate::batbelt::evm::types::{EvmEvent, EvmParam};

use super::evm_file_parser::{span_to_line, type_to_string};

/// Parse an ItemEvent into an EvmEvent.
pub fn parse_event_definition(sess: &Session, event: &ast::ItemEvent<'_>) -> EvmEvent {
    let name = event.name.as_str().to_string();

    let params: Vec<EvmParam> = event
        .parameters
        .iter()
        .map(|field| {
            let param_name = field
                .name
                .map(|n| n.as_str().to_string())
                .unwrap_or_default();
            let type_name = type_to_string(sess, &field.ty);
            EvmParam {
                name: param_name,
                type_name,
                storage_location: None,
            }
        })
        .collect();

    let line = span_to_line(sess, event.name.span);

    EvmEvent {
        name,
        params,
        is_anonymous: event.anonymous,
        line,
    }
}
