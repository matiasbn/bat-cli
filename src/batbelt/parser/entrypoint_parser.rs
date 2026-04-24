use crate::batbelt::metadata::functions_source_code_metadata::{
    FunctionMetadataType, FunctionSourceCodeMetadata,
};

use crate::batbelt::metadata::structs_source_code_metadata::{
    StructMetadataType, StructSourceCodeMetadata,
};
use crate::batbelt::parser::syn_struct_classifier;
use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::config::{BatConfig, ProjectType};

use error_stack::{IntoReport, Report, Result, ResultExt};
use std::collections::HashSet;
use std::fs;

use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::{BatMetadata, SourceCodeMetadata};
use crate::batbelt::parser::function_parser::FunctionParser;

use crate::batbelt::parser::ParserError;

#[derive(Clone, Debug)]
pub struct EntrypointParser {
    pub name: String,
    pub program_name: String,
    pub dependencies: Vec<FunctionSourceCodeMetadata>,
    pub context_accounts: Option<StructSourceCodeMetadata>,
    pub entry_point_function: FunctionSourceCodeMetadata,
}

impl EntrypointParser {
    pub fn new(
        name: String,
        program_name: String,
        dependencies: Vec<FunctionSourceCodeMetadata>,
        context_accounts: Option<StructSourceCodeMetadata>,
        entry_point_function: FunctionSourceCodeMetadata,
    ) -> Self {
        Self {
            name,
            program_name,
            dependencies,
            context_accounts,
            entry_point_function,
        }
    }

    pub fn new_from_name(entrypoint_name: &str) -> Result<Self, ParserError> {
        Self::new_from_name_and_program(entrypoint_name, None)
    }

    pub fn new_from_name_and_program(
        entrypoint_name: &str,
        program_name: Option<&str>,
    ) -> Result<Self, ParserError> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        if let Ok(ep_metadata) =
            bat_metadata.get_entrypoint_metadata_by_name(entrypoint_name.to_string())
        {
            let entry_point_function = bat_metadata
                .source_code
                .get_function_by_id(ep_metadata.entrypoint_function_id.clone())
                .change_context(ParserError)?;
            let context_accounts = if ep_metadata.context_accounts_id.is_empty() {
                None
            } else {
                bat_metadata
                    .source_code
                    .get_struct_by_id(ep_metadata.context_accounts_id.clone())
                    .ok()
            };

            // Resolve dependencies recursively from the entrypoint function
            // First ensure the entrypoint function's dependencies are computed
            let _ = FunctionParser::new_from_metadata(entry_point_function.clone());
            let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
            let dependencies =
                Self::resolve_all_dependencies(&entry_point_function.metadata_id, &bat_metadata);

            return Ok(Self {
                name: ep_metadata.name,
                program_name: ep_metadata.program_name,
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
                    && program_name.is_none_or(|pn| {
                        func_meta.program_name.is_empty() || func_meta.program_name == pn
                    })
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
        let resolved_program_name = if let Some(pn) = program_name {
            pn.to_string()
        } else {
            entrypoint_function.program_name.clone()
        };

        let config = BatConfig::get_config().change_context(ParserError)?;
        let context_accounts = if config.project_type == ProjectType::Pinocchio {
            // For Pinocchio: find ContextAccounts struct in the same file as the entry point.
            // Returns None if the entry point has no associated context struct (e.g. emit_event).
            Self::find_pinocchio_context_accounts(&entrypoint_function).ok()
        } else {
            let context_name = Self::get_context_name(entrypoint_name).unwrap();
            let structs_metadata = SourceCodeMetadata::get_filtered_structs_by_program(
                Some(context_name.clone()),
                Some(StructMetadataType::ContextAccounts),
                if resolved_program_name.is_empty() {
                    None
                } else {
                    Some(&resolved_program_name)
                },
            )
            .change_context(ParserError)?;
            Some(
                structs_metadata
                    .iter()
                    .find(|struct_metadata| struct_metadata.name == context_name)
                    .ok_or(ParserError)
                    .into_report()
                    .attach_printable(format!(
                        "Error context_accounts struct by name {} for entrypoint_name: {}",
                        context_name, entrypoint_name
                    ))?
                    .clone(),
            )
        };

        let ep_metadata = EntrypointMetadata::new(
            entrypoint_name.to_string(),
            context_accounts
                .as_ref()
                .map(|ca| ca.metadata_id.clone())
                .unwrap_or_default(),
            entrypoint_function.metadata_id.clone(),
            BatMetadata::create_metadata_id(),
            resolved_program_name.clone(),
        );

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
            program_name: resolved_program_name,
            dependencies,
            context_accounts,
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

        // For Pinocchio (and other non-Anchor), entry points are already classified
        // per-file by syn_struct_classifier. Read them from BatMetadata.
        if config.project_type == ProjectType::Pinocchio {
            let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
            let mut entrypoints_names: Vec<String> = bat_metadata
                .source_code
                .functions_source_code
                .iter()
                .filter(|f| {
                    f.function_type == FunctionMetadataType::EntryPoint
                        && program_lib_path.is_none_or(|lib_path| {
                            // Match by program: derive program name from lib_path
                            let pn = lib_path
                                .trim_end_matches("/src/lib.rs")
                                .trim_end_matches("/src/main.rs")
                                .split('/')
                                .next_back()
                                .unwrap_or("");
                            f.program_name.is_empty() || f.program_name == pn
                        })
                })
                .map(|f| f.name.clone())
                .collect();
            if sorted {
                entrypoints_names.sort();
            }
            return Ok(entrypoints_names);
        }

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
            let classification = syn_struct_classifier::classify_file_from_path(lib_path);
            if !classification.entrypoint_function_names.is_empty() {
                entrypoints_names.extend(classification.entrypoint_function_names);
            } else {
                // Fallback to BatSonar if syn parsing fails
                let bat_sonar =
                    BatSonar::new_from_path(lib_path, Some("#[program"), SonarResultType::Function);
                entrypoints_names.extend(bat_sonar.results.iter().map(|ep| ep.name.clone()));
            }
        }
        if sorted {
            entrypoints_names.sort();
        }
        Ok(entrypoints_names)
    }

    /// For Pinocchio entry points: find the ContextAccounts struct used by the process function.
    /// Strategy:
    /// 1. Look for a ContextAccounts struct in the same file
    /// 2. If not found, parse the process function body for `SomeStruct::try_from(accounts)`
    ///    and find that struct in all ContextAccounts metadata
    fn find_pinocchio_context_accounts(
        entrypoint_function: &FunctionSourceCodeMetadata,
    ) -> Result<StructSourceCodeMetadata, ParserError> {
        let ep_path = &entrypoint_function.path;
        let all_ca = SourceCodeMetadata::get_filtered_structs(
            None,
            Some(StructMetadataType::ContextAccounts),
        )
        .change_context(ParserError)?;

        // Strategy 1: same file
        let same_file_ca: Vec<_> = all_ca.iter().filter(|s| s.path == *ep_path).collect();
        if let Some(ca) = same_file_ca.into_iter().next() {
            return Ok(ca.clone());
        }

        // Strategy 2: parse function body for `SomeStruct::try_from`
        let file_content = fs::read_to_string(ep_path).unwrap_or_default();
        if let Some(ca_name) = Self::extract_try_from_struct_name(&file_content) {
            if let Some(ca) = all_ca.into_iter().find(|s| s.name == ca_name) {
                return Ok(ca);
            }
        }

        Err(Report::new(ParserError).attach_printable(format!(
            "No ContextAccounts struct found for Pinocchio entry point {}",
            entrypoint_function.name
        )))
    }

    /// Extracts the struct name from `SomeStruct::try_from(accounts)` in a file.
    fn extract_try_from_struct_name(file_content: &str) -> Option<String> {
        // Look for pattern: SomeIdentifier::try_from(accounts
        // This is simpler and more reliable than full syn parsing of function bodies
        for line in file_content.lines() {
            let trimmed = line.trim();
            if let Some(pos) = trimmed.find("::try_from(accounts") {
                // Extract the identifier before ::try_from
                let before = &trimmed[..pos];
                // Get the last token (could be preceded by `let accounts =` etc.)
                let name = before
                    .split_whitespace()
                    .last()
                    .unwrap_or("")
                    .trim_start_matches('(');
                if !name.is_empty() && name.chars().next().unwrap().is_uppercase() {
                    return Some(name.to_string());
                }
            }
        }
        None
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
        let clean_name = entrypoint_name.replace(".md", "");

        // Try syn-based extraction first
        for lib_path in &lib_paths {
            let content = fs::read_to_string(lib_path).unwrap_or_default();
            if let Some(ctx_type) =
                syn_struct_classifier::get_context_type_for_entrypoint(&content, &clean_name)
            {
                return Ok(ctx_type);
            }
        }

        // Fallback: string matching
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
                    function_name == clean_name
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
                    function_name == clean_name
                } else {
                    false
                }
            })
            .unwrap();
        let canditate_lines = [
            lib_file_lines[entrypoint_index],
            lib_file_lines[entrypoint_index + 1],
        ];
        let context_line = if canditate_lines[0].contains("Context<") {
            canditate_lines[0]
        } else {
            canditate_lines[1]
        };
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
