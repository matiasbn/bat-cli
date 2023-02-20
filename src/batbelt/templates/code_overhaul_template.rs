use crate::batbelt;
use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::functions_metadata::get_function_parameters;
use crate::batbelt::metadata::BatMetadata;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;

use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::code_overhaul_template::CoderOverhaulTemplatePlaceholders::{
    CompleteWithNotes, CompleteWithStateChanges,
};
use crate::batbelt::templates::TemplateError;
use colored::Colorize;
use error_stack::{Result, ResultExt};
use inflector::Inflector;

pub struct CodeOverhaulTemplate {
    pub entrypoint_name: String,
    pub entrypoint_parser: Option<EntrypointParser>,
}

impl CodeOverhaulTemplate {
    pub fn new(entrypoint_name: &str, initialized: bool) -> Result<Self, TemplateError> {
        let entrypoint_parser = if initialized {
            BatMetadata::metadata_is_initialized().change_context(TemplateError)?;
            let ep_parser =
                EntrypointParser::new_from_name(entrypoint_name).change_context(TemplateError)?;
            Some(ep_parser)
        } else {
            None
        };
        Ok(Self {
            entrypoint_name: entrypoint_name.to_string(),
            entrypoint_parser,
        })
    }
    pub fn to_markdown_file(&self, file_path: &str) -> Result<MarkdownFile, TemplateError> {
        let content = self.get_markdown_content().clone();
        let template = MarkdownFile::new_from_path_and_content(&file_path, content);
        Ok(template)
    }

    fn get_markdown_content(&self) -> String {
        let state_changes_content =
            CodeOverhaulSection::StateChanges.get_section_content(self.entrypoint_parser.clone());
        let notes_content =
            CodeOverhaulSection::Notes.get_section_content(self.entrypoint_parser.clone());
        let signers_content =
            CodeOverhaulSection::Signers.get_section_content(self.entrypoint_parser.clone());
        let function_parameters_content = CodeOverhaulSection::FunctionParameters
            .get_section_content(self.entrypoint_parser.clone());
        let context_accounts_content = CodeOverhaulSection::ContextAccounts
            .get_section_content(self.entrypoint_parser.clone());
        let validations_content =
            CodeOverhaulSection::Validations.get_section_content(self.entrypoint_parser.clone());
        let miro_frame_url_content =
            CodeOverhaulSection::MiroFrameUrl.get_section_content(self.entrypoint_parser.clone());

        format!(
            "{state_changes_content}\
            \n\
            \n\
            {notes_content}\
            \n\
            \n\
            {signers_content}\
            \n\
            \n\
            {function_parameters_content}\
            \n\
            \n\
            {context_accounts_content}\
            \n\
            \n\
            {validations_content}\
            \n\
            \n\
            {miro_frame_url_content}
            ",
        )
    }

    fn get_mut_accounts(results: Vec<SonarResult>) -> Vec<Vec<String>> {
        let mut_accounts_results = results
            .iter()
            .filter(|account| {
                account.content.contains("#[account(") && account.content.contains("mut")
            })
            .collect::<Vec<_>>();
        let mut result_vec: Vec<Vec<String>> = vec![];
        for mut_account_result in mut_accounts_results {
            let content_lines = mut_account_result.content.lines().clone();
            let account_name = mut_account_result.name.clone();
            let prefix = format!("pub {}: ", account_name);
            let mut is_mut = false;
            if content_lines.count() == 2 {
                let first_line = mut_account_result.content.lines().next().unwrap();
                let first_line = first_line
                    .trim()
                    .trim_start_matches("#[account(")
                    .trim_end_matches(")]");
                is_mut = first_line.split(",").any(|spl| spl.trim() == "mut");
            } else {
                is_mut = mut_account_result
                    .content
                    .lines()
                    .any(|line| line.trim().trim_end_matches(",") == "mut");
            };
            if is_mut {
                let last_line = mut_account_result
                    .content
                    .lines()
                    .last()
                    .unwrap()
                    .trim_end_matches(">,");
                let account_definition = last_line.trim().strip_prefix(&prefix).unwrap();
                let lifetime = account_definition.clone().split("<").last().unwrap();
                let lifetime_split = lifetime.split(" ").collect::<Vec<_>>();
                let account_type = if lifetime_split.len() > 1 {
                    lifetime_split.last().unwrap().to_string()
                } else {
                    account_definition.split("<").next().unwrap().to_string()
                };
                result_vec.push(vec![account_name, account_type]);
            }
        }
        result_vec
    }
}

#[derive(strum_macros::Display)]
pub enum CodeOverhaulSection {
    StateChanges,
    Notes,
    Signers,
    FunctionParameters,
    ContextAccounts,
    Validations,
    MiroFrameUrl,
}

impl CodeOverhaulSection {
    pub fn to_markdown_header(&self) -> String {
        format!("# {}:", self.to_string().to_sentence_case())
    }

    pub fn to_title(&self) -> String {
        format!("{}:", self.to_string().to_sentence_case())
    }

    pub fn get_section_content(&self, ep_parser: Option<EntrypointParser>) -> String {
        let section_content = if ep_parser.is_some() {
            let entrypoint_parser = ep_parser.unwrap();
            match self {
                CodeOverhaulSection::StateChanges => {
                    format!("- {}", CompleteWithStateChanges.to_placeholder())
                }
                CodeOverhaulSection::Notes => format!("- {}", CompleteWithNotes.to_placeholder()),
                CodeOverhaulSection::Signers => self.get_signers_section_content(entrypoint_parser),
                CodeOverhaulSection::FunctionParameters => {
                    self.get_function_parameters_section_content(entrypoint_parser)
                }
                CodeOverhaulSection::ContextAccounts => {
                    self.get_context_account_section_content(entrypoint_parser)
                }
                CodeOverhaulSection::Validations => {
                    self.get_validations_section_content(entrypoint_parser)
                }
                CodeOverhaulSection::MiroFrameUrl => {
                    CoderOverhaulTemplatePlaceholders::CompleteWithMiroFrameUrl.to_placeholder()
                }
            }
        } else {
            "".to_string()
        };

        format!("{}\n\n{}", self.to_markdown_header(), section_content)
    }

    fn get_validations_section_content(&self, entrypoint_parser: EntrypointParser) -> String {
        if entrypoint_parser.handler.is_none() {
            return format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
            );
        }
        let handler_function = entrypoint_parser.handler.unwrap();
        let context_source_code = handler_function.to_source_code(None);
        let instruction_file_path = handler_function.path.clone();
        let handler_if_validations = BatSonar::new_from_path(
            &instruction_file_path,
            Some(&handler_function.name),
            SonarResultType::IfValidation,
        );

        // get the if validations inside any if statement to filter from the handler validations
        let if_validations = handler_if_validations
            .results
            .iter()
            .map(|if_validation| {
                let if_in_validations =
                    BatSonar::new_scanned(&if_validation.content, SonarResultType::Validation);
                if !if_in_validations.results.is_empty() {
                    if_in_validations.results
                } else {
                    vec![]
                }
            })
            .fold(vec![], |mut result, current| {
                for res in current {
                    result.push(res);
                }
                result
            });

        // any if that contains an if validation is considered a validation
        let mut filtered_if_validations = handler_if_validations
            .clone()
            .results
            .iter()
            .filter(|if_est| {
                if_validations
                    .clone()
                    .iter()
                    .any(|if_val| if_est.content.contains(&if_val.content))
            })
            .map(|result| result.content.clone())
            .collect::<Vec<_>>();

        let handler_validations =
            BatSonar::new_from_path(&instruction_file_path, None, SonarResultType::Validation);

        // if there are validations in if_validations, then filter them from handler validations to avoid repetition
        let mut filtered_handler_validations = if if_validations.is_empty() {
            handler_validations
                .results
                .iter()
                .map(|result| result.content.clone())
                .collect::<Vec<_>>()
        } else {
            handler_validations
                .results
                .iter()
                .filter(|validation| {
                    !handler_if_validations
                        .results
                        .iter()
                        .any(|if_val| if_val.content.contains(&validation.content.to_string()))
                })
                .map(|val| val.content.clone())
                .collect::<Vec<_>>()
        };

        let ca_accounts_only_validations = BatSonar::new_scanned(
            &context_source_code.clone().get_source_code_content(),
            SonarResultType::ContextAccountsOnlyValidation,
        );

        let mut ca_accounts_results = ca_accounts_only_validations
            .results
            .iter()
            .map(|result| result.content.clone())
            .collect::<Vec<_>>();

        let mut validations_vec: Vec<String> = vec![];
        validations_vec.append(&mut ca_accounts_results);
        validations_vec.append(&mut filtered_if_validations);
        validations_vec.append(&mut filtered_handler_validations);

        let validations_content = if validations_vec.is_empty() {
            format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
            )
        } else {
            validations_vec
                .iter()
                .map(|validation| format!("- ```rust\n{}\n  ```", validation))
                .collect::<Vec<_>>()
                .join("\n")
        };
        validations_content
    }

    fn get_signers_section_content(&self, entrypoint_parser: EntrypointParser) -> String {
        let context_source_code = entrypoint_parser.context_accounts.to_source_code(None);
        let context_lines = context_source_code.get_source_code_content();
        // signer names is only the name of the signer
        // let signers_names = get_signers_description_from_co_file(&context_lines);
        let mut signers: Vec<String> = vec![];
        for (line_index, line) in context_lines.lines().enumerate() {
            if !line.contains("pub") {
                continue;
            }
            let next_pub_line = context_lines
                .lines()
                .enumerate()
                .position(|line| {
                    line.0 > line_index && line.1.contains("pub")
                        || line.0 == context_lines.lines().count() - 1
                })
                .unwrap();
            let mut content =
                context_lines.lines().collect::<Vec<_>>()[line_index + 1..=next_pub_line].to_vec();
            let has_signer = content.clone().last().unwrap().contains("Signer<");
            if !has_signer {
                continue;
            }
            let signer_name = content.clone().last().unwrap().trim().replace("pub ", "");
            let signer_name = signer_name.split(":").next().unwrap();
            // delete last line
            content.pop().unwrap();
            let signer_comments = content
                .iter()
                .filter(|line| {
                    line.clone()
                        .trim()
                        .split(" ")
                        .next()
                        .unwrap()
                        .contains("//")
                })
                .collect::<Vec<_>>();
            if signer_comments.len() == 0 {
                let signer_description = format!(
                    "- {}: {}",
                    signer_name,
                    CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription
                        .to_placeholder()
                );
                signers.push(signer_description)
            } else if signer_comments.len() == 1 {
                // prompt the user to state if the comment is correct
                let signer_description_comment = signer_comments[0].split("// ").last().unwrap();
                let prompt_text = format!(
                    "is this a proper description of the signer {}?: '{}'",
                    signer_name.red(),
                    signer_description_comment
                );
                let is_correct = batbelt::cli_inputs::select_yes_or_no(&prompt_text).unwrap();
                if is_correct {
                    let signer_description =
                        format!("- {}: {}", signer_name, signer_description_comment);
                    signers.push(signer_description);
                } else {
                    let signer_description = format!(
                        "- {}: {}",
                        signer_name,
                        CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription
                            .to_placeholder()
                    );
                    signers.push(signer_description);
                }
                // multiple line description
            } else {
                // prompt the user to select the lines that contains the description and join them
                let prompt_text = format!(
                    "Use the spacebar to select the lines that describes the signer {}.
                        Hit enter if is not a proper description:",
                    signer_name.red()
                );
                let signer_formatted: Vec<&str> = signer_comments
                    .iter()
                    .map(|line| line.split("// ").last().unwrap())
                    .collect();
                let selections = batbelt::cli_inputs::multiselect(
                    &prompt_text,
                    signer_formatted.clone(),
                    Some(&vec![false; signer_formatted.clone().len()]),
                )
                .unwrap();
                if selections.is_empty() {
                    let signer_description = format!(
                        "- {}: {}",
                        signer_name,
                        CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription
                            .to_placeholder()
                    );
                    signers.push(signer_description);
                } else {
                    // take the selections and create the array
                    let signer_total_comment = signer_formatted
                        .into_iter()
                        .enumerate()
                        .filter(|line| selections.contains(&line.0))
                        .map(|line| line.1)
                        .collect::<Vec<_>>()
                        .join(". ");
                    let signer_description = format!("- {}: {}", signer_name, signer_total_comment);
                    signers.push(signer_description);
                }
            }
        }
        if signers.is_empty() {
            return format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoSignersDetected.to_placeholder(),
            );
        }
        signers.join("\n")
    }

    fn get_context_account_section_content(&self, entrypoint_parser: EntrypointParser) -> String {
        let context_accounts_source_code = entrypoint_parser.context_accounts.to_source_code(None);
        let context_accounts_content = context_accounts_source_code.get_source_code_content();
        let accounts = BatSonar::new_scanned(
            &context_accounts_content,
            SonarResultType::ContextAccountsNoValidation,
        );

        let accounts_string = accounts
            .results
            .iter()
            .fold("".to_string(), |result, next| {
                format!("{}\n\n{}", result, next.content)
            });
        let first_line = context_accounts_content.lines().next().unwrap();
        let last_line = context_accounts_content.lines().last().unwrap();
        let context_filtered = format!(
            "{}\n{}\n{}",
            first_line,
            accounts_string.trim_start_matches("\n"),
            last_line,
        );
        let formatted = context_filtered
            .lines()
            .map(|line| format!("  {}", line))
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}\n{}\n{}", "- ```rust", formatted, "  ```")
    }

    fn get_function_parameters_section_content(
        &self,
        entrypoint_parser: EntrypointParser,
    ) -> String {
        if entrypoint_parser.handler.is_none() {
            return format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoFunctionParametersDetected.to_placeholder()
            );
        }
        let handler_function = entrypoint_parser.handler.unwrap();
        let handler_function_parameters = get_function_parameters(
            handler_function
                .to_source_code(None)
                .get_source_code_content(),
        );
        let function_parameters_content = if handler_function_parameters.is_empty() {
            format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoFunctionParametersDetected.to_placeholder()
            )
        } else {
            handler_function_parameters
                .iter()
                .fold("".to_string(), |result, parameter| {
                    if parameter.contains("Context<") {
                        return result;
                    }
                    if result.is_empty() {
                        format!("- {}", parameter.trim_end_matches(","))
                    } else {
                        format!("{}\n- {}", result, parameter.trim_end_matches(","))
                    }
                })
        };
        function_parameters_content
    }
}

#[derive(strum_macros::Display)]
pub enum CoderOverhaulTemplatePlaceholders {
    NoSignersDetected,
    NoValidationsDetected,
    NoFunctionParametersDetected,
    CompleteWithStateChanges,
    CompleteWithNotes,
    CompleteWithSignerDescription,
    CompleteWithMiroFrameUrl,
}

impl CoderOverhaulTemplatePlaceholders {
    pub fn to_placeholder(&self) -> String {
        self.to_string().to_screaming_snake_case()
    }
}

#[test]
fn test_to_title() {
    let expected = "Signers:";
    let title = CodeOverhaulSection::Signers.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");

    let expected = "Context accounts:";
    let title = CodeOverhaulSection::ContextAccounts.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");

    let expected = "Validations:";
    let title = CodeOverhaulSection::Validations.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");
}
