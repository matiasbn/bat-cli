use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::traits_metadata::TraitMetadata;
use crate::batbelt::metadata::{BatMetadataParser, MetadataId};
use crate::batbelt::parser::trait_parser::TraitParser;
use crate::batbelt::parser::{ParserError, ParserResult};

use crate::batbelt::metadata::metadata_cache::{MetadataCache, MetadataCacheType};
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
    pub function_metadata: FunctionMetadata,
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
        function_metadata: FunctionMetadata,
        content: String,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
        optional_trait_impl_parser_vec: Option<Vec<TraitParser>>,
    ) -> Result<Self, ParserError> {
        let mut new_function_parser = Self {
            name,
            function_metadata,
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
        new_function_parser.get_function_dependencies(
            optional_function_metadata_vec,
            optional_trait_impl_parser_vec,
        )?;
        log::debug!(
            "new_function_parser_with_dependencies:\n{:#?}",
            new_function_parser
        );
        new_function_parser.save_function_metadata_cache()?;
        Ok(new_function_parser)
    }

    fn save_function_metadata_cache(&self) -> ParserResult<()> {
        self.function_metadata
            .save_dependency_cache(self.dependencies.clone())
            .change_context(ParserError)?;
        self.function_metadata
            .save_external_dependency_cache(self.external_dependencies.clone())
            .change_context(ParserError)?;
        Ok(())
    }

    pub fn new_from_metadata(
        function_metadata: FunctionMetadata,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
        optional_trait_impl_parser_vec: Option<Vec<TraitParser>>,
    ) -> Result<Self, ParserError> {
        let name = function_metadata.name.clone();
        let content = function_metadata
            .to_source_code_parser(None)
            .get_source_code_content();
        Self::new(
            name,
            function_metadata,
            content,
            optional_function_metadata_vec,
            optional_trait_impl_parser_vec,
        )
    }

    fn get_function_dependencies(
        &mut self,
        optional_function_metadata_vec: Option<Vec<FunctionMetadata>>,
        optional_trait_impl_parser_vec: Option<Vec<TraitParser>>,
    ) -> Result<(), ParserError> {
        let function_metadata = if optional_function_metadata_vec.is_some() {
            optional_function_metadata_vec.unwrap()
        } else {
            FunctionMetadata::get_filtered_metadata(None, None).change_context(ParserError)?
        };

        let trait_impl_parser_vec = if optional_trait_impl_parser_vec.is_some() {
            optional_trait_impl_parser_vec.unwrap()
        } else {
            TraitMetadata::get_trait_parser_vec(None, None, Some(function_metadata.clone()))
                .change_context(ParserError)?
        };
        let double_parentheses_regex = Regex::new(r"[A-Z][a-z]*\(\([A-Za-z, _:.]*\)\)").unwrap();

        let impl_function_regex =
            Regex::new(r"[A-Za-z0-9_]+::[A-Za-z0-9]+\(\(?[A-Za-z0-9]*\)?\)").unwrap();

        let impl_function_matches = impl_function_regex
            .find_iter(&self.body.clone())
            .map(|impl_match| impl_match.as_str().to_string())
            .collect::<Vec<_>>();

        let mut dependency_function_metadata_id_vec = vec![];

        for impl_match in impl_function_matches {
            let mut impl_match_split = impl_match.split("::");
            let impl_match_to = impl_match_split.next().unwrap();
            let impl_match_function_name =
                Self::get_function_name_from_signature(impl_match_split.next().unwrap());
            let impl_function_metadata =
                trait_impl_parser_vec
                    .clone()
                    .into_iter()
                    .find_map(|impl_parser| {
                        if impl_parser.impl_to.contains(impl_match_to) {
                            let f_metadata = impl_parser
                                .impl_functions
                                .into_iter()
                                .find(|impl_fn| impl_fn.name == impl_match_function_name);
                            if f_metadata.is_some() {
                                Some(f_metadata.unwrap())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    });
            if impl_function_metadata.is_some() {
                dependency_function_metadata_id_vec
                    .push(impl_function_metadata.unwrap().metadata_id);
            }
        }

        let dependency_regex = Regex::new(r"[A-Za-z0-9_]+\(([A-Za-z0-9_:.&, ()]*)\)").unwrap(); //[A-Za-z0-9_]+\(([A-Za-z0-9_,():\s])*\)$
        let dependency_function_names_vec = dependency_regex
            .find_iter(&self.body.clone())
            .filter_map(|reg_match| {
                let match_str = reg_match.as_str().to_string();
                log::debug!("match_str_regex {}", match_str);
                let function_name = Self::get_function_name_from_signature(&match_str);
                log::debug!("match_str_regex_function_name {}", function_name);
                let matching_line = self
                    .body
                    .clone()
                    .lines()
                    .find(|line| line.contains(&match_str))
                    .unwrap()
                    .to_string();
                if function_name != self.name
                    && function_name != "Ok"
                    && function_name != "Some"
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
            .clone()
            .into_iter()
            .filter(|f_meta| {
                !dependency_function_metadata_id_vec
                    .clone()
                    .into_iter()
                    .any(|dep_metadata_id| dep_metadata_id == f_meta.metadata_id.clone())
            })
            .collect::<Vec<_>>();

        for dependency_function_name in dependency_function_names_vec {
            let dependency_function_metadata_vec = filtered_function_metadata_vec
                .clone()
                .into_iter()
                .filter(|f_metadata| f_metadata.clone().name == dependency_function_name.clone())
                .collect::<Vec<_>>();
            if !dependency_function_metadata_vec.clone().is_empty() {
                for dependency_metadata in dependency_function_metadata_vec.clone() {
                    if !dependency_metadata
                        .read_cache()
                        .change_context(ParserError)?
                        .is_null()
                    {
                        dependency_function_metadata_id_vec.push(dependency_metadata.metadata_id);
                    } else {
                        let function_parser = dependency_metadata
                            .to_function_parser(
                                Some(function_metadata.clone()),
                                Some(trait_impl_parser_vec.clone()),
                            )
                            .change_context(ParserError)?;
                        dependency_function_metadata_id_vec
                            .push(function_parser.function_metadata.metadata_id);
                    }
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
