use std::collections::HashMap;

use crate::batbelt::evm::types::{EvmContract, EvmFunction};

/// Represents a resolved function call.
#[derive(Debug, Clone)]
pub struct ResolvedCall {
    pub caller_contract: String,
    pub caller_function: String,
    pub callee_contract: String,
    pub callee_function: String,
    pub is_external: bool,
    pub is_super: bool,
}

/// Resolves function calls within and across contracts.
pub struct CallResolver<'a> {
    contracts_by_name: HashMap<String, &'a EvmContract>,
}

impl<'a> CallResolver<'a> {
    pub fn new(contracts: &'a [EvmContract]) -> Self {
        let contracts_by_name = contracts.iter().map(|c| (c.name.clone(), c)).collect();
        Self { contracts_by_name }
    }

    /// Resolve all calls in a function's body.
    pub fn resolve_calls(
        &self,
        contract_name: &str,
        function: &EvmFunction,
    ) -> Vec<ResolvedCall> {
        let mut calls = Vec::new();
        let body = &function.body_source;

        if body.is_empty() {
            return calls;
        }

        // Pattern: simple function call `functionName(`
        // This is a heuristic — we look for identifiers followed by `(`
        let identifier_pattern = regex::Regex::new(r"(\w+)\s*\(").unwrap();

        for cap in identifier_pattern.captures_iter(body) {
            let callee_name = &cap[1];

            // Skip Solidity keywords and built-ins
            if is_builtin(callee_name) {
                continue;
            }

            // Try to resolve the call
            if let Some(resolved) = self.resolve_single_call(
                contract_name,
                &function.name,
                callee_name,
                body,
            ) {
                calls.push(resolved);
            }
        }

        // Pattern: external call `contract.function(`
        let external_pattern = regex::Regex::new(r"(\w+)\.(\w+)\s*\(").unwrap();
        for cap in external_pattern.captures_iter(body) {
            let target = &cap[1];
            let method = &cap[2];

            if is_builtin(target) || is_builtin(method) {
                continue;
            }

            // Check if target is a known contract
            if self.contracts_by_name.contains_key(target) {
                calls.push(ResolvedCall {
                    caller_contract: contract_name.to_string(),
                    caller_function: function.name.clone(),
                    callee_contract: target.to_string(),
                    callee_function: method.to_string(),
                    is_external: true,
                    is_super: false,
                });
            }
        }

        // Pattern: super.function(
        let super_pattern = regex::Regex::new(r"super\.(\w+)\s*\(").unwrap();
        for cap in super_pattern.captures_iter(body) {
            let method = &cap[1];
            calls.push(ResolvedCall {
                caller_contract: contract_name.to_string(),
                caller_function: function.name.clone(),
                callee_contract: contract_name.to_string(),
                callee_function: method.to_string(),
                is_external: false,
                is_super: true,
            });
        }

        calls
    }

    fn resolve_single_call(
        &self,
        contract_name: &str,
        caller_function: &str,
        callee_name: &str,
        _body: &str,
    ) -> Option<ResolvedCall> {
        // First check internal functions in same contract
        if let Some(contract) = self.contracts_by_name.get(contract_name) {
            if contract.functions.iter().any(|f| f.name == callee_name) {
                return Some(ResolvedCall {
                    caller_contract: contract_name.to_string(),
                    caller_function: caller_function.to_string(),
                    callee_contract: contract_name.to_string(),
                    callee_function: callee_name.to_string(),
                    is_external: false,
                    is_super: false,
                });
            }
        }

        // Check inherited functions
        if let Some(contract) = self.contracts_by_name.get(contract_name) {
            for base_name in &contract.base_contracts {
                if let Some(base) = self.contracts_by_name.get(base_name.as_str()) {
                    if base.functions.iter().any(|f| f.name == callee_name) {
                        return Some(ResolvedCall {
                            caller_contract: contract_name.to_string(),
                            caller_function: caller_function.to_string(),
                            callee_contract: base_name.clone(),
                            callee_function: callee_name.to_string(),
                            is_external: false,
                            is_super: false,
                        });
                    }
                }
            }
        }

        None
    }
}

fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "require"
            | "assert"
            | "revert"
            | "emit"
            | "keccak256"
            | "sha256"
            | "ripemd160"
            | "ecrecover"
            | "addmod"
            | "mulmod"
            | "selfdestruct"
            | "type"
            | "abi"
            | "block"
            | "msg"
            | "tx"
            | "gasleft"
            | "blockhash"
            | "address"
            | "uint256"
            | "uint"
            | "int"
            | "bool"
            | "bytes"
            | "string"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "return"
            | "delete"
            | "new"
            | "this"
            | "super"
            | "push"
            | "pop"
            | "length"
    )
}
