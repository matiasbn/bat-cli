//! FileScope: Per-file import and item resolution.
//!
//! This module builds a map of short names (as used in code) to their full canonical
//! paths based on the `use` statements in a file. It also tracks items (structs, enums,
//! functions, impls) defined locally in the file.
//!
//! # Why this exists
//!
//! When resolving `Context<Initialize>` in a function, we need to know which `Initialize`
//! the code refers to. Without a scope, we can only guess by name — but multiple crates
//! may define types with the same name. `FileScope::resolve_name("Initialize")` returns
//! the full path by inspecting `use crate::instructions::admin::initialize::Initialize`.
//!
//! # Design
//!
//! - `name_to_path`: short name -> full module path (e.g. `"Initialize"` -> `"crate::instructions::admin::initialize::Initialize"`)
//! - `glob_imports`: modules imported via `use foo::*`; used as fallback for unresolved names
//! - `local_items`: items defined in the file itself (they don't need import resolution)
//!
//! The builder walks `syn::UseTree` recursively to handle nested and grouped imports.

use std::collections::{HashMap, HashSet};
use syn::{Item, UseTree};

#[derive(Clone, Debug, Default)]
pub struct FileScope {
    /// Absolute or relative path to the file this scope was built from.
    pub path: String,
    /// Maps a short name (as used in code) to a canonical `::`-separated path.
    ///
    /// Examples:
    /// - `use crate::state::State;` → `"State"` -> `"crate::state::State"`
    /// - `use anchor_lang::prelude::*;` → entry goes to `glob_imports` instead
    /// - `use foo::{Bar, Baz as Qux};` → `"Bar"` -> `"foo::Bar"`, `"Qux"` -> `"foo::Baz"`
    pub name_to_path: HashMap<String, String>,
    /// Modules imported with a trailing `*`. Cannot resolve specific names,
    /// but useful as a fallback when a name isn't in `name_to_path`.
    pub glob_imports: Vec<String>,
    /// Items defined in this file (structs, enums, functions, traits, mods, type aliases).
    pub local_items: HashSet<String>,
}

impl FileScope {
    /// Build a `FileScope` from the already-parsed syn AST of a file.
    pub fn from_syn_file(path: impl Into<String>, syn_file: &syn::File) -> Self {
        let mut scope = Self {
            path: path.into(),
            ..Default::default()
        };
        scope.visit_items(&syn_file.items);
        scope
    }

    /// Build a `FileScope` from file contents (parses with `syn::parse_file`).
    pub fn from_file_content(
        path: impl Into<String>,
        content: &str,
    ) -> Result<Self, syn::Error> {
        let syn_file = syn::parse_file(content)?;
        Ok(Self::from_syn_file(path, &syn_file))
    }

    /// Resolve a short name to its full path. Priority order:
    /// 1. Explicit imports (`name_to_path`)
    /// 2. Local items (prefixed with `crate::<file>::`)
    /// 3. `None` (may be from a glob import or unknown)
    ///
    /// Glob imports are returned separately via `resolve_name_candidates`.
    pub fn resolve_name(&self, name: &str) -> Option<String> {
        if let Some(path) = self.name_to_path.get(name) {
            return Some(path.clone());
        }
        if self.local_items.contains(name) {
            // Local item: we don't know the full module path from a file alone,
            // but we can mark it as "self::<name>" to indicate same-file origin.
            return Some(format!("self::{}", name));
        }
        None
    }

    /// Returns all possible paths a name could resolve to, including glob-imported modules.
    /// If `name_to_path` has a direct match, only that is returned. Otherwise, returns
    /// one candidate per glob-imported module (prefixing the name).
    pub fn resolve_name_candidates(&self, name: &str) -> Vec<String> {
        if let Some(path) = self.name_to_path.get(name) {
            return vec![path.clone()];
        }
        if self.local_items.contains(name) {
            return vec![format!("self::{}", name)];
        }
        self.glob_imports
            .iter()
            .map(|m| format!("{}::{}", m, name))
            .collect()
    }

    fn visit_items(&mut self, items: &[Item]) {
        for item in items {
            match item {
                Item::Use(item_use) => {
                    self.collect_use_tree(&item_use.tree, "");
                }
                Item::Struct(s) => {
                    self.local_items.insert(s.ident.to_string());
                }
                Item::Enum(e) => {
                    self.local_items.insert(e.ident.to_string());
                }
                Item::Fn(f) => {
                    self.local_items.insert(f.sig.ident.to_string());
                }
                Item::Trait(t) => {
                    self.local_items.insert(t.ident.to_string());
                }
                Item::Type(ty) => {
                    self.local_items.insert(ty.ident.to_string());
                }
                Item::Const(c) => {
                    self.local_items.insert(c.ident.to_string());
                }
                Item::Static(s) => {
                    self.local_items.insert(s.ident.to_string());
                }
                Item::Mod(m) => {
                    self.local_items.insert(m.ident.to_string());
                    // Recurse into inline modules to collect their use statements too.
                    // This is important for Anchor's `#[program] pub mod foo { ... }`.
                    if let Some((_, ref sub_items)) = m.content {
                        self.visit_items(sub_items);
                    }
                }
                Item::Impl(impl_block) => {
                    // The `impl Foo { ... }` block doesn't introduce a new name,
                    // but its methods are local. We don't track methods individually
                    // (that's the job of trait_metadata), but we note that `Foo`
                    // has local impls. This is informational only.
                    if let syn::Type::Path(type_path) = &*impl_block.self_ty {
                        if let Some(last) = type_path.path.segments.last() {
                            self.local_items.insert(last.ident.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively walks a `UseTree`, building the full path as it descends and
    /// recording each leaf (name, rename, or glob) in the scope.
    fn collect_use_tree(&mut self, tree: &UseTree, prefix: &str) {
        match tree {
            UseTree::Path(use_path) => {
                let ident = use_path.ident.to_string();
                let new_prefix = if prefix.is_empty() {
                    ident
                } else {
                    format!("{}::{}", prefix, ident)
                };
                self.collect_use_tree(&use_path.tree, &new_prefix);
            }
            UseTree::Name(use_name) => {
                let name = use_name.ident.to_string();
                let full_path = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                self.name_to_path.insert(name, full_path);
            }
            UseTree::Rename(use_rename) => {
                let original = use_rename.ident.to_string();
                let alias = use_rename.rename.to_string();
                let full_path = if prefix.is_empty() {
                    original
                } else {
                    format!("{}::{}", prefix, original)
                };
                // The alias is what appears in code, so that's the key.
                self.name_to_path.insert(alias, full_path);
            }
            UseTree::Glob(_) => {
                if !prefix.is_empty() {
                    self.glob_imports.push(prefix.to_string());
                }
            }
            UseTree::Group(use_group) => {
                for sub_tree in &use_group.items {
                    self.collect_use_tree(sub_tree, prefix);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(content: &str) -> FileScope {
        FileScope::from_file_content("test.rs", content).unwrap()
    }

    #[test]
    fn test_simple_use() {
        let scope = build("use crate::state::State;");
        assert_eq!(
            scope.resolve_name("State"),
            Some("crate::state::State".to_string())
        );
    }

    #[test]
    fn test_grouped_use() {
        let scope = build("use foo::{Bar, Baz};");
        assert_eq!(scope.resolve_name("Bar"), Some("foo::Bar".to_string()));
        assert_eq!(scope.resolve_name("Baz"), Some("foo::Baz".to_string()));
    }

    #[test]
    fn test_rename_use() {
        let scope = build("use foo::Bar as Qux;");
        assert_eq!(scope.resolve_name("Qux"), Some("foo::Bar".to_string()));
        assert_eq!(scope.resolve_name("Bar"), None);
    }

    #[test]
    fn test_glob_use() {
        let scope = build("use anchor_lang::prelude::*;");
        assert_eq!(scope.glob_imports, vec!["anchor_lang::prelude".to_string()]);
        let candidates = scope.resolve_name_candidates("Pubkey");
        assert_eq!(candidates, vec!["anchor_lang::prelude::Pubkey".to_string()]);
    }

    #[test]
    fn test_nested_grouped() {
        let scope = build(
            "use foo::{
                bar::{Baz, Qux},
                quux::Corge as Grault,
            };",
        );
        assert_eq!(scope.resolve_name("Baz"), Some("foo::bar::Baz".to_string()));
        assert_eq!(scope.resolve_name("Qux"), Some("foo::bar::Qux".to_string()));
        assert_eq!(
            scope.resolve_name("Grault"),
            Some("foo::quux::Corge".to_string())
        );
    }

    #[test]
    fn test_local_struct() {
        let scope = build("pub struct Initialize<'info> { pub state: String }");
        assert!(scope.local_items.contains("Initialize"));
        assert_eq!(
            scope.resolve_name("Initialize"),
            Some("self::Initialize".to_string())
        );
    }

    #[test]
    fn test_inline_module() {
        let scope = build(
            r#"
            pub mod inner {
                use crate::foo::Bar;
            }
            "#,
        );
        assert_eq!(scope.resolve_name("Bar"), Some("crate::foo::Bar".to_string()));
        assert!(scope.local_items.contains("inner"));
    }

    #[test]
    fn test_anchor_program_pattern() {
        // The realistic Marinade pattern: imports + #[program] mod with nested use
        let scope = build(
            r#"
            use crate::state::State;
            use anchor_lang::prelude::*;

            #[program]
            pub mod marinade_finance {
                use super::*;
                pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
            }
            "#,
        );
        assert_eq!(
            scope.resolve_name("State"),
            Some("crate::state::State".to_string())
        );
        assert!(scope.glob_imports.contains(&"anchor_lang::prelude".to_string()));
    }
}
