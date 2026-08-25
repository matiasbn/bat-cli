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
    /// AI-supplied resolutions for the runtime-dynamic interface→implementation
    /// bindings that static analysis cannot pin (see `unresolved_calls`): interface
    /// type name → the concrete in-scope contract it points to in this deployment.
    /// The deploy graph follows these to reach downstream storage writers. Preserved
    /// across `sonar` regeneration (like `miro`); written by `bat-cli resolve`.
    #[serde(default)]
    pub resolutions: std::collections::HashMap<String, String>,
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
                // Preserve AI-supplied interface resolutions across regeneration.
                if let Some(res_val) = json.get("resolutions") {
                    if let Ok(res) = serde_json::from_value::<
                        std::collections::HashMap<String, String>,
                    >(res_val.clone())
                    {
                        metadata.resolutions = res;
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

        // struct name → (field name → declared type), for typing `$.field` receivers.
        let mut struct_fields: std::collections::HashMap<
            String,
            std::collections::HashMap<String, String>,
        > = std::collections::HashMap::new();
        for c in &contracts {
            for s in &c.structs {
                let fields: std::collections::HashMap<String, String> = s
                    .fields
                    .iter()
                    .map(|f| (f.name.clone(), f.type_name.clone()))
                    .collect();
                struct_fields.insert(s.name.clone(), fields);
            }
        }

        // Callee graph, built from the same single parse per function (no separate pass).
        let mut all_deps: Vec<FunctionDependency> = Vec::new();

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

            let mut functions: Vec<FunctionMetadata> = Vec::new();
            for f in &contract.functions {
                let func_id = format!("{}_{}_{}", contract.file_path, contract.name, f.name);
                // Parameters passed as `storage` references (e.g. a library's
                // `execute(CvammStorage storage $, …)`) are storage pointers too.
                let storage_params: Vec<String> = f
                    .params
                    .iter()
                    .filter(|p| p.storage_location.as_deref() == Some("storage"))
                    .map(|p| p.name.clone())
                    .collect();
                // ONE parse per function: storage writes, external call targets,
                // local/param types AND callee names, all from a single AST walk.
                let analysis = crate::batbelt::evm::parser::call_resolver::analyze_body(
                    &f.body_source,
                    &state_vars,
                    &storage_params,
                );
                let storage_writes = analysis.storage_writes;
                // Receiver types visible here: state vars (inherited) + params + locals.
                let mut local_var_types = var_types.clone();
                for (n, t) in &analysis.local_types {
                    local_var_types.insert(n.clone(), t.clone());
                }
                let unresolved_calls = compute_unresolved_calls(
                    &analysis.call_targets,
                    &local_var_types,
                    &struct_fields,
                    &contract_names,
                    &is_interface,
                    &external_contracts,
                    &impl_map,
                    &method_map,
                );
                all_deps.push(FunctionDependency {
                    function_metadata_id: func_id.clone(),
                    callees: analysis.call_names,
                });
                functions.push(FunctionMetadata {
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
                });
            }

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
        metadata.function_dependencies = all_deps;

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

/// Drop the "noise" from every function's `unresolved_calls`: an interface call is
/// only worth resolving if it can REACH a storage write. We taint every function
/// that can reach a write — over the static call graph PLUS the unresolved→candidate
/// edges, seeded by the direct writers — then keep an unresolved call only when one
/// of its candidates is tainted. So `stable.balanceOf` (a read whose candidates never
/// mutate) is dropped, while `$.borrowerOps.adjustPosition` (which reaches a write
/// several hops down) is kept.
pub fn prune_unresolved_noise(metadata: &mut EvmBatMetadata) {
    use std::collections::{HashMap, HashSet};
    // Lookups from the metadata (no re-parsing: reuse `function_dependencies`).
    let contract_names: HashSet<String> =
        metadata.contracts.iter().map(|c| c.name.clone()).collect();
    let contract_file: HashMap<String, String> = metadata
        .contracts
        .iter()
        .map(|c| (c.name.clone(), c.file_path.clone()))
        .collect();
    let mut method_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut bases: HashMap<String, Vec<String>> = HashMap::new();
    let mut caller_contract: HashMap<String, String> = HashMap::new();
    for c in &metadata.contracts {
        bases.insert(c.name.clone(), c.base_contracts.clone());
        for f in &c.functions {
            method_map
                .entry(f.name.clone())
                .or_default()
                .push(c.name.clone());
            caller_contract.insert(f.metadata_id.clone(), c.name.clone());
        }
    }
    let fid = |contract: &str, method: &str| -> Option<String> {
        contract_file
            .get(contract)
            .map(|fp| format!("{fp}_{contract}_{method}"))
    };

    // Edges: static callees (from function_dependencies) + unresolved candidates.
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for dep in &metadata.function_dependencies {
        let Some(cname) = caller_contract.get(&dep.function_metadata_id) else {
            continue;
        };
        let out = edges.entry(dep.function_metadata_id.clone()).or_default();
        for callee in &dep.callees {
            if let Some((tgt, method)) = callee.split_once('.') {
                if contract_names.contains(tgt) {
                    if let Some(id) = fid(tgt, method) {
                        out.push(id);
                    }
                }
                // `var.method` (interface) flows through unresolved candidates below.
            } else {
                // Internal call: the caller's contract or a base that defines it.
                let mut chain = vec![cname.clone()];
                if let Some(bs) = bases.get(cname) {
                    chain.extend(bs.iter().cloned());
                }
                for cand in chain {
                    if method_map.get(callee).is_some_and(|v| v.contains(&cand)) {
                        if let Some(id) = fid(&cand, callee) {
                            out.push(id);
                        }
                        break;
                    }
                }
            }
        }
    }
    // Seed = direct writers; add the unresolved→candidate edges.
    let mut taint: HashSet<String> = HashSet::new();
    for c in &metadata.contracts {
        for f in &c.functions {
            if !f.storage_writes.is_empty() {
                taint.insert(f.metadata_id.clone());
            }
            let out = edges.entry(f.metadata_id.clone()).or_default();
            for u in &f.unresolved_calls {
                for cand in &u.candidates {
                    if let Some(id) = fid(cand, &u.method) {
                        out.push(id);
                    }
                }
            }
        }
    }

    // Fixpoint by worklist over reverse edges (a caller taints when a callee does).
    let mut rev: HashMap<String, Vec<String>> = HashMap::new();
    for (caller, outs) in &edges {
        for o in outs {
            rev.entry(o.clone()).or_default().push(caller.clone());
        }
    }
    let mut work: Vec<String> = taint.iter().cloned().collect();
    while let Some(t) = work.pop() {
        if let Some(callers) = rev.get(&t) {
            for caller in callers.clone() {
                if taint.insert(caller.clone()) {
                    work.push(caller);
                }
            }
        }
    }

    // Keep an unresolved call only if a candidate can reach a write.
    for c in &mut metadata.contracts {
        for f in &mut c.functions {
            if f.unresolved_calls.is_empty() {
                continue;
            }
            f.unresolved_calls.retain(|u| {
                u.candidates
                    .iter()
                    .any(|cand| fid(cand, &u.method).is_some_and(|id| taint.contains(&id)))
            });
        }
    }
}

/// Compute the external calls a function makes that static analysis cannot pin to
/// a unique in-scope target — the AI-resolution work-list. A `receiver.method` call
/// is emitted when it is NOT already resolved to a single concrete contract via the
/// receiver's declared type; each carries best-effort candidates.
#[allow(clippy::too_many_arguments)]
fn compute_unresolved_calls(
    targets: &[(String, String)],
    var_types: &std::collections::HashMap<String, String>,
    struct_fields: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
    contract_names: &std::collections::HashSet<String>,
    is_interface: &std::collections::HashSet<String>,
    external_contracts: &std::collections::HashSet<String>,
    impl_map: &std::collections::HashMap<String, Vec<String>>,
    method_map: &std::collections::HashMap<String, Vec<String>>,
) -> Vec<UnresolvedCall> {
    let mut out: Vec<UnresolvedCall> = Vec::new();
    let mut seen: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for (receiver, method) in targets {
        let (receiver, method) = (receiver.clone(), method.clone());
        // Internal dispatch, or a call by contract name (already resolved).
        if receiver == "this" || receiver == "super" || contract_names.contains(&receiver) {
            continue;
        }

        let inferred_type = receiver_type(&receiver, var_types, struct_fields);

        // Implementers of the receiver's declared interface type that define the
        // method — a STATIC, type-proven resolution.
        let typed_impls: Vec<String> = if inferred_type.is_empty() {
            Vec::new()
        } else {
            impl_map
                .get(&inferred_type)
                .map(|impls| {
                    impls
                        .iter()
                        .filter(|c| {
                            !is_interface.contains(*c)
                                && !external_contracts.contains(*c)
                                && method_map.get(&method).is_some_and(|v| v.contains(c))
                        })
                        .cloned()
                        .collect()
                })
                .unwrap_or_default()
        };
        // Exactly one type-proven implementer → unambiguous; the deploy graph follows
        // it. Not AI work.
        if typed_impls.len() == 1 {
            continue;
        }

        // Otherwise: candidates are the type-proven impls if any, else any in-scope
        // concrete contract defining the method (name-inferred, for the AI to confirm).
        let mut candidates: Vec<String> = if !typed_impls.is_empty() {
            typed_impls
        } else {
            method_map.get(&method).cloned().unwrap_or_default()
        };
        candidates.retain(|c| !is_interface.contains(c) && !external_contracts.contains(c));
        candidates.sort();
        candidates.dedup();

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

/// The declared type of a call receiver expression: a bare variable (`positionManager`)
/// via `var_types`, or a one-level struct-pointer field (`$.borrowerOps`) via the base
/// variable's struct type and its field types. Empty when it can't be typed.
fn receiver_type(
    receiver: &str,
    var_types: &std::collections::HashMap<String, String>,
    struct_fields: &std::collections::HashMap<String, std::collections::HashMap<String, String>>,
) -> String {
    if let Some((base, field)) = receiver.split_once('.') {
        let field = field.trim_end_matches("[]");
        // Only a single field hop is resolved (covers `$.borrowerOps`); deeper chains
        // stay untyped.
        if field.contains('.') || field.contains('[') {
            return String::new();
        }
        if let Some(base_type) = var_types.get(base) {
            if let Some(fields) = struct_fields.get(base_type) {
                if let Some(field_type) = fields.get(field) {
                    return field_type.clone();
                }
            }
        }
        return String::new();
    }
    if receiver.contains('[') {
        return String::new();
    }
    // A cast receiver `IFace(addr)` renders as `IFace()` — its type is the cast
    // target. Gate on a PascalCase base so a plain function call (`_s()`) isn't
    // mistaken for a type.
    if let Some(base) = receiver.strip_suffix("()") {
        if base
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_uppercase())
            && base.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return base.to_string();
        }
        return String::new();
    }
    var_types.get(receiver).cloned().unwrap_or_default()
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
