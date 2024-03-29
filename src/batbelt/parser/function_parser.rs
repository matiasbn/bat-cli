use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;

use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, MetadataId};

use crate::batbelt::parser::ParserError;

use crate::batbelt::metadata::function_dependencies_metadata::{
    FunctionDependenciesMetadata, FunctionDependencyInfo,
};

use error_stack::{Report, Result, ResultExt};
use regex::Regex;

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
        // only not external
        let trait_metadata_vec = bat_metadata
            .traits
            .into_iter()
            .filter(|t_metadata| !t_metadata.external_trait)
            .collect::<Vec<_>>();
        let mut body_clone = self.body.clone();

        let double_parentheses_regex = Regex::new(r"[A-Z][a-z]*\(\([A-Za-z, _:.]*\)\)").unwrap();
        let mut dependency_function_metadata_id_vec = vec![];

        let impl_function_regex =
            Regex::new(r"[A-Za-z0-9_]+::[A-Za-z0-9]+\(\(?[&._A-Za-z0-9]*\)?\)").unwrap();

        let impl_function_matches = impl_function_regex
            .find_iter(&body_clone)
            .map(|impl_match| impl_match.as_str().to_string())
            .collect::<Vec<_>>();
        log::debug!("impl_function_matches: \n{:#?}", impl_function_matches);
        for impl_match in impl_function_matches {
            log::debug!("impl_match: {}", impl_match);
            // delete from body to avoid double checking
            body_clone = body_clone.replace(&impl_match, "");
            let impl_function_signature_match = Self::get_function_name_from_signature(&impl_match);
            log::debug!("impl_match_signature: {}", &impl_function_signature_match);
            let impl_function_metadata_id =
                trait_metadata_vec
                    .clone()
                    .into_iter()
                    .find_map(|trait_metadata| {
                        match trait_metadata.impl_functions.into_iter().find(|impl_func| {
                            impl_func.trait_signature == impl_function_signature_match
                        }) {
                            None => None,
                            Some(impl_func) => Some(impl_func.function_source_code_metadata_id),
                        }
                    });
            log::debug!(
                "impl_function_metadata_match {:#?}",
                impl_function_metadata_id
            );
            if impl_function_metadata_id.is_some() {
                dependency_function_metadata_id_vec.push(impl_function_metadata_id.unwrap());
            }
        }

        let dependency_regex = Regex::new(r"[A-Za-z0-9_]+\(([A-Za-z0-9_:.&, ()]*)\)").unwrap(); //[A-Za-z0-9_]+\(([A-Za-z0-9_,():\s])*\)$
        let dependency_function_names_vec = dependency_regex
            .find_iter(&body_clone)
            .filter_map(|reg_match| {
                let match_str = reg_match.as_str().to_string();
                log::debug!("match_str_regex {}", match_str);
                let function_name = Self::get_function_name_from_signature(&match_str);
                log::debug!("match_str_regex_function_name {}", function_name);
                if function_name == "Ok" || function_name == "Some" {
                    return None;
                };
                let matching_line = body_clone
                    .lines()
                    .find(|line| line.contains(&match_str))
                    .unwrap()
                    .to_string();
                if function_name != self.name
                    && !double_parentheses_regex.is_match(&match_str)
                    && !matching_line.contains("self.")
                    && !matching_line.contains("Self::")
                {
                    Some(function_name)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        // filter the already found dependencies
        let filtered_function_metadata_vec = function_metadata
            .into_iter()
            .filter(|f_meta| {
                !dependency_function_metadata_id_vec
                    .clone()
                    .into_iter()
                    .any(|dep_metadata_id| dep_metadata_id == f_meta.metadata_id.clone())
            })
            .collect::<Vec<_>>();
        let _bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        for dependency_function_name in dependency_function_names_vec {
            let dependency_function_metadata_vec = filtered_function_metadata_vec
                .clone()
                .into_iter()
                .filter(|f_metadata| f_metadata.clone().name == dependency_function_name.clone())
                .collect::<Vec<_>>();
            if !dependency_function_metadata_vec.clone().is_empty() {
                for dependency_metadata in dependency_function_metadata_vec.clone() {
                    dependency_function_metadata_id_vec
                        .push(dependency_metadata.metadata_id.clone())
                }
            } else {
                self.external_dependencies.push(dependency_function_name);
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
        if self.content.is_empty() || self.signature.is_empty() {
            return Err(Report::new(ParserError).attach_printable(
                "Error parsing function, both content and signature needs to be initialized",
            ))?;
        }
        let content_lines = self.content.lines();
        let function_signature = self.signature.clone();

        //Function parameters
        // single line function
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
            if function_signature_tokenized.is_empty() || function_signature_tokenized[0].is_empty()
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
            //multiline
            // parameters contains :
            let filtered: Vec<String> = function_signature
                .lines()
                .filter(|line| {
                    line.contains(':')
                        && !line.contains("pub fn")
                        && !line.contains("pub (crate) fn")
                })
                .map(|line| line.trim().trim_end_matches(',').to_string())
                .collect();
            filtered
        };
        let result = parameters
            .into_iter()
            .map(|parameter| {
                let tokenized = parameter.split(": ");
                FunctionParameterParser {
                    parameter_name: tokenized.clone().next().unwrap().to_string(),
                    parameter_type: tokenized.clone().last().unwrap().to_string(),
                }
            })
            .collect::<Vec<_>>();
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
