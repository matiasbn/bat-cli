use crate::batbelt::metadata::functions_source_code_metadata::{
    get_function_body, get_function_parameters, FunctionSourceCodeMetadata, FunctionMetadataType,
};

use crate::batbelt::metadata::structs_source_code_metadata::{StructSourceCodeMetadata, StructMetadataType};
use crate::batbelt::sonar::{BatSonar, SonarResultType};

use crate::config::BatConfig;

use error_stack::{IntoReport, Report, Result, ResultExt};
use std::fs;

use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, BatMetadataType, MetadataId};

use crate::batbelt::parser::ParserError;

#[derive(Clone)]
pub struct EntrypointParser {
    pub name: String,
    pub handler: Option<FunctionSourceCodeMetadata>,
    pub context_accounts: StructSourceCodeMetadata,
    pub entry_point_function: FunctionSourceCodeMetadata,
}

impl EntrypointParser {
    pub fn new(
        name: String,
        handler: Option<FunctionSourceCodeMetadata>,
        context_accounts: StructSourceCodeMetadata,
        entry_point_function: FunctionSourceCodeMetadata,
    ) -> Self {
        Self {
            name,
            handler,
            context_accounts,
            entry_point_function,
        }
    }

    pub fn new_from_name(entrypoint_name: &str) -> Result<Self, ParserError> {
        BatMetadataType::Struct
            .check_is_initialized()
            .change_context(ParserError)?;
        BatMetadataType::Function
            .check_is_initialized()
            .change_context(ParserError)?;
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        if let Ok(ep_metadata) =
            bat_metadata.get_entrypoint_metadata_by_name(entrypoint_name.to_string())
        {
            let entry_point_function = bat_metadata
                .source_code
                .get_function_by_id(ep_metadata.entrypoint_function_id.clone())
                .change_context(ParserError)?;
            let handler = match ep_metadata.handler_id {
                None => None,
                Some(h_id) => Some(
                    bat_metadata
                        .source_code
                        .get_function_by_id(h_id)
                        .change_context(ParserError)?,
                ),
            };
            let context_accounts = bat_metadata
                .source_code
                .get_struct_by_id(ep_metadata.context_accounts_id.clone())
                .change_context(ParserError)?;
            return Ok(Self {
                name: ep_metadata.name,
                handler,
                context_accounts,
                entry_point_function,
            });
        };

        let entrypoint_section = FunctionSourceCodeMetadata::get_filtered_metadata(
            Some(entrypoint_name),
            Some(FunctionMetadataType::EntryPoint),
        )
        .change_context(ParserError)?;

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

        let entrypoint_content = entrypoint_function
            .to_source_code_parser(None)
            .get_source_code_content();
        let entrypoint_function_body = get_function_body(&entrypoint_content);

        let handlers =
            FunctionSourceCodeMetadata::get_filtered_metadata(None, Some(FunctionMetadataType::Handler))
                .change_context(ParserError)?;
        let context_name = Self::get_context_name(entrypoint_name).unwrap();

        let handler = handlers.into_iter().find(|function_metadata| {
            let function_source_code = function_metadata.to_source_code_parser(None);
            let function_content = function_source_code.get_source_code_content();
            let function_parameters = get_function_parameters(function_content);
            !function_parameters.is_empty()
                && function_parameters[0].contains("Context<")
                && function_parameters[0].contains(&context_name)
                && (entrypoint_function_body.contains(&function_metadata.name))
        });
        let structs_metadata = StructSourceCodeMetadata::get_filtered_metadata(
            Some(&context_name),
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
            handler_id: match handler.clone() {
                None => None,
                Some(handler_function) => Some(handler_function.metadata_id),
            },
            context_accounts_id: context_accounts.metadata_id.clone(),
            entrypoint_function_id: entrypoint_function.metadata_id.clone(),
            miro_frame_id: None,
        };

        ep_metadata
            .update_metadata_file()
            .change_context(ParserError)?;

        Ok(Self {
            name: entrypoint_name.to_string(),
            handler,
            context_accounts: context_accounts.clone(),
            entry_point_function: entrypoint_function,
        })
    }

    pub fn get_entrypoint_names(sorted: bool) -> Result<Vec<String>, ParserError> {
        let BatConfig {
            program_lib_path, ..
        } = BatConfig::get_config().change_context(ParserError)?;

        let bat_sonar = BatSonar::new_from_path(
            &program_lib_path,
            Some("#[program"),
            SonarResultType::Function,
        );
        let mut entrypoints_names: Vec<String> = bat_sonar
            .results
            .iter()
            .map(|entrypoint| entrypoint.name.clone())
            .collect();
        if sorted {
            entrypoints_names.sort();
        }
        Ok(entrypoints_names)
    }

    pub fn get_all_contexts_names() -> Vec<String> {
        let entrypoints_names = Self::get_entrypoint_names(false).unwrap();

        entrypoints_names
            .into_iter()
            .map(|ep_name| Self::get_context_name(&ep_name).unwrap())
            .collect::<Vec<_>>()
    }

    pub fn get_context_name(entrypoint_name: &str) -> Result<String, ParserError> {
        let BatConfig {
            program_lib_path, ..
        } = BatConfig::get_config().change_context(ParserError)?;
        let lib_file = fs::read_to_string(program_lib_path).unwrap();
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
        let canditate_lines = vec![
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
