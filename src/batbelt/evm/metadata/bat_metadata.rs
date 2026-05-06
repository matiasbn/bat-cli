use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs};

use crate::batbelt::evm::types::{
    AccessControlType, EvmContract, EvmContractType, EvmEvent, EvmModifierDef, EvmMutability,
    EvmParam, EvmVisibility, StorageVariable,
};

#[derive(Debug)]
pub struct EvmMetadataError;

impl fmt::Display for EvmMetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("EvmMetadata error")
    }
}

impl Error for EvmMetadataError {}

pub type EvmMetadataResult<T> = error_stack::Result<T, EvmMetadataError>;

const EVM_METADATA_FILE: &str = "BatMetadata.json";

/// EVM-specific BatMetadata structure (separate from SVM metadata).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EvmBatMetadata {
    pub contracts: Vec<ContractMetadata>,
    pub entry_points: Vec<EntryPointMetadata>,
    pub function_dependencies: Vec<FunctionDependency>,
    pub interfaces: Vec<InterfaceMetadata>,
    #[serde(default)]
    pub miro: MiroMetadataRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractMetadata {
    pub metadata_id: String,
    pub name: String,
    pub file_path: String,
    pub contract_type: EvmContractType,
    pub base_contracts: Vec<String>,
    pub functions: Vec<FunctionMetadata>,
    pub state_variables: Vec<StorageVariable>,
    pub events: Vec<EvmEvent>,
    pub modifiers: Vec<EvmModifierDef>,
    pub line: usize,
    /// true if the contract comes from lib/ (external dependency)
    #[serde(default)]
    pub external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionMetadata {
    pub metadata_id: String,
    pub name: String,
    pub contract_name: String,
    pub visibility: EvmVisibility,
    pub mutability: EvmMutability,
    pub modifiers: Vec<String>,
    pub params: Vec<EvmParam>,
    pub returns: Vec<EvmParam>,
    pub line: usize,
    #[serde(default)]
    pub end_line: usize,
    pub is_constructor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPointMetadata {
    pub metadata_id: String,
    pub name: String,
    pub contract_name: String,
    pub function_metadata_id: String,
    pub access_control: Vec<AccessControlType>,
    pub storage_reads: Vec<String>,
    pub storage_writes: Vec<String>,
    pub external_calls: Vec<String>,
    pub events_emitted: Vec<String>,
    pub modifiers: Vec<String>,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDependency {
    pub function_metadata_id: String,
    pub callees: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceMetadata {
    pub name: String,
    pub implemented_by: Vec<String>,
    pub functions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MiroMetadataRef {
    pub frames: Vec<MiroFrameRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiroFrameRef {
    pub entry_point_name: String,
    pub frame_id: String,
    pub frame_url: String,
    #[serde(default)]
    pub images_deployed: bool,
    #[serde(default)]
    pub entry_point_image_id: String,
    #[serde(default)]
    pub validations_image_id: String,
    #[serde(default)]
    pub dependency_image_ids: Vec<String>,
}

impl EvmBatMetadata {
    pub fn read_metadata() -> EvmMetadataResult<Self> {
        let content = fs::read_to_string(EVM_METADATA_FILE).map_err(|e| {
            Report::new(EvmMetadataError)
                .attach_printable(format!("Cannot read {}: {}", EVM_METADATA_FILE, e))
        })?;
        let metadata: Self = serde_json::from_str(&content).map_err(|e| {
            Report::new(EvmMetadataError)
                .attach_printable(format!("Cannot parse {}: {}", EVM_METADATA_FILE, e))
        })?;
        Ok(metadata)
    }

    pub fn save_metadata(&self) -> EvmMetadataResult<()> {
        let content = serde_json::to_string_pretty(self).map_err(|e| {
            Report::new(EvmMetadataError)
                .attach_printable(format!("Cannot serialize metadata: {}", e))
        })?;
        fs::write(EVM_METADATA_FILE, content).map_err(|e| {
            Report::new(EvmMetadataError)
                .attach_printable(format!("Cannot write {}: {}", EVM_METADATA_FILE, e))
        })?;
        Ok(())
    }

    pub fn create_empty() -> EvmMetadataResult<()> {
        let metadata = Self::default();
        metadata.save_metadata()
    }

    /// Atomically read, modify, and save EVM metadata.
    pub fn update_metadata<F>(f: F) -> EvmMetadataResult<()>
    where
        F: FnOnce(&mut EvmBatMetadata),
    {
        let mut metadata = Self::read_metadata()?;
        f(&mut metadata);
        metadata.save_metadata()
    }

    /// Get miro frame ref by entry point name.
    pub fn get_miro_frame_by_ep_name(&self, ep_name: &str) -> Option<&MiroFrameRef> {
        self.miro
            .frames
            .iter()
            .find(|f| f.entry_point_name == ep_name)
    }

    pub fn get_contract_by_name(&self, name: &str) -> Option<&ContractMetadata> {
        self.contracts.iter().find(|c| c.name == name)
    }

    pub fn get_function_by_id(&self, id: &str) -> Option<&FunctionMetadata> {
        self.contracts
            .iter()
            .flat_map(|c| c.functions.iter())
            .find(|f| f.metadata_id == id)
    }

    pub fn get_entry_point_by_name(&self, name: &str) -> Option<&EntryPointMetadata> {
        self.entry_points.iter().find(|ep| ep.name == name)
    }

    /// Build metadata from parsed contracts.
    pub fn from_contracts(contracts: Vec<EvmContract>) -> Self {
        let mut metadata = Self::default();

        for contract in &contracts {
            let contract_id = format!("{}_{}", contract.file_path, contract.name);

            let functions: Vec<FunctionMetadata> = contract
                .functions
                .iter()
                .map(|f| {
                    let func_id = format!("{}_{}_{}", contract.file_path, contract.name, f.name);
                    FunctionMetadata {
                        metadata_id: func_id,
                        name: f.name.clone(),
                        contract_name: contract.name.clone(),
                        visibility: f.visibility.clone(),
                        mutability: f.mutability.clone(),
                        modifiers: f.modifiers.clone(),
                        params: f.params.clone(),
                        returns: f.returns.clone(),
                        line: f.line,
                        end_line: f.end_line,
                        is_constructor: f.is_constructor,
                    }
                })
                .collect();

            let contract_metadata = ContractMetadata {
                metadata_id: contract_id,
                name: contract.name.clone(),
                file_path: contract.file_path.clone(),
                contract_type: contract.contract_type.clone(),
                base_contracts: contract.base_contracts.clone(),
                functions,
                state_variables: contract.storage_variables.clone(),
                events: contract.events.clone(),
                modifiers: contract.modifiers.clone(),
                line: contract.line,
                external: contract.external,
            };

            metadata.contracts.push(contract_metadata);
        }

        // Build entry points from external/public functions (skip external/lib contracts)
        for contract in &metadata.contracts.clone() {
            if contract.external {
                continue;
            }
            if matches!(
                contract.contract_type,
                EvmContractType::Interface | EvmContractType::Library
            ) {
                continue;
            }

            // Detect overloaded function names within this contract
            let ep_functions: Vec<_> = contract
                .functions
                .iter()
                .filter(|f| {
                    matches!(
                        f.visibility,
                        EvmVisibility::External | EvmVisibility::Public
                    ) && !f.is_constructor
                })
                .collect();

            let mut name_counts: std::collections::HashMap<String, usize> =
                std::collections::HashMap::new();
            for func in &ep_functions {
                *name_counts.entry(func.name.clone()).or_insert(0) += 1;
            }

            for func in &ep_functions {
                // If overloaded, append param types to disambiguate
                let ep_name = if name_counts.get(&func.name).copied().unwrap_or(0) > 1 {
                    let param_types = func
                        .params
                        .iter()
                        .map(|p| p.type_name.clone())
                        .collect::<Vec<_>>()
                        .join(",");
                    format!("{}.{}({})", contract.name, func.name, param_types)
                } else {
                    format!("{}.{}", contract.name, func.name)
                };

                let ep = EntryPointMetadata {
                    metadata_id: format!("ep_{}", func.metadata_id),
                    name: ep_name,
                    contract_name: contract.name.clone(),
                    function_metadata_id: func.metadata_id.clone(),
                    access_control: detect_access_control(&func.modifiers),
                    storage_reads: vec![],
                    storage_writes: vec![],
                    external_calls: vec![],
                    events_emitted: vec![],
                    modifiers: func.modifiers.clone(),
                    dependencies: vec![],
                };
                metadata.entry_points.push(ep);
            }
        }

        metadata
    }
}

fn detect_access_control(modifiers: &[String]) -> Vec<AccessControlType> {
    let mut result = Vec::new();

    for modifier in modifiers {
        match modifier.as_str() {
            "onlyOwner" => result.push(AccessControlType::OnlyOwner),
            "onlyRole" => result.push(AccessControlType::RoleBased {
                role: "DEFAULT_ADMIN_ROLE".to_string(),
            }),
            other => {
                if other.starts_with("only") {
                    result.push(AccessControlType::CustomModifier {
                        name: other.to_string(),
                    });
                }
            }
        }
    }

    if result.is_empty() {
        result.push(AccessControlType::None);
    }

    result
}
