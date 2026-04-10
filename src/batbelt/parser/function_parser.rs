use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;

use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, MetadataId};

use crate::batbelt::parser::syn_function_dependency_parser::{self, CallType};

/// Normalizes spaces around angle brackets in generic types.
/// syn's `to_token_stream()` produces `Context < Initialize >` but
/// source code has `Context<Initialize>`. This normalizes to the compact form.
pub fn normalize_generic_type(ty: &str) -> String {
    ty.replace(" < ", "<")
        .replace("< ", "<")
        .replace(" <", "<")
        .replace(" > ", ">")
        .replace("> ", ">")
        .replace(" >", ">")
        .replace(" , ", ", ")
}
use crate::batbelt::parser::ParserError;

use crate::batbelt::metadata::function_dependencies_metadata::{
    FunctionDependenciesMetadata, FunctionDependencyInfo,
};

use error_stack::{Report, Result, ResultExt};

#[derive(Clone, Debug)]
pub struct FunctionDependencyParser {
    pub is_external: bool,
    pub function_name: String,
    pub dependency_metadata_matches: Vec<MetadataId>,
}

#[derive(Clone, Debug)]
pub struct FunctionParameterParser {
    pub parameter_name: String,
    pub parameter_type: String,
}

#[derive(Clone, Debug)]
pub struct FunctionParser {
    pub name: String,
    pub function_metadata: FunctionSourceCodeMetadata,
    pub content: String,
    pub signature: String,
    pub body: String,
    pub parameters: Vec<FunctionParameterParser>,
    pub dependencies: Vec<MetadataId>,
    pub external_dependencies: Vec<String>,
}

impl FunctionParser {
    fn new(
        name: String,
        function_metadata: FunctionSourceCodeMetadata,
        content: String,
    ) -> Result<Self, ParserError> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let mut new_function_parser = Self {
            name,
            function_metadata: function_metadata.clone(),
            content,
            signature: "".to_string(),
            body: "".to_string(),
            parameters: vec![],
            dependencies: vec![],
            external_dependencies: vec![],
        };
        new_function_parser.get_function_signature();
        new_function_parser.get_function_body();
        new_function_parser.get_function_parameters()?;
        log::debug!("new_function_parser:\n{:#?}", new_function_parser);
        log::debug!("new_function_body:\n{}", new_function_parser.body);
        match bat_metadata.get_functions_dependencies_metadata_by_function_metadata_id(
            function_metadata.metadata_id,
        ) {
            Ok(function_dep_metadata) => {
                new_function_parser.dependencies = function_dep_metadata
                    .clone()
                    .dependencies
                    .into_iter()
                    .map(|func_dep| func_dep.function_metadata_id)
                    .collect();
                new_function_parser.external_dependencies =
                    function_dep_metadata.external_dependencies;
                return Ok(new_function_parser);
            }
            Err(_) => {
                new_function_parser.get_function_dependencies()?;
                log::debug!(
                    "new_function_parser_with_dependencies:\n{:#?}",
                    new_function_parser
                );
                let function_dependencies_metadata = FunctionDependenciesMetadata::new(
                    new_function_parser.name.clone(),
                    BatMetadata::create_metadata_id(),
                    new_function_parser.function_metadata.metadata_id.clone(),
                    new_function_parser
                        .dependencies
                        .clone()
                        .into_iter()
                        .map(|func_dep| {
                            bat_metadata
                                .source_code
                                .get_function_by_id(func_dep)
                                .change_context(ParserError)
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .map(|func_meta| FunctionDependencyInfo {
                            function_name: func_meta.name.clone(),
                            function_metadata_id: func_meta.metadata_id,
                        })
                        .collect::<Vec<_>>(),
                    new_function_parser.external_dependencies.clone(),
                );
                function_dependencies_metadata
                    .update_metadata_file()
                    .change_context(ParserError)?;
                for function_dependency in new_function_parser.clone().dependencies {
                    if let Err(_) = bat_metadata
                        .get_functions_dependencies_metadata_by_function_metadata_id(
                            function_dependency.clone(),
                        )
                        .change_context(ParserError)
                    {
                        let function_metadata = bat_metadata
                            .source_code
                            .get_function_by_id(function_dependency.clone())
                            .change_context(ParserError)?;
                        FunctionParser::new_from_metadata(function_metadata)?;
                    }
                }
            }
        }
        Ok(new_function_parser)
    }

    pub fn new_from_metadata(
        function_metadata: FunctionSourceCodeMetadata,
    ) -> Result<Self, ParserError> {
        let name = function_metadata.name.clone();
        let content = function_metadata
            .to_source_code_parser(None)
            .get_source_code_content();
        Self::new(name, function_metadata, content)
    }

    fn get_function_dependencies(&mut self) -> Result<(), ParserError> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let function_metadata = bat_metadata.source_code.functions_source_code.clone();
        let trait_metadata_vec = bat_metadata
            .traits
            .into_iter()
            .filter(|t_metadata| !t_metadata.external_trait)
            .collect::<Vec<_>>();

        let detected_calls = match syn_function_dependency_parser::detect_function_calls(&self.content) {
            Ok(calls) => calls,
            Err(e) => {
                log::warn!("syn parse failed for {}: {}", self.name, e);
                return Ok(());
            }
        };

        log::debug!("syn detected_calls: {:#?}", detected_calls);

        let mut dependency_function_metadata_id_vec: Vec<MetadataId> = vec![];

        for call in &detected_calls {
            if call.function_name == self.name {
                continue;
            }

            match &call.call_type {
                CallType::StaticMethod { type_name } => {
                    let trait_signature = format!("{}::{}", type_name, call.function_name);
                    let impl_function_metadata_id =
                        trait_metadata_vec
                            .iter()
                            .find_map(|trait_metadata| {
                                trait_metadata.impl_functions.iter().find_map(|impl_func| {
                                    if impl_func.trait_signature == trait_signature {
                                        Some(impl_func.function_source_code_metadata_id.clone())
                                    } else {
                                        None
                                    }
                                })
                            });
                    if let Some(metadata_id) = impl_function_metadata_id {
                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                            dependency_function_metadata_id_vec.push(metadata_id);
                        }
                    } else if let Some(metadata_id) = self.resolve_function_by_name(&call.function_name, &function_metadata) {
                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                            dependency_function_metadata_id_vec.push(metadata_id);
                        }
                    } else {
                        let dep_name = format!("{}::{}", type_name, call.function_name);
                        if !self.external_dependencies.contains(&dep_name) {
                            self.external_dependencies.push(dep_name);
                        }
                    }
                }
                CallType::FreeFunction => {
                    if let Some(metadata_id) = self.resolve_function_by_name(&call.function_name, &function_metadata) {
                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                            dependency_function_metadata_id_vec.push(metadata_id);
                        }
                    } else if !self.external_dependencies.contains(&call.function_name) {
                        self.external_dependencies.push(call.function_name.clone());
                    }
                }
                CallType::MethodCall { receiver } => {
                    // Resolve method calls on ctx.accounts (e.g. ctx.accounts.process())
                    if let Some(ref recv) = receiver {
                        if recv.contains("accounts") {
                            // Extract context type name from function parameters
                            // Note: syn's to_token_stream() adds spaces around < and >,
                            // so we normalize by removing spaces around angle brackets
                            let context_type_name = self.parameters.iter().find_map(|p| {
                                let normalized_type = normalize_generic_type(&p.parameter_type);
                                if normalized_type.contains("Context<") {
                                    // Extract T from Context<T> or Context<'_, T>
                                    let after_context = normalized_type.split("Context<").nth(1)?;
                                    let inner = after_context.trim_end_matches('>');
                                    // Handle lifetime params like Context<'_, Initialize>
                                    let type_name = inner.split(',').last()?.trim();
                                    Some(type_name.to_string())
                                } else {
                                    None
                                }
                            });

                            if let Some(ctx_type) = context_type_name {
                                // Find the impl block for this context type (impl T where impl_from is empty = direct impl)
                                let trait_signature = format!("{}::{}", ctx_type, call.function_name);
                                let impl_function_metadata_id = trait_metadata_vec.iter().find_map(|trait_metadata| {
                                    // Direct impl: impl_to matches context type and impl_from is empty
                                    if trait_metadata.impl_to == ctx_type && trait_metadata.impl_from.is_empty() {
                                        trait_metadata.impl_functions.iter().find_map(|impl_func| {
                                            if impl_func.trait_signature == trait_signature {
                                                Some(impl_func.function_source_code_metadata_id.clone())
                                            } else {
                                                None
                                            }
                                        })
                                    } else {
                                        None
                                    }
                                });

                                if let Some(metadata_id) = impl_function_metadata_id {
                                    if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                                        dependency_function_metadata_id_vec.push(metadata_id);
                                    }
                                } else {
                                    // Fallback: try any impl block matching the trait_signature
                                    let fallback_id = trait_metadata_vec.iter().find_map(|trait_metadata| {
                                        trait_metadata.impl_functions.iter().find_map(|impl_func| {
                                            if impl_func.trait_signature == trait_signature {
                                                Some(impl_func.function_source_code_metadata_id.clone())
                                            } else {
                                                None
                                            }
                                        })
                                    });
                                    if let Some(metadata_id) = fallback_id {
                                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                                            dependency_function_metadata_id_vec.push(metadata_id);
                                        }
                                    } else if let Some(metadata_id) = self.resolve_function_by_name(&call.function_name, &function_metadata) {
                                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                                            dependency_function_metadata_id_vec.push(metadata_id);
                                        }
                                    }
                                }
                                continue;
                            }
                        }
                    }
                    // Resolve self.method() calls: find which impl block contains
                    // the current function, then look up the method in the same impl block
                    if let Some(ref recv) = receiver {
                        if recv == "self" || recv.starts_with("self.") {
                            if let Some(metadata_id) = self.resolve_self_method_call(
                                &call.function_name,
                                &trait_metadata_vec,
                            ) {
                                if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                                    dependency_function_metadata_id_vec.push(metadata_id);
                                }
                                continue;
                            }
                        }
                    }
                    // Fallback: try to resolve by name for other method calls
                    if let Some(metadata_id) = self.resolve_function_by_name(&call.function_name, &function_metadata) {
                        if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                            dependency_function_metadata_id_vec.push(metadata_id);
                        }
                    }
                }
            }
        }

        self.dependencies = dependency_function_metadata_id_vec;
        Ok(())
    }

    fn resolve_function_by_name(
        &self,
        function_name: &str,
        function_metadata: &[FunctionSourceCodeMetadata],
    ) -> Option<MetadataId> {
        let candidates: Vec<_> = function_metadata
            .iter()
            .filter(|f| f.name == function_name)
            .collect();

        match candidates.len() {
            0 => None,
            1 => Some(candidates[0].metadata_id.clone()),
            _ => {
                // Priority 1: same file
                if let Some(f) = candidates.iter()
                    .find(|f| f.path == self.function_metadata.path)
                {
                    return Some(f.metadata_id.clone());
                }
                // Priority 2: use import analysis — parse the caller's file to find
                // which module the function was imported from
                if let Some(f) = self.resolve_by_imports(function_name, &candidates) {
                    return Some(f);
                }
                // Priority 3: same directory
                let self_dir = std::path::Path::new(&self.function_metadata.path).parent();
                if let Some(f) = candidates.iter()
                    .find(|f| std::path::Path::new(&f.path).parent() == self_dir)
                {
                    return Some(f.metadata_id.clone());
                }
                // Priority 4: first match
                log::warn!(
                    "Ambiguous resolution for '{}': {} candidates, using first match. Caller: {}",
                    function_name,
                    candidates.len(),
                    self.function_metadata.path,
                );
                Some(candidates[0].metadata_id.clone())
            }
        }
    }

    /// Resolves `self.method()` calls by finding which impl block contains the current
    /// function (via trait metadata), then looking up the method in the same impl block.
    fn resolve_self_method_call(
        &self,
        method_name: &str,
        trait_metadata_vec: &[crate::batbelt::metadata::trait_metadata::TraitMetadata],
    ) -> Option<MetadataId> {
        // Find which impl block contains the current function
        let self_impl_type = trait_metadata_vec.iter().find_map(|trait_metadata| {
            let contains_self = trait_metadata.impl_functions.iter().any(|impl_func| {
                impl_func.function_source_code_metadata_id == self.function_metadata.metadata_id
            });
            if contains_self {
                Some(trait_metadata.impl_to.clone())
            } else {
                None
            }
        })?;

        log::debug!(
            "resolve_self_method_call: function '{}' belongs to impl '{}'",
            self.name,
            self_impl_type
        );

        // Now look for method_name in the same type's impl blocks
        let trait_signature = format!("{}::{}", self_impl_type, method_name);
        trait_metadata_vec.iter().find_map(|trait_metadata| {
            if trait_metadata.impl_to == self_impl_type {
                trait_metadata.impl_functions.iter().find_map(|impl_func| {
                    if impl_func.trait_signature == trait_signature {
                        Some(impl_func.function_source_code_metadata_id.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        })
    }

    /// Parses the caller's file with `syn::parse_file()` to extract `use` imports,
    /// then matches imported module paths against candidate function file paths.
    fn resolve_by_imports(
        &self,
        function_name: &str,
        candidates: &[&FunctionSourceCodeMetadata],
    ) -> Option<MetadataId> {
        let file_content = std::fs::read_to_string(&self.function_metadata.path).ok()?;
        let syntax = syn::parse_file(&file_content).ok()?;

        // Collect all imported paths that end with the function name
        let import_paths = Self::extract_use_paths_for_name(&syntax.items, function_name);

        if import_paths.is_empty() {
            return None;
        }

        log::debug!(
            "Import paths for '{}' in {}: {:?}",
            function_name,
            self.function_metadata.path,
            import_paths
        );

        // For each import path, try to match it against candidate file paths.
        // E.g., import "crate::state::initialize::process" should match a candidate
        // whose path contains "state/initialize" or similar.
        for import_path in &import_paths {
            // Convert module path segments to a path-like string for matching
            // e.g., ["crate", "state", "initialize", "process"] -> "state/initialize"
            let path_segments: Vec<&str> = import_path
                .split("::")
                .filter(|s| *s != "crate" && *s != "super" && *s != "self" && *s != function_name)
                .collect();

            if path_segments.is_empty() {
                continue;
            }

            // Build a path fragment to match against file paths
            let path_fragment = path_segments.join("/");

            if let Some(candidate) = candidates.iter().find(|c| {
                c.path.contains(&path_fragment)
            }) {
                log::debug!(
                    "Resolved '{}' via import '{}' -> {} (path fragment: {})",
                    function_name,
                    import_path,
                    candidate.path,
                    path_fragment
                );
                return Some(candidate.metadata_id.clone());
            }

            // Also try matching with just the last module segment (the file name)
            // e.g., "initialize" should match "*/initialize.rs" or "*/initialize/mod.rs"
            if let Some(&last_segment) = path_segments.last() {
                let file_match_rs = format!("{}.rs", last_segment);
                let file_match_mod = format!("{}/mod.rs", last_segment);
                let dir_match = format!("/{}/", last_segment);
                if let Some(candidate) = candidates.iter().find(|c| {
                    c.path.ends_with(&file_match_rs)
                        || c.path.ends_with(&file_match_mod)
                        || c.path.contains(&dir_match)
                }) {
                    log::debug!(
                        "Resolved '{}' via import '{}' (last segment '{}') -> {}",
                        function_name,
                        import_path,
                        last_segment,
                        candidate.path
                    );
                    return Some(candidate.metadata_id.clone());
                }
            }
        }

        None
    }

    /// Recursively extracts all `use` paths from a file's items that import `target_name`.
    /// Returns paths like "crate::state::initialize::process".
    fn extract_use_paths_for_name(items: &[syn::Item], target_name: &str) -> Vec<String> {
        let mut paths = vec![];
        for item in items {
            match item {
                syn::Item::Use(item_use) => {
                    Self::collect_use_paths(&item_use.tree, "", target_name, &mut paths);
                }
                syn::Item::Mod(item_mod) => {
                    // Also check inline modules (mod foo { use ... })
                    if let Some((_, ref items)) = item_mod.content {
                        let mut sub_paths = Self::extract_use_paths_for_name(items, target_name);
                        paths.append(&mut sub_paths);
                    }
                }
                _ => {}
            }
        }
        paths
    }

    fn collect_use_paths(
        tree: &syn::UseTree,
        prefix: &str,
        target_name: &str,
        paths: &mut Vec<String>,
    ) {
        match tree {
            syn::UseTree::Path(use_path) => {
                let new_prefix = if prefix.is_empty() {
                    use_path.ident.to_string()
                } else {
                    format!("{}::{}", prefix, use_path.ident)
                };
                Self::collect_use_paths(&use_path.tree, &new_prefix, target_name, paths);
            }
            syn::UseTree::Name(use_name) => {
                if use_name.ident == target_name {
                    let full_path = if prefix.is_empty() {
                        target_name.to_string()
                    } else {
                        format!("{}::{}", prefix, target_name)
                    };
                    paths.push(full_path);
                }
            }
            syn::UseTree::Rename(use_rename) => {
                if use_rename.rename == target_name || use_rename.ident == target_name {
                    let full_path = if prefix.is_empty() {
                        use_rename.ident.to_string()
                    } else {
                        format!("{}::{}", prefix, use_rename.ident)
                    };
                    paths.push(full_path);
                }
            }
            syn::UseTree::Glob(_) => {
                // For `use module::*`, we can't be sure which function it imports,
                // but we record the module path for matching
                if !prefix.is_empty() {
                    paths.push(format!("{}::{}", prefix, target_name));
                }
            }
            syn::UseTree::Group(use_group) => {
                for item in &use_group.items {
                    Self::collect_use_paths(item, prefix, target_name, paths);
                }
            }
        }
    }

    fn get_function_signature(&mut self) {
        let function_signature = self.content.clone();
        let function_signature = function_signature
            .split('{')
            .next()
            .unwrap()
            .split("->")
            .next()
            .unwrap();
        self.signature = function_signature.trim().to_string();
    }

    fn get_function_parameters(&mut self) -> Result<(), ParserError> {
        use quote::ToTokens;

        if self.content.is_empty() {
            return Err(Report::new(ParserError).attach_printable(
                "Error parsing function, content needs to be initialized",
            ))?;
        }

        let item_fn = syn::parse_str::<syn::ItemFn>(&self.content).or_else(|_| {
            let wrapped = format!("fn __wrapper() {{ {} }}", &self.content);
            syn::parse_str::<syn::ItemFn>(&wrapped)
        });

        let result = match item_fn {
            Ok(item_fn) => {
                item_fn
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::FnArg::Receiver(_) => None,
                        syn::FnArg::Typed(pat_type) => {
                            let name = pat_type.pat.to_token_stream().to_string();
                            let ty = pat_type.ty.to_token_stream().to_string();
                            Some(FunctionParameterParser {
                                parameter_name: name,
                                parameter_type: ty,
                            })
                        }
                    })
                    .collect::<Vec<_>>()
            }
            Err(_) => {
                // Fallback to legacy string parsing
                let function_signature = self.signature.clone();
                let content_lines = self.content.lines();
                let parameters = if content_lines.clone().next().unwrap().contains('{') {
                    let function_signature_tokenized = function_signature
                        .trim_start_matches("pub (crate) fn ")
                        .trim_start_matches("pub fn ")
                        .split('(')
                        .last()
                        .unwrap()
                        .trim_end_matches(')')
                        .split(' ')
                        .collect::<Vec<_>>();
                    if function_signature_tokenized.is_empty()
                        || function_signature_tokenized[0].is_empty()
                    {
                        return Ok(());
                    }
                    let mut parameters: Vec<String> = vec![];
                    function_signature_tokenized.iter().enumerate().fold(
                        "".to_string(),
                        |total, current| {
                            if current.1.contains(':') {
                                if !total.is_empty() {
                                    parameters.push(total);
                                }
                                current.1.to_string()
                            } else if current.0 == function_signature_tokenized.len() - 1 {
                                parameters.push(format!("{} {}", total, current.1));
                                total
                            } else {
                                format!("{} {}", total, current.1)
                            }
                        },
                    );
                    parameters
                } else {
                    function_signature
                        .lines()
                        .filter(|line| {
                            line.contains(':')
                                && !line.contains("pub fn")
                                && !line.contains("pub (crate) fn")
                        })
                        .map(|line| line.trim().trim_end_matches(',').to_string())
                        .collect()
                };
                parameters
                    .into_iter()
                    .map(|parameter| {
                        let tokenized = parameter.split(": ");
                        FunctionParameterParser {
                            parameter_name: tokenized.clone().next().unwrap().to_string(),
                            parameter_type: tokenized.clone().last().unwrap().to_string(),
                        }
                    })
                    .collect::<Vec<_>>()
            }
        };
        self.parameters = result;
        Ok(())
    }

    fn get_function_body(&mut self) {
        let function_body = self.content.clone();
        let mut body = function_body.split('{');
        body.next();
        let body = body.collect::<Vec<_>>().join("{");
        self.body = body.trim_end_matches('}').trim().to_string();
    }

    pub fn get_function_name_from_signature(function_signature: &str) -> String {
        function_signature
            .trim_start_matches("pub ")
            .trim_start_matches("(crate) ")
            .trim_start_matches("fn ")
            .split('(')
            .next()
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .to_string()
    }
}

// #[test]
// fn test_get_function_information() {
//     let test_function = "/// Validates a given key and its permissions.
// /// Returns the parsed permissions.
// pub fn validate_and_parse_static<'a, 'b: 'a, P: StaticPermissionKey>(
//     profile: impl Into<ZeroCopyWrapper<'a, 'b, Profile>>,
//     key: &Signer,
//     key_index: u16,
//     clock: Option<&Clock>,
// ) -> Result<KeyValidate<(P, u128)>> {
//     validate_against_list(profile, key, key_index, once(P::permission_key()), clock)
// }";
//     let function_parser = FunctionParser {
//         name: "".to_string(),
//         function_metadata: FunctionMetadata {
//             path: "../star-atlas-programs/sol-programs/programs/player_profile/src/util.rs"
//                 .to_string(),
//             name: "validate_and_parse_static".to_string(),
//             function_type: FunctionMetadataType::Other,
//             start_line_index: 121,
//             end_line_index: 128,
//         },
//         content: "".to_string(),
//         signature: "".to_string(),
//         body: "".to_string(),
//         parameters: vec![],
//         dependencies: vec![],
//         external_dependencies: vec![],
//     };
// }
//
// #[test]
// fn test_dep_regex() {
//     let test_text = "
// profile.into();
// let list = profile.list()?;
// let profile_key = list.get(key_index as usize).ok_or_else(|| {
// ProfilePermissions::from_bits_truncate(u128::from_le_bytes(profile_key.permissions))
// contains(ProfilePermissions::AUTH)
// return Ok(KeyValidate::Auth);
// Ok(Clock::get()?.unix_timestamp),
// Ok(clock.unix_timestamp),
// Ok(KeyValidate::Permissioned((
// profile_key.permission_key,
// profile_key.permissions,
// )))
//
// match validate_key(profile, key, key_index, clock)? {
// KeyValidate::Auth => Ok(KeyValidate::Auth),
// KeyValidate::Permissioned((permission_key, permissions)) => {
// let permissions = u128::from_le_bytes(permissions);
// Ok(KeyValidate::Permissioned((
// permission_key,
// P::from_bits_truncate(permissions),
// permissions,
// )))
//
// match validate_and_parse(profile, key, key_index, clock)? {
// KeyValidate::Auth => Ok(KeyValidate::Auth),
// KeyValidate::Permissioned((permission_key, permissions, permissions_raw)) => {
// if !valid_permission_keys.into_iter().any(|k| k == &permission_key)
// Ok(KeyValidate::Permissioned((permissions, permissions_raw)))
// validate_against_list(profile, key, key_index, once(P::permission_key()), clock)
// match validate_and_parse_static(profile, key, key_index, clock)? {
// KeyValidate::Auth => Ok(KeyValidate::Auth),
// KeyValidate::Permissioned((permissions, permissions_raw)) => {
// let permissions: P = permissions;
// if permissions.contains(required_permissions) {
// Ok(KeyValidate::Permissioned(permissions_raw))
// match validate_against_list(profile, key, key_index, valid_permission_keys, clock)? {
// KeyValidate::Auth => Ok(KeyValidate::Auth),
// KeyValidate::Permissioned((permissions, permissions_raw)) => {
//
// if permissions.contains(required_permissions) {
// Ok(KeyValidate::Permissioned(permissions_raw))";
//
//     let dependency_regex = Regex::new(r"[A-Za-z0-9_]+\(([A-Za-z0-9_,\s]*)\)").unwrap(); //[A-Za-z0-9_]+\(([A-Za-z0-9_,():\s])*\)$
//     let dependency_parser_vec = dependency_regex
//         .find_iter(test_text)
//         .map(|regex_match| regex_match.as_str().to_string())
//         .collect::<Vec<_>>();
//     println!("result: \n{:#?}", dependency_parser_vec);
// }
//
// #[test]
// fn test_detect_double_parentheses() {
//     let test_text = "Permissioned((permissions, permissions_raw))";
//     let test_regex = Regex::new(r"[A-Z][a-z]*\(\([A-Za-z, _:.]*\)\)").unwrap();
//     let result = test_regex.is_match(test_text);
//     println!("{}", result);
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_use_paths_simple() {
        let source = r#"
            use crate::state::initialize::process;
            use crate::utils::check_context;
            fn main() {}
        "#;
        let syntax = syn::parse_file(source).unwrap();

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "process");
        assert_eq!(paths, vec!["crate::state::initialize::process"]);

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "check_context");
        assert_eq!(paths, vec!["crate::utils::check_context"]);

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "nonexistent");
        assert!(paths.is_empty());
    }

    #[test]
    fn test_extract_use_paths_grouped() {
        let source = r#"
            use crate::state::initialize::{process, validate};
            fn main() {}
        "#;
        let syntax = syn::parse_file(source).unwrap();

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "process");
        assert_eq!(paths, vec!["crate::state::initialize::process"]);

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "validate");
        assert_eq!(paths, vec!["crate::state::initialize::validate"]);
    }

    #[test]
    fn test_extract_use_paths_glob() {
        let source = r#"
            use crate::state::initialize::*;
            fn main() {}
        "#;
        let syntax = syn::parse_file(source).unwrap();

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "process");
        assert_eq!(paths, vec!["crate::state::initialize::process"]);
    }

    #[test]
    fn test_extract_use_paths_rename() {
        let source = r#"
            use crate::state::initialize::process as init_process;
            fn main() {}
        "#;
        let syntax = syn::parse_file(source).unwrap();

        let paths = FunctionParser::extract_use_paths_for_name(&syntax.items, "init_process");
        assert_eq!(paths, vec!["crate::state::initialize::process"]);
    }
}
