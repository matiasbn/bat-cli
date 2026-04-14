//! TypeResolver: Resolve `syn::Type` to a canonical full path using a `FileScope`.
//!
//! # What it does
//!
//! Given a syn::Type like `Context<Initialize>` or `Box<Account<'info, State>>`, this
//! module strips wrappers (references, Box, Arc, Rc, RefCell, Option, etc.) and resolves
//! the innermost type name through a `FileScope`'s import map.
//!
//! # Why
//!
//! The previous implementation used string manipulation (`contains("Context<")`) which
//! was fragile and couldn't disambiguate types with the same name in different modules.
//! This resolver walks the AST directly and uses real import information.
//!
//! # Special case: Context<T>
//!
//! Anchor's `Context<T>` has an `accounts: T` field. So `ctx.accounts` has type `T`.
//! The method `resolve_context_inner` extracts that inner type, which is how we
//! resolve `ctx.accounts.process()` to the correct `impl T { fn process() }` block.

use crate::batbelt::parser::file_scope::FileScope;
use quote::ToTokens;

/// A type resolved (or attempted to be resolved) to a canonical path.
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedType {
    /// Resolved to a known type with a canonical path.
    /// The `type_name` is the short name; `full_path` is what the FileScope returned.
    Known {
        type_name: String,
        full_path: String,
    },
    /// The type name was extracted but couldn't be resolved through the FileScope.
    /// It may still be usable by falling back to glob imports or by matching names
    /// across the project.
    NameOnly { type_name: String },
    /// The type couldn't be extracted at all (e.g., `impl Trait`, function pointer,
    /// tuple, slice, etc.).
    Unknown,
}

impl ResolvedType {
    pub fn type_name(&self) -> Option<&str> {
        match self {
            ResolvedType::Known { type_name, .. } => Some(type_name),
            ResolvedType::NameOnly { type_name } => Some(type_name),
            ResolvedType::Unknown => None,
        }
    }

    pub fn full_path(&self) -> Option<&str> {
        match self {
            ResolvedType::Known { full_path, .. } => Some(full_path),
            _ => None,
        }
    }
}

pub struct TypeResolver<'a> {
    file_scope: &'a FileScope,
}

impl<'a> TypeResolver<'a> {
    pub fn new(file_scope: &'a FileScope) -> Self {
        Self { file_scope }
    }

    /// Resolve a syn::Type to a canonical path. Strips wrappers recursively
    /// (`&T`, `&mut T`, `Box<T>`, `Arc<T>`, etc.) until it hits a concrete type,
    /// then resolves through the FileScope.
    pub fn resolve(&self, ty: &syn::Type) -> ResolvedType {
        let inner = Self::strip_wrappers(ty);
        self.resolve_direct(inner)
    }

    /// Same as `resolve` but treats `Context<T>` specially: returns the type of `T`
    /// (the accounts struct) instead of `Context` itself.
    ///
    /// This is the workhorse for resolving `ctx.accounts.method()` calls in Anchor.
    pub fn resolve_context_inner(&self, ty: &syn::Type) -> Option<ResolvedType> {
        let inner = Self::strip_wrappers(ty);
        if let syn::Type::Path(type_path) = inner {
            let segments = &type_path.path.segments;
            if let Some(last) = segments.last() {
                if last.ident == "Context" {
                    // Extract the generic argument: Context<T> or Context<'info, T>
                    if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                        for arg in args.args.iter().rev() {
                            if let syn::GenericArgument::Type(inner_ty) = arg {
                                return Some(self.resolve(inner_ty));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Resolve a direct (unwrapped) type path to a canonical path.
    fn resolve_direct(&self, ty: &syn::Type) -> ResolvedType {
        match ty {
            syn::Type::Path(type_path) => {
                let segments = &type_path.path.segments;
                if segments.is_empty() {
                    return ResolvedType::Unknown;
                }
                // If the path has multiple segments, the first one should be
                // resolved and the rest appended. E.g., `module::Type` where
                // `module` is imported.
                if segments.len() == 1 {
                    let name = segments[0].ident.to_string();
                    match self.file_scope.resolve_name(&name) {
                        Some(full_path) => ResolvedType::Known {
                            type_name: name,
                            full_path,
                        },
                        None => ResolvedType::NameOnly { type_name: name },
                    }
                } else {
                    // Multi-segment path: prefer the last segment as the type name.
                    let last_name = segments.last().unwrap().ident.to_string();
                    let first_name = segments[0].ident.to_string();
                    let rest: Vec<String> = segments
                        .iter()
                        .skip(1)
                        .map(|s| s.ident.to_string())
                        .collect();
                    // Try to resolve the first segment through the scope
                    let full_path = match self.file_scope.resolve_name(&first_name) {
                        Some(base) => format!("{}::{}", base, rest.join("::")),
                        None => type_path
                            .path
                            .to_token_stream()
                            .to_string()
                            .replace(' ', ""),
                    };
                    ResolvedType::Known {
                        type_name: last_name,
                        full_path,
                    }
                }
            }
            _ => ResolvedType::Unknown,
        }
    }

    /// Recursively strip wrapper types to get at the inner concrete type.
    /// Handles: `&T`, `&mut T`, `Box<T>`, `Arc<T>`, `Rc<T>`, `RefCell<T>`, `Cell<T>`,
    /// `Mutex<T>`, `Option<T>`, `Account<'info, T>`, `AccountLoader<'info, T>`, etc.
    ///
    /// For Anchor wrappers like `Account<'info, T>`, we return `T`.
    fn strip_wrappers(ty: &syn::Type) -> &syn::Type {
        match ty {
            syn::Type::Reference(type_ref) => Self::strip_wrappers(&type_ref.elem),
            syn::Type::Paren(type_paren) => Self::strip_wrappers(&type_paren.elem),
            syn::Type::Group(type_group) => Self::strip_wrappers(&type_group.elem),
            syn::Type::Path(type_path) => {
                // If this is a single-segment generic wrapper we recognize, recurse into it.
                if let Some(last) = type_path.path.segments.last() {
                    let name = last.ident.to_string();
                    const WRAPPERS: &[&str] = &[
                        "Box",
                        "Arc",
                        "Rc",
                        "RefCell",
                        "Cell",
                        "Mutex",
                        "RwLock",
                        "Option",
                        "Result",
                        "Account",
                        "AccountLoader",
                        "UncheckedAccount",
                        "Signer",
                        "SystemAccount",
                        "Interface",
                        "InterfaceAccount",
                        "Sysvar",
                    ];
                    if WRAPPERS.contains(&name.as_str()) {
                        if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                            // Find the first Type argument (skip lifetimes).
                            for arg in &args.args {
                                if let syn::GenericArgument::Type(inner) = arg {
                                    return Self::strip_wrappers(inner);
                                }
                            }
                        }
                    }
                }
                ty
            }
            _ => ty,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batbelt::parser::file_scope::FileScope;

    fn scope_with(use_stmts: &str) -> FileScope {
        FileScope::from_file_content("test.rs", use_stmts).unwrap()
    }

    fn parse_type(src: &str) -> syn::Type {
        syn::parse_str::<syn::Type>(src).unwrap()
    }

    #[test]
    fn test_resolve_simple_type() {
        let scope = scope_with("use crate::state::State;");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("State");
        let resolved = resolver.resolve(&ty);
        assert_eq!(
            resolved,
            ResolvedType::Known {
                type_name: "State".to_string(),
                full_path: "crate::state::State".to_string(),
            }
        );
    }

    #[test]
    fn test_resolve_through_reference() {
        let scope = scope_with("use crate::state::State;");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("&State");
        assert_eq!(
            resolver.resolve(&ty).full_path(),
            Some("crate::state::State")
        );
    }

    #[test]
    fn test_resolve_through_box() {
        let scope = scope_with("use crate::state::State;");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("Box<Account<'info, State>>");
        assert_eq!(
            resolver.resolve(&ty).full_path(),
            Some("crate::state::State")
        );
    }

    #[test]
    fn test_resolve_context_inner() {
        let scope = scope_with("use crate::instructions::admin::initialize::Initialize;");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("Context<Initialize>");
        let inner = resolver.resolve_context_inner(&ty).unwrap();
        assert_eq!(
            inner.full_path(),
            Some("crate::instructions::admin::initialize::Initialize")
        );
    }

    #[test]
    fn test_resolve_context_with_lifetime() {
        let scope = scope_with("use crate::instructions::admin::initialize::Initialize;");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("Context<'info, Initialize>");
        let inner = resolver.resolve_context_inner(&ty).unwrap();
        assert_eq!(
            inner.full_path(),
            Some("crate::instructions::admin::initialize::Initialize")
        );
    }

    #[test]
    fn test_resolve_unknown_type_name_only() {
        let scope = scope_with("");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("SomeType");
        assert!(matches!(
            resolver.resolve(&ty),
            ResolvedType::NameOnly { .. }
        ));
    }

    #[test]
    fn test_not_context_returns_none() {
        let scope = scope_with("");
        let resolver = TypeResolver::new(&scope);
        let ty = parse_type("String");
        assert!(resolver.resolve_context_inner(&ty).is_none());
    }
}
