use std::collections::HashMap;

use crate::batbelt::evm::types::EvmContract;

/// Resolves C3 linearization for Solidity inheritance.
/// Returns contracts in linearized order (most derived first).
pub struct InheritanceResolver<'a> {
    contracts_by_name: HashMap<String, &'a EvmContract>,
}

impl<'a> InheritanceResolver<'a> {
    pub fn new(contracts: &'a [EvmContract]) -> Self {
        let contracts_by_name: HashMap<String, &EvmContract> = contracts
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();
        Self { contracts_by_name }
    }

    /// Get C3 linearization for a contract.
    /// Returns list of contract names from most derived (self) to most base.
    pub fn linearize(&self, contract_name: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut visited = Vec::new();
        self.c3_linearize(contract_name, &mut result, &mut visited);
        result
    }

    fn c3_linearize(&self, name: &str, result: &mut Vec<String>, visited: &mut Vec<String>) {
        if visited.contains(&name.to_string()) {
            return;
        }
        visited.push(name.to_string());

        if let Some(contract) = self.contracts_by_name.get(name) {
            // Process bases in reverse order (rightmost base is most base-like)
            for base_name in contract.base_contracts.iter().rev() {
                self.c3_linearize(base_name, result, visited);
            }
        }

        result.push(name.to_string());
    }

    /// Get all inherited functions for a contract (including from bases).
    /// Returns functions with contract name indicating where they're defined.
    pub fn get_all_functions(
        &self,
        contract_name: &str,
    ) -> Vec<(&str, &crate::batbelt::evm::types::EvmFunction)> {
        let linearization = self.linearize(contract_name);
        let mut result = Vec::new();
        let mut seen_signatures: Vec<String> = Vec::new();

        // Most derived first — overrides win
        for name in linearization.iter().rev() {
            if let Some(contract) = self.contracts_by_name.get(name.as_str()) {
                for func in &contract.functions {
                    let sig = format!("{}({})", func.name, func.params.len());
                    if !seen_signatures.contains(&sig) {
                        seen_signatures.push(sig);
                        result.push((contract.name.as_str(), func));
                    }
                }
            }
        }

        result
    }

    /// Check if a contract inherits from another (directly or transitively).
    pub fn inherits_from(&self, contract_name: &str, base_name: &str) -> bool {
        let linearization = self.linearize(contract_name);
        linearization.contains(&base_name.to_string()) && contract_name != base_name
    }
}
