use quote::ToTokens;
use syn_solidity::Spanned;

use crate::batbelt::evm::types::{EvmEvent, EvmParam};

use super::evm_file_parser::span_to_line;

/// Parse an ItemEvent into an EvmEvent.
pub fn parse_event_definition(event: &syn_solidity::ItemEvent, _source: &str) -> EvmEvent {
    let name = event.name.to_string();

    let params: Vec<EvmParam> = event
        .parameters
        .iter()
        .map(|field| {
            let param_name = field
                .name
                .as_ref()
                .map(|n| n.to_string())
                .unwrap_or_default();
            let type_name = field.ty.to_string();
            EvmParam {
                name: param_name,
                type_name,
                storage_location: None,
            }
        })
        .collect();

    let line = span_to_line(event.name.span());

    EvmEvent {
        name,
        params,
        is_anonymous: event.anonymous.is_some(),
        line,
    }
}
