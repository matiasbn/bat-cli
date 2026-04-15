use crate::batbelt::metadata::functions_source_code_metadata::{
    FunctionMetadataType, FunctionSourceCodeMetadata,
};

use crate::batbelt::metadata::structs_source_code_metadata::{
    StructMetadataType, StructSourceCodeMetadata,
};
use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::config::BatConfig;

use error_stack::{IntoReport, Report, Result, ResultExt};
use std::collections::HashSet;
use std::fs;

use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, SourceCodeMetadata};
use crate::batbelt::parser::function_parser::FunctionParser;

use crate::batbelt::parser::ParserError;

#[derive(Clone, Debug)]
pub struct EntrypointParser {
    pub name: String,
    pub dependencies: Vec<FunctionSourceCodeMetadata>,
    pub context_accounts: StructSourceCodeMetadata,
    pub entry_point_function: FunctionSourceCodeMetadata,
}

impl EntrypointParser {
    pub fn new(
        name: String,
        dependencies: Vec<FunctionSourceCodeMetadata>,
        context_accounts: StructSourceCodeMetadata,
        entry_point_function: FunctionSourceCodeMetadata,
    ) -> Self {
        Self {
            name,
            dependencies,
            context_accounts,
            entry_point_function,
        }
    }

    pub fn new_from_name(entrypoint_name: &str) -> Result<Self, ParserError> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        if let Ok(ep_metadata) =
            bat_metadata.get_entrypoint_metadata_by_name(entrypoint_name.to_string())
        {
            let entry_point_function = bat_metadata
                .source_code
                .get_function_by_id(ep_metadata.entrypoint_function_id.clone())
                .change_context(ParserError)?;
            let context_accounts = bat_metadata
                .source_code
                .get_struct_by_id(ep_metadata.context_accounts_id.clone())
                .change_context(ParserError)?;

            // Resolve dependencies recursively from the entrypoint function
            // First ensure the entrypoint function's dependencies are computed
            let _ = FunctionParser::new_from_metadata(entry_point_function.clone());
            let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
            let dependencies =
                Self::resolve_all_dependencies(&entry_point_function.metadata_id, &bat_metadata);

            return Ok(Self {
                name: ep_metadata.name,
                dependencies,
                context_accounts,
                entry_point_function,
            });
        };

        let entrypoint_section = BatMetadata::read_metadata()
            .change_context(ParserError)?
            .source_code
            .functions_source_code
            .into_iter()
            .filter(|func_meta| {
                func_meta.name == entrypoint_name
                    && func_meta.function_type == FunctionMetadataType::EntryPoint
            })
            .collect::<Vec<_>>();

        if entrypoint_section.len() != 1 {
            return Err(Report::new(ParserError)
                .attach_printable(
                    "Incorrect amount of results looking for entrypoint function section"
                        .to_string(),
                )
                .attach_printable(format!("expected: 1,  got: {}", entrypoint_section.len()))
                .attach_printable(format!("sections_filtered:\n{:#?}", entrypoint_section)))?;
        }

        let entrypoint_function = entrypoint_section.first().unwrap().clone();

        let context_name = Self::get_context_name(entrypoint_name).unwrap();

        let structs_metadata = SourceCodeMetadata::get_filtered_structs(
            Some(context_name.clone()),
            Some(StructMetadataType::ContextAccounts),
        )
        .change_context(ParserError)?;
        let context_accounts = structs_metadata
            .iter()
            .find(|struct_metadata| struct_metadata.name == context_name)
            .ok_or(ParserError)
            .into_report()
            .attach_printable(format!(
                "Error context_accounts struct by name {} for entrypoint_name: {}",
                context_name, entrypoint_name
            ))?;

        let ep_metadata = EntrypointMetadata {
            name: entrypoint_name.to_string(),
            metadata_id: BatMetadata::create_metadata_id(),
            handler_id: None,
            context_accounts_id: context_accounts.metadata_id.clone(),
            entrypoint_function_id: entrypoint_function.metadata_id.clone(),
        };

        ep_metadata
            .update_metadata_file()
            .change_context(ParserError)?;

        // Compute dependencies for the entrypoint function
        let _ = FunctionParser::new_from_metadata(entrypoint_function.clone());
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let dependencies =
            Self::resolve_all_dependencies(&entrypoint_function.metadata_id, &bat_metadata);

        Ok(Self {
            name: entrypoint_name.to_string(),
            dependencies,
            context_accounts: context_accounts.clone(),
            entry_point_function: entrypoint_function,
        })
    }

    fn resolve_all_dependencies(
        entrypoint_function_id: &str,
        bat_metadata: &BatMetadata,
    ) -> Vec<FunctionSourceCodeMetadata> {
        let mut visited = HashSet::new();
        let mut result = vec![];
        Self::collect_deps(
            entrypoint_function_id,
            bat_metadata,
            &mut visited,
            &mut result,
        );
        result
    }

    fn collect_deps(
        function_id: &str,
        bat_metadata: &BatMetadata,
        visited: &mut HashSet<String>,
        result: &mut Vec<FunctionSourceCodeMetadata>,
    ) {
        if !visited.insert(function_id.to_string()) {
            return;
        }
        if let Ok(dep_meta) = bat_metadata
            .get_functions_dependencies_metadata_by_function_metadata_id(function_id.to_string())
        {
            for dep in &dep_meta.dependencies {
                if let Ok(func) = bat_metadata
                    .source_code
                    .get_function_by_id(dep.function_metadata_id.clone())
                {
                    result.push(func);
                    Self::collect_deps(&dep.function_metadata_id, bat_metadata, visited, result);
                }
            }
        }
    }

    pub fn get_entrypoint_names_from_program_lib(sorted: bool) -> Result<Vec<String>, ParserError> {
        Self::get_entrypoint_names_filtered(sorted, None)
    }

    pub fn get_entrypoint_names_filtered(
        sorted: bool,
        program_lib_path: Option<&str>,
    ) -> Result<Vec<String>, ParserError> {
        let config = BatConfig::get_config().change_context(ParserError)?;

        let lib_paths = match program_lib_path {
            Some(path) => vec![path.to_string()],
            None => {
                if config.program_lib_paths.is_empty() {
                    vec![config.program_lib_path.clone()]
                } else {
                    config.program_lib_paths.clone()
                }
            }
        };

        let mut entrypoints_names: Vec<String> = Vec::new();
        for lib_path in &lib_paths {
            let bat_sonar = BatSonar::new_from_path(
                lib_path,
                Some("#[program"),
                SonarResultType::Function,
            );
            entrypoints_names.extend(bat_sonar.results.iter().map(|ep| ep.name.clone()));
        }
        if sorted {
            entrypoints_names.sort();
        }
        Ok(entrypoints_names)
    }

    pub fn get_all_contexts_names() -> Vec<String> {
        let entrypoints_names = Self::get_entrypoint_names_from_program_lib(false).unwrap();

        entrypoints_names
            .into_iter()
            .map(|ep_name| Self::get_context_name(&ep_name).unwrap())
            .collect::<Vec<_>>()
    }

    pub fn get_context_name(entrypoint_name: &str) -> Result<String, ParserError> {
        let config = BatConfig::get_config().change_context(ParserError)?;
        let lib_paths = if config.program_lib_paths.is_empty() {
            vec![config.program_lib_path.clone()]
        } else {
            config.program_lib_paths.clone()
        };
        // Find the lib file that contains this entrypoint
        let mut lib_file = String::new();
        for lib_path in &lib_paths {
            let content = fs::read_to_string(lib_path).unwrap_or_default();
            if content.lines().any(|line| {
                if line.contains("pub fn") {
                    let function_name = line
                        .split('(')
                        .next()
                        .unwrap()
                        .split('<')
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .last()
                        .unwrap();
                    function_name == entrypoint_name.replace(".md", "")
                } else {
                    false
                }
            }) {
                lib_file = content;
                break;
            }
        }
        if lib_file.is_empty() {
            lib_file = fs::read_to_string(&lib_paths[0]).unwrap();
        }
        let lib_file_lines: Vec<&str> = lib_file.lines().collect();
        let entrypoint_index = lib_file
            .lines()
            .position(|line| {
                if line.contains("pub fn") {
                    let function_name = line
                        .split('(')
                        .next()
                        .unwrap()
                        .split('<')
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .last()
                        .unwrap();
                    function_name == entrypoint_name.replace(".md", "")
                } else {
                    false
                }
            })
            .unwrap();
        let canditate_lines = [
            lib_file_lines[entrypoint_index],
            lib_file_lines[entrypoint_index + 1],
        ];

        // if is not in the same line as the entrypoint name, is in the next line
        let context_line = if canditate_lines[0].contains("Context<") {
            canditate_lines[0]
        } else {
            canditate_lines[1]
        };

        // replace all the extra strings to get the Context name
        let parsed_context_name = context_line
            .replace("'_, ", "")
            .replace("'info, ", "")
            .replace("<'info>", "")
            .split("Context<")
            .map(|l| l.to_string())
            .collect::<Vec<String>>()[1]
            .split('>')
            .map(|l| l.to_string())
            .collect::<Vec<String>>()[0]
            .clone();
        Ok(parsed_context_name)
    }
}
