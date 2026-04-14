use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;

use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, MetadataId};

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
        use crate::batbelt::parser::call_resolver::{CallResolver, Resolution};
        use crate::batbelt::parser::file_scope::FileScope;

        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let function_metadata = bat_metadata.source_code.functions_source_code.clone();
        let trait_metadata_vec = bat_metadata
            .traits
            .into_iter()
            .filter(|t_metadata| !t_metadata.external_trait)
            .collect::<Vec<_>>();

        // Build the FileScope for the file that contains this function.
        // FileScope gives us the imports map for deterministic name resolution.
        let file_content = match std::fs::read_to_string(&self.function_metadata.path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!(
                    "Could not read file '{}' for dependency resolution: {}",
                    self.function_metadata.path,
                    e
                );
                return Ok(());
            }
        };

        let file_scope = match FileScope::from_file_content(
            self.function_metadata.path.clone(),
            &file_content,
        ) {
            Ok(scope) => scope,
            Err(e) => {
                log::warn!(
                    "Could not parse file '{}' for FileScope: {}",
                    self.function_metadata.path,
                    e
                );
                return Ok(());
            }
        };

        // Parse the function itself. We try parsing as a standalone ItemFn first;
        // if that fails (e.g. for method bodies without the fn keyword visible),
        // we wrap it.
        let item_fn = syn::parse_str::<syn::ItemFn>(&self.content).or_else(|_| {
            let wrapped = format!("fn __wrapper() {{ {} }}", &self.content);
            syn::parse_str::<syn::ItemFn>(&wrapped)
        });

        let item_fn = match item_fn {
            Ok(f) => f,
            Err(e) => {
                log::warn!("syn parse failed for function '{}': {}", self.name, e);
                return Ok(());
            }
        };

        // Create the resolver and run it.
        let resolver = CallResolver::new(
            &file_scope,
            &trait_metadata_vec,
            &function_metadata,
            &self.function_metadata.metadata_id,
        );
        let resolved_calls = resolver.resolve_function(&item_fn);

        log::debug!(
            "CallResolver: function '{}' has {} calls",
            self.name,
            resolved_calls.len()
        );

        let mut dependency_function_metadata_id_vec: Vec<MetadataId> = vec![];

        for call in resolved_calls {
            match call.resolution {
                Resolution::Internal(metadata_id) => {
                    if !dependency_function_metadata_id_vec.contains(&metadata_id) {
                        dependency_function_metadata_id_vec.push(metadata_id);
                    }
                }
                Resolution::External(name) => {
                    if !self.external_dependencies.contains(&name) {
                        self.external_dependencies.push(name);
                    }
                }
                Resolution::Unresolved(name) => {
                    log::debug!(
                        "Unresolved call '{}' in function '{}' ({}), marking external",
                        name,
                        self.name,
                        self.function_metadata.path
                    );
                    if !self.external_dependencies.contains(&name) {
                        self.external_dependencies.push(name);
                    }
                }
            }
        }

        self.dependencies = dependency_function_metadata_id_vec;
        Ok(())
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

