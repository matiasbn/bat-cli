//! CallResolver: Deterministic resolution of function calls to metadata IDs.
//!
//! # Overview
//!
//! Given a parsed function (`syn::ItemFn`) and the surrounding project context, this
//! module returns a `Vec<ResolvedCall>` — one entry per call site — where each call
//! is either:
//!
//! - `Resolution::Internal(MetadataId)` — resolved to a function in this project's metadata
//! - `Resolution::External(String)` — resolved to a function outside the project (std, anchor, etc.)
//! - `Resolution::Unresolved(String)` — couldn't resolve with certainty
//!
//! # Why "unresolved" instead of "best guess"
//!
//! The previous implementation would fall back to picking the first function with a matching
//! name when resolution failed. In a codebase like Marinade (20+ functions named `process`),
//! this produced garbage dependencies. Here we refuse to guess: if we can't resolve
//! deterministically, we mark as unresolved and log a warning.
//!
//! # Resolution rules
//!
//! - **`foo(args)` (FreeFunction)**: Use the FileScope to find the canonical path of `foo`.
//!   If `foo` is imported (`use some::mod::foo`), look up the function whose metadata path
//!   contains the import module. If `foo` is a local item, look up by name + same file.
//!
//! - **`Type::method(args)` (StaticMethod)**: Resolve `Type` through FileScope to get
//!   its full path. Find the trait_metadata entry where `impl_to == Type` (and ideally
//!   with matching full path). Look up `method` in its `impl_functions`.
//!
//! - **`ctx.accounts.method(args)` (Anchor context method)**: The receiver type is
//!   `Context<T>`'s inner `T`. We know this because `ctx` is a function parameter of
//!   type `Context<T>`, and we track param types. Find the impl block for `T` and
//!   look up `method`.
//!
//! - **`self.method(args)`**: Find which impl block contains the current function
//!   (via trait_metadata), then look up `method` in the same block.
//!
//! - **`var.method(args)` where `var` is a parameter**: Look up `var`'s type in
//!   param_types, find impl block for that type, look up `method`.
//!
//! Anything else is `Unresolved`.

use std::collections::{HashMap, HashSet};

use syn::visit::Visit;

use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::trait_metadata::TraitMetadata;
use crate::batbelt::metadata::MetadataId;
use crate::batbelt::parser::file_scope::FileScope;
use crate::batbelt::parser::type_resolver::{ResolvedType, TypeResolver};

/// Final resolution outcome for a call site.
#[derive(Clone, Debug, PartialEq)]
pub enum Resolution {
    /// Resolved to a function that exists in the project metadata.
    Internal(MetadataId),
    /// Resolved to a function outside the project. The string is an identifier
    /// (e.g., `"Type::method"` or `"free_fn"`).
    External(String),
    /// Could not resolve with certainty. The string is the call site name for logging.
    Unresolved(String),
}

#[derive(Clone, Debug)]
pub struct ResolvedCall {
    pub function_name: String,
    pub resolution: Resolution,
}

/// Methods that are part of the Rust standard library, Anchor framework, or
/// Solana runtime and should not be treated as dependencies.
const FILTERED_METHOD_NAMES: &[&str] = &[
    // std
    "unwrap",
    "expect",
    "clone",
    "to_string",
    "to_owned",
    "iter",
    "into_iter",
    "map",
    "filter",
    "collect",
    "fold",
    "for_each",
    "find",
    "any",
    "all",
    "push",
    "pop",
    "len",
    "is_empty",
    "contains",
    "get",
    "insert",
    "remove",
    "extend",
    "ok_or",
    "ok_or_else",
    "map_err",
    "and_then",
    "or_else",
    "unwrap_or",
    "unwrap_or_else",
    "as_ref",
    "as_mut",
    "borrow",
    "borrow_mut",
    "into",
    "from",
    "try_into",
    "try_from",
    "default",
    "to_vec",
    "as_slice",
    "as_str",
    "is_some",
    "is_none",
    "is_ok",
    "is_err",
    "trim",
    "trim_start",
    "trim_end",
    "split",
    "join",
    "replace",
    "starts_with",
    "ends_with",
    "lines",
    "chars",
    "bytes",
    "next",
    "enumerate",
    "skip",
    "take",
    "zip",
    "chain",
    "flat_map",
    "flatten",
    "filter_map",
    "position",
    "count",
    "sort",
    "sort_by",
    "sort_by_key",
    "dedup",
    // error-stack
    "change_context",
    "attach_printable",
    "report",
    "into_report",
    // Anchor / Solana runtime
    "key",
    "to_account_info",
    "to_accounts",
    "to_account_infos",
    "to_account_metas",
    "load",
    "load_mut",
    "load_init",
    "reload",
    "try_borrow_data",
    "try_borrow_mut_data",
    "lamports",
    "data",
    "data_len",
    "data_is_empty",
    "owner",
    "executable",
    "rent_epoch",
    "program_id",
    "signer",
    "signers",
    "cpi_accounts",
    "cpi_program",
    "set_inner",
    "exit",
    "try_serialize",
    "try_deserialize",
    "try_deserialize_unchecked",
    "try_accounts",
    // arithmetic helpers
    "checked_add",
    "checked_sub",
    "checked_mul",
    "checked_div",
    "checked_rem",
    "saturating_add",
    "saturating_sub",
    "saturating_mul",
    "saturating_div",
    "wrapping_add",
    "wrapping_sub",
    "wrapping_mul",
    "wrapping_div",
    "overflowing_add",
    "overflowing_sub",
    "overflowing_mul",
    // Pubkey and key helpers
    "to_bytes",
    "as_array",
    "find_program_address",
    "create_program_address",
    // number conversions
    "to_le_bytes",
    "to_be_bytes",
    "from_le_bytes",
    "from_be_bytes",
];

/// Free-function names that are macros or intrinsics; not real dependencies.
const FILTERED_FUNCTION_NAMES: &[&str] = &[
    "Ok",
    "Some",
    "Err",
    "None",
    "vec",
    "format",
    "println",
    "eprintln",
    "print",
    "eprint",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "write",
    "writeln",
    "log",
    "cfg",
    "include",
    "include_str",
    "include_bytes",
    "env",
    "option_env",
    "concat",
    "stringify",
    "file",
    "line",
    "column",
    "module_path",
    "Box",
    "Vec",
    "String",
    "Arc",
    "Rc",
    "Mutex",
    "RefCell",
    // Anchor macros / helpers
    "msg",
    "emit",
    "emit_cpi",
    "require",
    "require_eq",
    "require_neq",
    "require_keys_eq",
    "require_keys_neq",
    "require_gt",
    "require_gte",
    "require_lt",
    "require_lte",
    "invoke",
    "invoke_signed",
    "system_program",
];

/// Resolves function calls inside a single function body to precise metadata IDs.
pub struct CallResolver<'a> {
    file_scope: &'a FileScope,
    trait_metadata: &'a [TraitMetadata],
    function_metadata: &'a [FunctionSourceCodeMetadata],
    /// Metadata ID of the function currently being analyzed (for `self.method()` resolution).
    current_function_id: &'a str,
}

impl<'a> CallResolver<'a> {
    pub fn new(
        file_scope: &'a FileScope,
        trait_metadata: &'a [TraitMetadata],
        function_metadata: &'a [FunctionSourceCodeMetadata],
        current_function_id: &'a str,
    ) -> Self {
        Self {
            file_scope,
            trait_metadata,
            function_metadata,
            current_function_id,
        }
    }

    /// Main entry point. Analyzes a parsed function and returns all resolved calls.
    pub fn resolve_function(&self, item_fn: &syn::ItemFn) -> Vec<ResolvedCall> {
        let type_resolver = TypeResolver::new(self.file_scope);

        // Build a map of parameter_name -> ResolvedType for the current function.
        let mut param_types: HashMap<String, ResolvedType> = HashMap::new();
        for input in &item_fn.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let name = pat_ident.ident.to_string();
                    let resolved = type_resolver.resolve(&pat_type.ty);
                    param_types.insert(name, resolved);
                }
            }
        }

        // Also track the `accounts` field of Context parameters: for `ctx: Context<T>`,
        // `ctx.accounts` has type `T`. We store this as a virtual entry so the
        // receiver resolver can look it up.
        let mut context_accounts_types: HashMap<String, ResolvedType> = HashMap::new();
        for input in &item_fn.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let name = pat_ident.ident.to_string();
                    if let Some(inner) = type_resolver.resolve_context_inner(&pat_type.ty) {
                        context_accounts_types.insert(name, inner);
                    }
                }
            }
        }

        let mut visitor = CallVisitor {
            calls: Vec::new(),
            seen: HashSet::new(),
        };
        visitor.visit_block(&item_fn.block);

        visitor
            .calls
            .into_iter()
            .map(|raw| {
                let resolution = self.resolve_call(&raw, &param_types, &context_accounts_types);
                ResolvedCall {
                    function_name: raw.function_name,
                    resolution,
                }
            })
            // Exclude calls that resolve to the current function itself (true recursion).
            // Filtering by name alone is wrong when a same-named function exists in another
            // module (e.g. entrypoint `open` calling `instructions::open`).
            .filter(|rc| match &rc.resolution {
                Resolution::Internal(id) => id != self.current_function_id,
                _ => true,
            })
            .collect()
    }

    fn resolve_call(
        &self,
        raw: &RawCall,
        param_types: &HashMap<String, ResolvedType>,
        context_accounts_types: &HashMap<String, ResolvedType>,
    ) -> Resolution {
        match &raw.call_type {
            RawCallType::FreeFunction => self.resolve_free_function(&raw.function_name),
            RawCallType::StaticMethod { type_name } => {
                self.resolve_static_method(type_name, &raw.function_name)
            }
            RawCallType::MethodCall { receiver } => self.resolve_method_call(
                receiver.as_deref(),
                &raw.function_name,
                param_types,
                context_accounts_types,
            ),
        }
    }

    /// Resolve `foo(args)` — a free-standing function call.
    fn resolve_free_function(&self, name: &str) -> Resolution {
        // Step 1: look up the name in the FileScope
        let candidates_paths = self.file_scope.resolve_name_candidates(name);

        let matching: Vec<_> = self
            .function_metadata
            .iter()
            .filter(|f| f.name == name)
            .collect();

        if matching.is_empty() {
            return Resolution::External(name.to_string());
        }

        // If there's exactly one function with this name in the project, use it.
        if matching.len() == 1 {
            return Resolution::Internal(matching[0].metadata_id.clone());
        }

        // Multiple matches: disambiguate via FileScope's import path.
        for candidate_path in &candidates_paths {
            // Skip "self::name" for glob resolution
            if candidate_path.starts_with("self::") {
                // Same-file: only match candidates whose path equals our file
                if let Some(f) = matching.iter().find(|f| f.path == self.file_scope.path) {
                    return Resolution::Internal(f.metadata_id.clone());
                }
                continue;
            }
            // Convert `crate::instructions::admin::initialize::foo` to a path fragment
            // like `instructions/admin/initialize` for file path matching.
            if let Some(fragment) = path_to_fragment(candidate_path, name) {
                if let Some(f) = matching.iter().find(|f| f.path.contains(&fragment)) {
                    return Resolution::Internal(f.metadata_id.clone());
                }
            }
        }

        log::warn!(
            "CallResolver: unresolved ambiguous free function '{}' from file '{}' ({} candidates)",
            name,
            self.file_scope.path,
            matching.len()
        );
        Resolution::Unresolved(name.to_string())
    }

    /// Resolve `Type::method(args)`.
    fn resolve_static_method(&self, type_name: &str, method: &str) -> Resolution {
        // Try to resolve Type through FileScope first to get the canonical path.
        let _full_type_path = self.file_scope.resolve_name(type_name);

        // First, check if type_name is actually a module name (e.g. `initialize::handler`
        // where `initialize` is a module re-exported via `pub use instructions::*`).
        // In that case, look for a function named `method` whose file path contains
        // the module name as a directory segment (e.g. `instructions/initialize`).
        let module_path_fragment = format!("/{}/", type_name);
        let module_matches: Vec<_> = self
            .function_metadata
            .iter()
            .filter(|f| f.name == method && f.path.contains(&module_path_fragment))
            .collect();
        if module_matches.len() == 1 {
            return Resolution::Internal(module_matches[0].metadata_id.clone());
        }
        // Also try matching with the module name as the last directory before src
        // (e.g. path ending in `type_name.rs`)
        if module_matches.is_empty() {
            let module_file_fragment = format!("/{}.rs", type_name);
            let file_matches: Vec<_> = self
                .function_metadata
                .iter()
                .filter(|f| f.name == method && f.path.contains(&module_file_fragment))
                .collect();
            if file_matches.len() == 1 {
                return Resolution::Internal(file_matches[0].metadata_id.clone());
            }
        }

        // Find trait_metadata entries where impl_to matches the type name.
        let trait_signature = format!("{}::{}", type_name, method);

        let matching_impls: Vec<_> = self
            .trait_metadata
            .iter()
            .filter(|tm| tm.impl_to == type_name)
            .collect();

        // Exact match on trait_signature among impls of this type
        for tm in &matching_impls {
            for impl_fn in &tm.impl_functions {
                if impl_fn.trait_signature == trait_signature {
                    return Resolution::Internal(impl_fn.function_source_code_metadata_id.clone());
                }
            }
        }

        // Not found in impls of this type → external (could be a constructor, std, etc.)
        Resolution::External(trait_signature)
    }

    /// Resolve `receiver.method(args)`.
    fn resolve_method_call(
        &self,
        receiver: Option<&str>,
        method: &str,
        param_types: &HashMap<String, ResolvedType>,
        context_accounts_types: &HashMap<String, ResolvedType>,
    ) -> Resolution {
        let Some(receiver_str) = receiver else {
            return Resolution::Unresolved(method.to_string());
        };

        // Case 1: `self.method()` — find the impl block that contains the current function
        // and look up `method` in the same block.
        if receiver_str == "self" || receiver_str == "self.mut" || receiver_str.starts_with("self.")
        {
            return self.resolve_self_method(method);
        }

        // Case 2: `ctx.accounts.method()` — extract the receiver root (`ctx`) and check
        // if it's a parameter whose type is `Context<T>`. If so, the accounts type is `T`.
        // Then look up `method` in the impl blocks of `T`.
        if let Some(dot_idx) = receiver_str.find('.') {
            let root = &receiver_str[..dot_idx];
            let rest = &receiver_str[dot_idx + 1..];
            if rest == "accounts" {
                if let Some(accounts_type) = context_accounts_types.get(root) {
                    return self.resolve_method_on_type(accounts_type, method);
                }
            }
        }

        // Case 3: `param.method()` — the receiver is a direct parameter reference.
        // Look up the parameter's type and search its impl blocks.
        if let Some(param_type) = param_types.get(receiver_str) {
            return self.resolve_method_on_type(param_type, method);
        }

        // Case 4: unresolved — we don't track local variable types yet.
        // Silently drop method calls that are common std library methods (already filtered
        // at visit time) or unknown.
        Resolution::Unresolved(method.to_string())
    }

    fn resolve_self_method(&self, method: &str) -> Resolution {
        // Find the impl block that contains the current function.
        let self_impl_type = self.trait_metadata.iter().find_map(|tm| {
            let contains = tm
                .impl_functions
                .iter()
                .any(|f| f.function_source_code_metadata_id == self.current_function_id);
            if contains {
                Some(tm.impl_to.clone())
            } else {
                None
            }
        });

        let Some(impl_type) = self_impl_type else {
            return Resolution::Unresolved(format!("self.{}", method));
        };

        let trait_signature = format!("{}::{}", impl_type, method);
        for tm in self.trait_metadata {
            if tm.impl_to == impl_type {
                for f in &tm.impl_functions {
                    if f.trait_signature == trait_signature {
                        return Resolution::Internal(f.function_source_code_metadata_id.clone());
                    }
                }
            }
        }
        Resolution::Unresolved(trait_signature)
    }

    fn resolve_method_on_type(&self, ty: &ResolvedType, method: &str) -> Resolution {
        let Some(type_name) = ty.type_name() else {
            return Resolution::Unresolved(method.to_string());
        };

        let trait_signature = format!("{}::{}", type_name, method);

        for tm in self.trait_metadata {
            if tm.impl_to == type_name {
                for f in &tm.impl_functions {
                    if f.trait_signature == trait_signature {
                        return Resolution::Internal(f.function_source_code_metadata_id.clone());
                    }
                }
            }
        }

        Resolution::External(trait_signature)
    }
}

/// Converts an import path like `crate::instructions::admin::initialize::foo` into
/// a path fragment like `instructions/admin/initialize` suitable for matching against
/// file paths. Returns `None` if the path has no meaningful segments.
fn path_to_fragment(import_path: &str, function_name: &str) -> Option<String> {
    let segments: Vec<&str> = import_path
        .split("::")
        .filter(|s| *s != "crate" && *s != "super" && *s != "self" && *s != function_name)
        .collect();
    if segments.is_empty() {
        None
    } else {
        Some(segments.join("/"))
    }
}

/// Internal representation of a detected call, before resolution.
#[derive(Clone, Debug)]
struct RawCall {
    function_name: String,
    call_type: RawCallType,
}

#[derive(Clone, Debug)]
enum RawCallType {
    FreeFunction,
    StaticMethod { type_name: String },
    MethodCall { receiver: Option<String> },
}

struct CallVisitor {
    calls: Vec<RawCall>,
    seen: HashSet<String>,
}

impl CallVisitor {
    fn record(&mut self, call: RawCall) {
        // Filter noise.
        match &call.call_type {
            RawCallType::FreeFunction | RawCallType::StaticMethod { .. } => {
                if FILTERED_FUNCTION_NAMES.contains(&call.function_name.as_str()) {
                    return;
                }
            }
            RawCallType::MethodCall { .. } => {
                if FILTERED_METHOD_NAMES.contains(&call.function_name.as_str()) {
                    return;
                }
            }
        }
        // Deduplicate by (call_type_kind, function_name, receiver).
        // Using a string key so we keep all distinct call sites but not dupes.
        let key = match &call.call_type {
            RawCallType::FreeFunction => format!("free::{}", call.function_name),
            RawCallType::StaticMethod { type_name } => {
                format!("static::{}::{}", type_name, call.function_name)
            }
            RawCallType::MethodCall { receiver } => format!(
                "method::{}::{}",
                receiver.as_deref().unwrap_or("?"),
                call.function_name
            ),
        };
        if self.seen.insert(key) {
            self.calls.push(call);
        }
    }
}

impl<'ast> Visit<'ast> for CallVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = &*node.func {
            let segments = &expr_path.path.segments;
            let len = segments.len();
            if len == 1 {
                let name = segments[0].ident.to_string();
                self.record(RawCall {
                    function_name: name,
                    call_type: RawCallType::FreeFunction,
                });
            } else if len >= 2 {
                let type_name = segments[len - 2].ident.to_string();
                let func_name = segments[len - 1].ident.to_string();
                self.record(RawCall {
                    function_name: func_name,
                    call_type: RawCallType::StaticMethod { type_name },
                });
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let receiver = receiver_to_string(&node.receiver);
        self.record(RawCall {
            function_name: method,
            call_type: RawCallType::MethodCall { receiver },
        });
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Build a dot-separated string from a receiver expression. For `ctx.accounts` returns
/// `"ctx.accounts"`. Returns `None` for receivers too complex to represent.
fn receiver_to_string(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path_expr) => {
            let s = path_expr
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            Some(s)
        }
        syn::Expr::Field(field_expr) => {
            let member = match &field_expr.member {
                syn::Member::Named(ident) => ident.to_string(),
                syn::Member::Unnamed(idx) => idx.index.to_string(),
            };
            match receiver_to_string(&field_expr.base) {
                Some(base) => Some(format!("{}.{}", base, member)),
                None => Some(member),
            }
        }
        syn::Expr::MethodCall(method_call) => {
            let base = receiver_to_string(&method_call.receiver);
            let method = method_call.method.to_string();
            match base {
                Some(b) => Some(format!("{}.{}()", b, method)),
                None => Some(format!("{}()", method)),
            }
        }
        syn::Expr::Paren(p) => receiver_to_string(&p.expr),
        syn::Expr::Reference(r) => receiver_to_string(&r.expr),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_fragment_simple() {
        assert_eq!(
            path_to_fragment("crate::instructions::admin::initialize::process", "process"),
            Some("instructions/admin/initialize".to_string())
        );
    }

    #[test]
    fn test_path_fragment_external() {
        assert_eq!(
            path_to_fragment("anchor_lang::prelude::Pubkey", "Pubkey"),
            Some("anchor_lang/prelude".to_string())
        );
    }

    #[test]
    fn test_path_fragment_empty() {
        assert_eq!(path_to_fragment("crate::foo", "foo"), None);
    }

    #[test]
    fn test_filter_filtered_function_name() {
        let mut visitor = CallVisitor {
            calls: vec![],
            seen: HashSet::new(),
        };
        visitor.record(RawCall {
            function_name: "Ok".to_string(),
            call_type: RawCallType::FreeFunction,
        });
        visitor.record(RawCall {
            function_name: "real_fn".to_string(),
            call_type: RawCallType::FreeFunction,
        });
        assert_eq!(visitor.calls.len(), 1);
        assert_eq!(visitor.calls[0].function_name, "real_fn");
    }

    #[test]
    fn test_filter_filtered_method_name() {
        let mut visitor = CallVisitor {
            calls: vec![],
            seen: HashSet::new(),
        };
        visitor.record(RawCall {
            function_name: "key".to_string(),
            call_type: RawCallType::MethodCall {
                receiver: Some("account".to_string()),
            },
        });
        visitor.record(RawCall {
            function_name: "process".to_string(),
            call_type: RawCallType::MethodCall {
                receiver: Some("ctx.accounts".to_string()),
            },
        });
        assert_eq!(visitor.calls.len(), 1);
        assert_eq!(visitor.calls[0].function_name, "process");
    }

    #[test]
    fn test_deduplication() {
        let mut visitor = CallVisitor {
            calls: vec![],
            seen: HashSet::new(),
        };
        let raw = RawCall {
            function_name: "foo".to_string(),
            call_type: RawCallType::FreeFunction,
        };
        visitor.record(raw.clone());
        visitor.record(raw.clone());
        visitor.record(raw);
        assert_eq!(visitor.calls.len(), 1);
    }

    #[test]
    fn test_receiver_to_string_path() {
        let expr: syn::Expr = syn::parse_str("foo").unwrap();
        assert_eq!(receiver_to_string(&expr), Some("foo".to_string()));
    }

    #[test]
    fn test_receiver_to_string_nested_field() {
        let expr: syn::Expr = syn::parse_str("ctx.accounts").unwrap();
        assert_eq!(receiver_to_string(&expr), Some("ctx.accounts".to_string()));
    }

    #[test]
    fn test_receiver_to_string_self() {
        let expr: syn::Expr = syn::parse_str("self").unwrap();
        assert_eq!(receiver_to_string(&expr), Some("self".to_string()));
    }

    #[test]
    fn test_receiver_to_string_self_field() {
        let expr: syn::Expr = syn::parse_str("self.state").unwrap();
        assert_eq!(receiver_to_string(&expr), Some("self.state".to_string()));
    }
}
