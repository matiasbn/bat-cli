use crate::batbelt;
use crate::batbelt::helpers::get::get_all_rust_files_from_program_path;
use crate::batbelt::metadata::functions_metadata::{
    get_function_body, get_function_parameters, FunctionMetadata, FunctionMetadataType,
};

use crate::batbelt::metadata::structs_metadata::{StructMetadata, StructMetadataType};
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::batbelt::structs::FileInfo;
use crate::config::BatConfig;
use colored::Colorize;
use error_stack::{Report, Result, ResultExt};
use std::fs;
use std::path::Path;

use crate::batbelt::metadata::BatMetadataType;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct EntrypointParserError;

impl fmt::Display for EntrypointParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Command line error")
    }
}

impl Error for EntrypointParserError {}

#[derive(Clone)]
pub struct EntrypointParser {
    pub name: String,
    pub handler: Option<FunctionMetadata>,
    pub context_accounts: StructMetadata,
    pub entrypoint_function: FunctionMetadata,
}

impl EntrypointParser {
    pub fn new(
        name: String,
        handler: Option<FunctionMetadata>,
        context_accounts: StructMetadata,
        entrypoint_function: FunctionMetadata,
    ) -> Self {
        Self {
            name,
            handler,
            context_accounts,
            entrypoint_function,
        }
    }

    pub fn new_from_name(entrypoint_name: &str) -> Result<Self, EntrypointParserError> {
        BatMetadataType::Structs
            .check_is_initialized()
            .change_context(EntrypointParserError)?;
        BatMetadataType::Functions
            .check_is_initialized()
            .change_context(EntrypointParserError)?;

        let functions_metadata = BatMetadataType::Functions
            .get_markdown_sections_from_metadata_file()
            .change_context(EntrypointParserError)?
            .into_iter()
            .map(|markdown_section| FunctionMetadata::from_markdown_section(markdown_section))
            .collect::<Vec<_>>();
        let entrypoint_function = functions_metadata
            .iter()
            .find(|function_metadata| {
                function_metadata.function_type == FunctionMetadataType::EntryPoint
                    && function_metadata.name == entrypoint_name
            })
            .unwrap();
        let entrypoint_content = entrypoint_function
            .to_source_code(None)
            .get_source_code_content();
        let entrypoint_function_body = get_function_body(&entrypoint_content);
        let handlers = functions_metadata
            .clone()
            .into_iter()
            .filter(|function_metadata| {
                function_metadata.function_type == FunctionMetadataType::Handler
            })
            .collect::<Vec<_>>();
        let context_name = Self::get_context_name(entrypoint_name).unwrap();
        let handler = handlers.into_iter().find(|function_metadata| {
            let function_source_code = function_metadata.to_source_code(None);
            let function_content = function_source_code.get_source_code_content();
            let function_parameters = get_function_parameters(function_content.clone());
            !function_parameters.is_empty()
                && function_parameters[0].contains("Context<")
                && function_parameters[0].contains(&context_name)
                && (entrypoint_function_body.contains(&function_metadata.name))
        });
        let structs_metadata = BatMetadataType::Structs
            .get_markdown_sections_from_metadata_file()
            .change_context(EntrypointParserError)?
            .into_iter()
            .map(|markdown_section| StructMetadata::from_markdown_section(markdown_section))
            .collect::<Vec<_>>();
        let context_accounts = structs_metadata
            .iter()
            .find(|struct_metadata| {
                struct_metadata.struct_type == StructMetadataType::ContextAccounts
                    && struct_metadata.name == context_name
            })
            .unwrap();
        Ok(Self {
            name: entrypoint_name.to_string(),
            handler: handler.clone(),
            context_accounts: context_accounts.clone(),
            entrypoint_function: entrypoint_function.clone(),
        })
    }

    pub fn get_entrypoints_names(sorted: bool) -> Result<Vec<String>, EntrypointParserError> {
        let BatConfig {
            program_lib_path, ..
        } = BatConfig::get_config().change_context(EntrypointParserError)?;

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
        let entrypoints_names = Self::get_entrypoints_names(false).unwrap();
        let context_accounts_names = entrypoints_names
            .into_iter()
            .map(|ep_name| Self::get_context_name(&ep_name).unwrap())
            .collect::<Vec<_>>();
        context_accounts_names
    }

    pub fn get_instruction_file_path_with_prompts(
        entrypoint_name: &str,
    ) -> Result<String, EntrypointParserError> {
        let instruction_files_info =
            get_all_rust_files_from_program_path().change_context(EntrypointParserError)?;

        let instruction_match = instruction_files_info
            .iter()
            .filter(|ifile| ifile.name.replace(".rs", "") == entrypoint_name)
            .collect::<Vec<&FileInfo>>();

        // if instruction exists, prompt the user if the file is correct
        let is_match = if instruction_match.len() == 1 {
            let instruction_match_path = Path::new(&instruction_match[0].path);
            let prompt_text = format!(
                "{}  <--- is this the correct instruction file for {}?:",
                instruction_match_path.to_str().unwrap().yellow(),
                entrypoint_name.green()
            );
            let correct_path = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
            correct_path
        } else {
            false
        };

        let instruction_file_path = if is_match {
            instruction_match[0].path.clone()
        } else {
            let prompt_text = format!("Select the instruction file for {}: ", entrypoint_name);
            let instruction_files_names = instruction_files_info
                .iter()
                .map(|f| f.name.clone())
                .collect::<Vec<String>>();
            let selection =
                batbelt::cli_inputs::select(&prompt_text, instruction_files_names, None).unwrap();
            let name = instruction_files_info.as_slice()[selection].path.clone();
            name
        };
        Ok(instruction_file_path.clone())
    }

    // fn assert_file_match_entrypoint_name()

    pub fn get_context_name(entrypoint_name: &str) -> Result<String, EntrypointParserError> {
        let BatConfig {
            program_lib_path, ..
        } = BatConfig::get_config().change_context(EntrypointParserError)?;
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
