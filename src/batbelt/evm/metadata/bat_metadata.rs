use error_stack::{Report, ResultExt};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs};

use crate::batbelt::evm::types::{
    AccessControlType, EvmContract, EvmContractType, EvmEvent, EvmFileItem, EvmModifierDef,
    EvmMutability, EvmParam, EvmVisibility, StorageVariable,
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
    pub file_items: Vec<EvmFileItem>,
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
    /// Storage locations this function writes (state vars / storage-pointer
    /// paths). Empty for a function that mutates no storage. Drives the
    /// "writes storage" marker on the diagram.
    #[serde(default)]
    pub storage_writes: Vec<String>,
    /// External calls that could NOT be statically resolved to a unique in-scope
    /// function — a call on an interface-typed receiver whose concrete target is a
    /// runtime property. An AI resolves these against the wiring to complete the
    /// cross-contract storage-change picture; each carries best-effort candidates.
    #[serde(default)]
    pub unresolved_calls: Vec<UnresolvedCall>,
}

/// An external call whose concrete target static analysis cannot pin down (the
/// receiver is an interface-typed variable — the implementation is bound at
/// runtime). Surfaced so an AI can resolve it from the deploy/wiring.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedCall {
    /// The call receiver as written, e.g. `positionManager`, `$.borrowerOps`.
    pub receiver: String,
    /// The method invoked, e.g. `adjustPosition`.
    pub method: String,
    /// The receiver's declared type when known (an interface, e.g.
    /// `IBorrowerOperations`); empty when it couldn't be typed (a struct field or
    /// local — resolved in a later phase).
    #[serde(default)]
    pub inferred_type: String,
    /// In-scope concrete contracts that plausibly implement this call (implementers
    /// of `inferred_type` that define `method`, else any in-scope contract defining
    /// `method`). The AI picks the real one from the wiring.
    #[serde(default)]
    pub candidates: Vec<String>,
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
    /// State of the automatic deployment (`miro evm-auto-deploy`).
    #[serde(default)]
    pub auto: AutoDeployState,
}

/// Everything the automatic deployment needs to stay incremental.
///
/// Keeping the allocator state here is what lets us place frames without ever
/// asking Miro where there is free space: the board is scanned once, when the
/// region is reserved, and never again.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoDeployState {
    pub region: Option<ShelfState>,
    #[serde(default)]
    pub frames: Vec<AutoDeployedFrame>,
}

/// Serializable snapshot of the shelf allocator cursor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShelfState {
    pub origin_x: f64,
    pub origin_y: f64,
    pub cursor_x: f64,
    pub cursor_y: f64,
    pub row_height: f64,
    pub row_max_width: f64,
    pub gutter: f64,
}

impl ShelfState {
    pub fn to_allocator(&self) -> crate::batbelt::miro::layout::ShelfAllocator {
        crate::batbelt::miro::layout::ShelfAllocator {
            origin_x: self.origin_x,
            origin_y: self.origin_y,
            cursor_x: self.cursor_x,
            cursor_y: self.cursor_y,
            row_height: self.row_height,
            row_max_width: self.row_max_width,
            gutter: self.gutter,
        }
    }
}

impl From<&crate::batbelt::miro::layout::ShelfAllocator> for ShelfState {
    fn from(allocator: &crate::batbelt::miro::layout::ShelfAllocator) -> Self {
        Self {
            origin_x: allocator.origin_x,
            origin_y: allocator.origin_y,
            cursor_x: allocator.cursor_x,
            cursor_y: allocator.cursor_y,
            row_height: allocator.row_height,
            row_max_width: allocator.row_max_width,
            gutter: allocator.gutter,
        }
    }
}

/// One entry point's frame, with every item it owns, so a re-deploy can update
/// instead of duplicating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoDeployedFrame {
    pub entry_point: String,
    pub frame_id: String,
    pub frame_url: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// `(graph node id, miro image id)`
    pub images: Vec<(String, String)>,
    pub connector_ids: Vec<String>,
    /// Invisible shapes used as connector endpoints, one per call site.
    #[serde(default)]
    pub marker_ids: Vec<String>,
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

    /// Build metadata from parsed contracts and file-level items.
    /// Preserves existing Miro metadata if present.
    pub fn from_contracts(contracts: Vec<EvmContract>, file_items: Vec<EvmFileItem>) -> Self {
        let mut metadata = Self::default();
        // Preserve existing Miro metadata across sonar re-runs.
        // Extract only the "miro" field from raw JSON to avoid losing frames
        // when other struct fields change between versions.
        if let Ok(content) = fs::read_to_string(EVM_METADATA_FILE) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(miro_val) = json.get("miro") {
                    if let Ok(miro) = serde_json::from_value::<MiroMetadataRef>(miro_val.clone()) {
                        metadata.miro = miro;
                    }
                }
            }
        }
        metadata.file_items = file_items;

        // Per contract: its own storage-variable names (excluding constants /
        // immutables, which don't live in storage) and its base contracts — so a
        // function's writes to inherited state variables resolve too.
        let mut own_state_vars: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut contract_bases: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for c in &contracts {
            let names: Vec<String> = c
                .storage_variables
                .iter()
                .filter(|v| !v.is_constant && !v.is_immutable)
                .map(|v| v.name.clone())
                .collect();
            own_state_vars.insert(c.name.clone(), names);
            contract_bases.insert(c.name.clone(), c.base_contracts.clone());
        }

        // Cross-contract call resolution lookups (Phase 1):
        //  - known contract names (a `Contract.method` call is already resolved),
        //  - impl_map: interface/base name → contracts declaring `is <name>`,
        //  - method_map: method name → concrete contracts that define it,
        //  - own_var_types: contract → (state-var name → declared type).
        let contract_names: std::collections::HashSet<String> =
            contracts.iter().map(|c| c.name.clone()).collect();
        let is_interface: std::collections::HashSet<String> = contracts
            .iter()
            .filter(|c| c.contract_type == EvmContractType::Interface)
            .map(|c| c.name.clone())
            .collect();
        // Vendored (`lib/`) contracts — a call resolving only to these (SafeERC20,
        // mocks, OZ tokens) is not an in-scope storage change worth chasing.
        let external_contracts: std::collections::HashSet<String> = contracts
            .iter()
            .filter(|c| c.external)
            .map(|c| c.name.clone())
            .collect();
        let mut impl_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut method_map: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut own_var_types: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = std::collections::HashMap::new();
        for c in &contracts {
            for b in &c.base_contracts {
                impl_map.entry(b.clone()).or_default().push(c.name.clone());
            }
            if c.contract_type != EvmContractType::Interface {
                for f in &c.functions {
                    method_map
                        .entry(f.name.clone())
                        .or_default()
                        .push(c.name.clone());
                }
            }
            let vt: std::collections::HashMap<String, String> = c
                .storage_variables
                .iter()
                .map(|v| (v.name.clone(), v.type_name.clone()))
                .collect();
            own_var_types.insert(c.name.clone(), vt);
        }

        for contract in &contracts {
            let contract_id = format!("{}_{}", contract.file_path, contract.name);

            // State-variable name → type, visible to this contract (own + inherited).
            let mut var_types: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            {
                let mut seen: std::collections::HashSet<String> =
                    std::collections::HashSet::new();
                let mut stack = vec![contract.name.clone()];
                while let Some(name) = stack.pop() {
                    if !seen.insert(name.clone()) {
                        continue;
                    }
                    if let Some(vt) = own_var_types.get(&name) {
                        for (k, v) in vt {
                            var_types.entry(k.clone()).or_insert_with(|| v.clone());
                        }
                    }
                    if let Some(bases) = contract_bases.get(&name) {
                        stack.extend(bases.iter().cloned());
                    }
                }
            }

            // State variables visible to every function in this contract.
            let mut state_vars: Vec<String> = Vec::new();
            let mut seen_contracts: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            resolve_state_vars(
                &contract.name,
                &own_state_vars,
                &contract_bases,
                &mut seen_contracts,
                &mut state_vars,
            );

            let functions: Vec<FunctionMetadata> = contract
                .functions
                .iter()
                .map(|f| {
                    let func_id = format!("{}_{}_{}", contract.file_path, contract.name, f.name);
                    // Parameters passed as `storage` references (e.g. a library's
                    // `execute(CvammStorage storage $, …)`) are storage pointers too;
                    // pass them so writes through `$` are caught even when body_source
                    // is only the inner statements (no signature to read them from).
                    let storage_params: Vec<String> = f
                        .params
                        .iter()
                        .filter(|p| p.storage_location.as_deref() == Some("storage"))
                        .map(|p| p.name.clone())
                        .collect();
                    let storage_writes =
                        crate::batbelt::evm::parser::call_resolver::extract_storage_writes_from_source(
                            &f.body_source,
                            &state_vars,
                            &storage_params,
                        );
                    let unresolved_calls = compute_unresolved_calls(
                        &f.body_source,
                        &var_types,
                        &contract_names,
                        &is_interface,
                        &external_contracts,
                        &impl_map,
                        &method_map,
                    );
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
                        storage_writes,
                        unresolved_calls,
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
                    storage_writes: func.storage_writes.clone(),
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

/// Collect a contract's effective storage-variable names: its own plus every
/// inherited one, walking base contracts transitively (guarded against cycles).
fn resolve_state_vars(
    name: &str,
    own: &std::collections::HashMap<String, Vec<String>>,
    bases: &std::collections::HashMap<String, Vec<String>>,
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    if !seen.insert(name.to_string()) {
        return;
    }
    if let Some(vars) = own.get(name) {
        out.extend(vars.iter().cloned());
    }
    if let Some(base_names) = bases.get(name) {
        for base in base_names {
            resolve_state_vars(base, own, bases, seen, out);
        }
    }
}

/// Compute the external calls a function makes that static analysis cannot pin to
/// a unique in-scope target — the AI-resolution work-list. A `receiver.method` call
/// is emitted when it is NOT already resolved to a single concrete contract via the
/// receiver's declared type; each carries best-effort candidates.
fn compute_unresolved_calls(
    body: &str,
    var_types: &std::collections::HashMap<String, String>,
    contract_names: &std::collections::HashSet<String>,
    is_interface: &std::collections::HashSet<String>,
    external_contracts: &std::collections::HashSet<String>,
    impl_map: &std::collections::HashMap<String, Vec<String>>,
    method_map: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<UnresolvedCall> {
    let targets =
        crate::batbelt::evm::parser::call_resolver::extract_external_call_targets(body);
    let mut out: Vec<UnresolvedCall> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for (receiver, method) in targets {
        // Internal dispatch, or a call by contract name (already resolved).
        if receiver == "this" || receiver == "super" || contract_names.contains(&receiver) {
            continue;
        }

        // A bare receiver may be a state variable whose declared type we know.
        let inferred_type = if receiver.contains('.') || receiver.contains('[') {
            String::new()
        } else {
            var_types.get(&receiver).cloned().unwrap_or_default()
        };

        // Candidates: implementers of the interface type that define the method,
        // else any in-scope concrete contract defining the method (name-inferred).
        let mut candidates: Vec<String> = Vec::new();
        if !inferred_type.is_empty() {
            if let Some(impls) = impl_map.get(&inferred_type) {
                let with_method: Vec<String> = impls
                    .iter()
                    .filter(|c| {
                        method_map
                            .get(&method)
                            .is_some_and(|v| v.contains(c))
                    })
                    .cloned()
                    .collect();
                candidates = with_method;
            }
        }
        if candidates.is_empty() {
            candidates = method_map.get(&method).cloned().unwrap_or_default();
        }
        candidates.retain(|c| !is_interface.contains(c) && !external_contracts.contains(c));
        candidates.sort();
        candidates.dedup();

        // A single implementer inferred from a REAL type is unambiguous — the deploy
        // graph already follows it; not AI work. Everything else is emitted.
        if candidates.len() == 1 && !inferred_type.is_empty() {
            continue;
        }
        // No plausible in-scope target (builtin / external library) — nothing to resolve.
        if candidates.is_empty() {
            continue;
        }
        if seen.insert((receiver.clone(), method.clone())) {
            out.push(UnresolvedCall {
                receiver,
                method,
                inferred_type,
                candidates,
            });
        }
    }
    out
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
