use error_stack::{IntoReport, Result, ResultExt};
use inflector::Inflector;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::batbelt::git::GitAction;
use crate::batbelt::metadata::context_accounts_metadata::ContextAccountsMetadata;
use crate::batbelt::metadata::structs_source_code_metadata::StructMetadataType;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser, SourceCodeMetadata};
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::parser::function_parser::FunctionParser;
use crate::batbelt::parser::solana_account_parser::{SolanaAccountParser, SolanaAccountType};
use crate::batbelt::parser::ParserResult;
use crate::batbelt::path::BatFile;
use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::batbelt::templates::code_overhaul_template::CoderOverhaulTemplatePlaceholders::{
    CompleteWithNotes, CompleteWithTheRestOfStateChanges,
};
use crate::batbelt::templates::{TemplateError, TemplateResult};
use crate::batbelt::{BatEnumerator, ShareableData};
use crate::config::BatConfig;

pub struct CodeOverhaulTemplate {
    pub entrypoint_name: String,
    pub entrypoint_parser: Option<EntrypointParser>,
}

impl CodeOverhaulTemplate {
    pub fn new(entrypoint_name: &str, initialized: bool) -> Result<Self, TemplateError> {
        let entrypoint_parser = if initialized {
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

    pub fn get_markdown_content(&self) -> TemplateResult<String> {
        let state_changes_content = CodeOverhaulSection::StateChanges
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let notes_content = CodeOverhaulSection::Notes
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let signers_content = CodeOverhaulSection::Signers
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let function_parameters_content = CodeOverhaulSection::HandlerFunctionParameters
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let context_accounts_content = CodeOverhaulSection::ContextAccounts
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let validations_content = CodeOverhaulSection::Validations
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;
        let miro_frame_url_content = CodeOverhaulSection::MiroFrameUrl
            .get_section_content_for_start_co_file(self.entrypoint_parser.clone())?;

        Ok(format!(
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
        ))
    }
}

#[derive(
    Default,
    Debug,
    Serialize,
    Deserialize,
    Clone,
    strum_macros::EnumIter,
    strum_macros::Display,
    PartialOrd,
    PartialEq,
)]
pub enum CodeOverhaulSection {
    #[default]
    StateChanges,
    Notes,
    Signers,
    HandlerFunctionParameters,
    ContextAccounts,
    Validations,
    MiroFrameUrl,
}

impl BatEnumerator for CodeOverhaulSection {}

impl CodeOverhaulSection {
    pub fn to_markdown_header(&self) -> String {
        format!("# {}:", self.to_string().to_sentence_case())
    }

    pub fn to_title(&self) -> String {
        format!("{}:", self.to_string().to_sentence_case())
    }

    pub fn get_section_content_for_start_co_file(
        &self,
        ep_parser: Option<EntrypointParser>,
    ) -> TemplateResult<String> {
        let section_content = if ep_parser.is_some() {
            let entrypoint_parser = ep_parser.unwrap();
            match self {
                CodeOverhaulSection::StateChanges => {
                    self.get_state_changes_content(entrypoint_parser)?
                }
                CodeOverhaulSection::Notes => self.get_notes_content(entrypoint_parser)?,
                CodeOverhaulSection::Signers => self.get_signers_section_content(entrypoint_parser),
                CodeOverhaulSection::HandlerFunctionParameters => {
                    self.get_handler_function_parameters_section_content(entrypoint_parser)?
                }
                CodeOverhaulSection::ContextAccounts => {
                    self.get_context_account_section_content(entrypoint_parser)
                }
                CodeOverhaulSection::Validations => {
                    self.get_validations_section_content(entrypoint_parser)?
                }
                CodeOverhaulSection::MiroFrameUrl => {
                    CoderOverhaulTemplatePlaceholders::CompleteWithMiroFrameUrl.to_placeholder()
                }
            }
        } else {
            "".to_string()
        };

        Ok(format!(
            "{}\n\n{}",
            self.to_markdown_header(),
            section_content
        ))
    }

    fn get_notes_content(&self, entry_point_parser: EntrypointParser) -> TemplateResult<String> {
        let context_accounts_struct_source_code_metadata_id =
            entry_point_parser.context_accounts.metadata_id;
        let context_accounts_metadata =
            ContextAccountsMetadata::find_context_accounts_metadata_by_struct_metadata_id(
                context_accounts_struct_source_code_metadata_id,
            )
            .change_context(TemplateError)?;
        let context_accounts_sc_metadata = SourceCodeMetadata::find_struct(
            context_accounts_metadata.name.clone(),
            StructMetadataType::ContextAccounts,
        )
        .change_context(TemplateError)?;
        let ca_sc_metadata_file_content = BatFile::Generic {
            file_path: context_accounts_sc_metadata.path.clone(),
        }
        .read_content(false)
        .change_context(TemplateError)?;
        let ca_sc_file_content_lines = ca_sc_metadata_file_content.lines();
        let ca_info_with_validation = context_accounts_metadata
            .context_accounts_info
            .clone()
            .into_iter()
            .filter_map(|ca_info| {
                if !ca_info.validations.is_empty() {
                    Some(ca_info.validations.clone())
                } else {
                    None
                }
            });
        if ca_info_with_validation.clone().count() == 0 {
            return Ok(format!("- {}", CompleteWithNotes.to_placeholder()));
        }

        let mut result = vec![];
        result.push(format!("- [ ] check validations:"));
        for ca_info_validations_vec in ca_info_with_validation.clone() {
            for ca_info_validation in ca_info_validations_vec.clone() {
                let validation_line = ca_sc_file_content_lines
                    .clone()
                    .position(|line| line.contains(&ca_info_validation))
                    .ok_or(TemplateError)
                    .into_report()?;
                let shared_permalink = ShareableData::new(String::new());
                GitAction::GetRepositoryPermalink {
                    file_path: context_accounts_sc_metadata.path.clone(),
                    start_line_index: validation_line + 1,
                    permalink: shared_permalink.original,
                }
                .execute_action()
                .change_context(TemplateError)?;
                // let permalink = format!("{}", &*shared_permalink.cloned.borrow());
                result.push(format!(
                    "  - [ ] [{}]({})",
                    ca_info_validation,
                    *shared_permalink.cloned.borrow()
                ));
            }
        }
        result.push(format!("- {}", CompleteWithNotes.to_placeholder()));
        Ok(result.join("\n"))
    }

    fn get_state_changes_content(
        &self,
        entry_point_parser: EntrypointParser,
    ) -> TemplateResult<String> {
        let bat_metadata = BatMetadata::read_metadata().change_context(TemplateError)?;
        let mut state_changes_content_vec = vec![];
        let context_accounts_metadata = bat_metadata
            .get_context_accounts_metadata_by_struct_source_code_metadata_id(
                entry_point_parser.context_accounts.metadata_id,
            )
            .change_context(TemplateError)?;

        let init_accounts = context_accounts_metadata
            .context_accounts_info
            .clone()
            .into_iter()
            .filter(|ca_info| ca_info.is_init)
            .collect::<Vec<_>>();
        for acc in init_accounts {
            state_changes_content_vec.push(format!(
                "- Initializes `{}`[{}], funded by `{}`",
                acc.account_name, acc.account_struct_name, acc.rent_exemption_account
            ))
        }

        let close_accounts = context_accounts_metadata
            .context_accounts_info
            .clone()
            .into_iter()
            .filter(|ca_info| ca_info.is_close)
            .collect::<Vec<_>>();
        for acc in close_accounts {
            state_changes_content_vec.push(format!(
                "- Closes `{}`[{}]. Rent exemption goes to `{}`",
                acc.account_name, acc.account_struct_name, acc.rent_exemption_account
            ))
        }

        let mut_program_state_accounts = context_accounts_metadata
            .clone()
            .context_accounts_info
            .into_iter()
            .filter(|ca_info| {
                ca_info.is_mut
                    && !ca_info.is_close
                    && !ca_info.is_init
                    && ca_info.solana_account_type == SolanaAccountType::ProgramStateAccount
            });

        for mut_program_state_acc in mut_program_state_accounts {
            let solana_acc_parser =
                SolanaAccountParser::new_from_struct_name_and_solana_account_type(
                    mut_program_state_acc.clone().account_struct_name,
                    mut_program_state_acc.clone().solana_account_type,
                )
                .change_context(TemplateError)?;
            state_changes_content_vec.push(format!(
                "- Updates `{}`[{}]:\n{}",
                mut_program_state_acc.clone().account_name,
                mut_program_state_acc.clone().account_struct_name,
                solana_acc_parser
                    .accounts
                    .clone()
                    .into_iter()
                    .map(|acc_parser| format!(
                        "\t- `{}.{}`[{}]",
                        mut_program_state_acc.clone().account_name,
                        acc_parser.account_name,
                        acc_parser.account_type
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        }

        let mut_unchecked_accounts = context_accounts_metadata
            .clone()
            .context_accounts_info
            .into_iter()
            .filter(|ca_info| {
                ca_info.is_mut && ca_info.solana_account_type == SolanaAccountType::UncheckedAccount
            });
        for mut_unchecked_account in mut_unchecked_accounts {
            state_changes_content_vec.push(format!(
                "- Updates `{}`[{}]",
                mut_unchecked_account.clone().account_name,
                mut_unchecked_account.clone().account_struct_name,
            ));
        }

        let mut_token_accounts = context_accounts_metadata
            .clone()
            .context_accounts_info
            .into_iter()
            .filter(|ca_info| {
                ca_info.is_mut && ca_info.solana_account_type == SolanaAccountType::TokenAccount
            })
            .collect::<Vec<_>>();

        for (mut_token_account_index, mut_token_account) in
            mut_token_accounts.clone().into_iter().enumerate()
        {
            let mut destination_index = 0;
            while destination_index < mut_token_accounts.len() {
                if destination_index == mut_token_account_index {
                    destination_index += 1;
                    continue;
                }
                state_changes_content_vec.push(format!(
                    "- Transfers `{}` tokens from `{}` to `{}`",
                    CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                    mut_token_account.clone().account_name,
                    mut_token_accounts.clone()[destination_index].account_name,
                ));
                destination_index += 1;
            }
            state_changes_content_vec.push(format!(
                "- Transfers `{}` tokens from `{}` to `{}`",
                CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                mut_token_account.clone().account_name,
                CoderOverhaulTemplatePlaceholders::CompleteWithDestinationTokenAccount
                    .to_placeholder(),
            ));

            destination_index = 0;

            while destination_index < mut_token_accounts.len() {
                if destination_index == mut_token_account_index {
                    destination_index += 1;
                    continue;
                }
                state_changes_content_vec.push(format!(
                    "- Delegates `{}` tokens from `{}` to `{}`",
                    CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                    mut_token_account.clone().account_name,
                    mut_token_accounts.clone()[destination_index].account_name,
                ));
                destination_index += 1;
            }
            state_changes_content_vec.push(format!(
                "- Delegates `{}` tokens from `{}` to `{}`",
                CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                mut_token_account.clone().account_name,
                CoderOverhaulTemplatePlaceholders::CompleteWithDestinationTokenAccount
                    .to_placeholder(),
            ));
        }

        let mut_mint_accounts = context_accounts_metadata
            .clone()
            .context_accounts_info
            .into_iter()
            .filter(|ca_info| {
                ca_info.is_mut && ca_info.solana_account_type == SolanaAccountType::Mint
            })
            .collect::<Vec<_>>();

        for mut_mint_account in mut_mint_accounts {
            for mut_token_account in mut_token_accounts.clone() {
                state_changes_content_vec.push(format!(
                    "- Mints `{}` tokens from `{}` mint to `{}`",
                    CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                    mut_mint_account.clone().account_name,
                    mut_token_account.clone().account_name,
                ));
            }
            state_changes_content_vec.push(format!(
                "- Mints `{}` tokens from `{}` mint to `{}`",
                CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                mut_mint_account.clone().account_name,
                CoderOverhaulTemplatePlaceholders::CompleteWithDestinationTokenAccount
                    .to_placeholder(),
            ));
            for mut_token_account in mut_token_accounts.clone() {
                state_changes_content_vec.push(format!(
                    "- Burns `{}` tokens from `{}` mint to `{}`",
                    CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                    mut_mint_account.clone().account_name,
                    mut_token_account.clone().account_name,
                ));
            }
            state_changes_content_vec.push(format!(
                "- Burns `{}` tokens from `{}` mint to `{}`",
                CoderOverhaulTemplatePlaceholders::CompleteWithAmount.to_placeholder(),
                mut_mint_account.clone().account_name,
                CoderOverhaulTemplatePlaceholders::CompleteWithDestinationTokenAccount
                    .to_placeholder(),
            ));
        }

        state_changes_content_vec.push(format!(
            "- `{}`",
            CompleteWithTheRestOfStateChanges.to_placeholder()
        ));
        Ok(state_changes_content_vec.join("\n"))
    }

    fn get_validations_section_content(
        &self,
        entrypoint_parser: EntrypointParser,
    ) -> TemplateResult<String> {
        log::debug!(
            "get_validations_section_content entrypoint_parser \n{:#?}",
            entrypoint_parser
        );
        if entrypoint_parser.handler.is_none() {
            return Ok(format!(
                "- {}",
                CoderOverhaulTemplatePlaceholders::NoValidationsDetected.to_placeholder()
            ));
        }
        let handler_function = entrypoint_parser.handler.unwrap();
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
        log::debug!("if_validations:\n{:#?}", if_validations);
        // any if that contains an if validation is considered a validation
        let mut filtered_if_validations = handler_if_validations
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
        log::debug!(
            "filtered_if_validations:\n{:#?}",
            filtered_if_validations.clone()
        );

        let handler_validations =
            BatSonar::new_from_path(&instruction_file_path, None, SonarResultType::Validation);
        log::debug!("handler_validations:\n{:#?}", handler_validations);

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
        log::debug!(
            "filtered_handler_validations:\n{:#?}",
            filtered_handler_validations.clone()
        );
        let bat_metadata = BatMetadata::read_metadata().change_context(TemplateError)?;
        let context_accounts_metadata = bat_metadata
            .get_context_accounts_metadata_by_struct_source_code_metadata_id(
                entrypoint_parser.context_accounts.metadata_id,
            )
            .change_context(TemplateError)?;

        log::debug!(
            "context_accounts_metadata:\n{:#?}",
            context_accounts_metadata
        );

        let mut ca_accounts_results = context_accounts_metadata
            .context_accounts_info
            .into_iter()
            .filter_map(|ca_metadata| {
                if !ca_metadata.validations.is_empty() {
                    let last_line = ca_metadata.content.lines().last().unwrap();
                    let last_line_tws = BatSonar::get_trailing_whitespaces(last_line);
                    let trailing_str = " ".repeat(last_line_tws);
                    let result = format!(
                        "{}#[account(\n{}\n{})]\n{}",
                        trailing_str,
                        ca_metadata
                            .validations
                            .into_iter()
                            .map(|validation| {
                                format!(
                                    "{}\t{},",
                                    trailing_str.clone(),
                                    validation.trim_end_matches(',')
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        trailing_str,
                        last_line
                    );
                    Some(result)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        log::debug!("ca_accounts_results:\n{:#?}", ca_accounts_results.clone());

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
        Ok(validations_content)
    }

    fn get_signers_section_content(&self, entrypoint_parser: EntrypointParser) -> String {
        let context_source_code = entrypoint_parser
            .context_accounts
            .to_source_code_parser(None);
        let context_lines = context_source_code.get_source_code_content();
        // signer names is only the name of the signer
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
            let signer_name = signer_name.split(':').next().unwrap();
            // delete last line
            content.pop().unwrap();
            let signer_comments = content
                .iter()
                .filter(|line| {
                    line.clone()
                        .trim()
                        .split(' ')
                        .next()
                        .unwrap()
                        .contains("//")
                })
                .collect::<Vec<_>>();
            if signer_comments.is_empty() {
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
                let signer_description =
                    format!("- {}: {}", signer_name, signer_description_comment);
                signers.push(signer_description);
                // multiple line description
            } else {
                let signer_formatted = signer_comments
                    .iter()
                    .map(|line| line.split("// ").last().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join(". ");
                let signer_description = format!("- {}: {}", signer_name, signer_formatted);
                signers.push(signer_description);
            }
        }
        if signers.is_empty() {
            return format!(
                "{}",
                CoderOverhaulTemplatePlaceholders::PermissionlessFunction.to_placeholder(),
            );
        }
        signers.join("\n")
    }

    fn get_context_account_section_content(&self, entrypoint_parser: EntrypointParser) -> String {
        let context_accounts_source_code = entrypoint_parser
            .context_accounts
            .to_source_code_parser(None);
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
            accounts_string.trim_start_matches('\n'),
            last_line,
        );
        let formatted = context_filtered
            .lines()
            .map(|line| format!("  {}", line))
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}\n{}\n{}", "- ```rust", formatted, "  ```")
    }

    fn get_handler_function_parameters_section_content(
        &self,
        entrypoint_parser: EntrypointParser,
    ) -> TemplateResult<String> {
        if entrypoint_parser.handler.is_none() {
            return Ok(format!(
                "{}",
                CoderOverhaulTemplatePlaceholders::NoHandlerFunctionParametersDetected
                    .to_placeholder()
            ));
        }
        let handler_function = entrypoint_parser.handler.unwrap();
        let handler_function_parser =
            FunctionParser::new_from_metadata(handler_function).change_context(TemplateError)?;
        let filtered_parameters = handler_function_parser
            .parameters
            .into_iter()
            .filter(|parameter| !parameter.parameter_type.contains("Context<"))
            .collect::<Vec<_>>();
        let function_parameters_content = if filtered_parameters.is_empty() {
            format!(
                "{}",
                CoderOverhaulTemplatePlaceholders::NoHandlerFunctionParametersDetected
                    .to_placeholder()
            )
        } else {
            let mut parameters = vec![];
            for parameter in filtered_parameters {
                parameters.push(format!(
                    "- {}: {}",
                    parameter.parameter_name,
                    parameter.parameter_type.trim_end_matches(',')
                ));
                if let Ok(struct_metadata) = SourceCodeMetadata::find_struct(
                    parameter.parameter_type.trim_start_matches("&").to_string(),
                    StructMetadataType::Other,
                ) {
                    parameters.push(format!(
                        "- ```rust\n{}\n  ```",
                        struct_metadata
                            .to_source_code_parser(None)
                            .get_source_code_content()
                            .lines()
                            .map(|line| format!("  {line}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ));
                }
            }
            parameters.join("\n")
        };
        Ok(function_parameters_content)
    }
}

#[derive(strum_macros::Display)]
pub enum CoderOverhaulTemplatePlaceholders {
    PermissionlessFunction,
    NoValidationsDetected,
    NoHandlerFunctionParametersDetected,
    CompleteWithTheRestOfStateChanges,
    CompleteWithNotes,
    CompleteWithSignerDescription,
    CompleteWithMiroFrameUrl,
    CompleteWithDestinationTokenAccount,
    CompleteWithAmount,
}

impl CoderOverhaulTemplatePlaceholders {
    pub fn to_placeholder(&self) -> String {
        self.to_string().to_screaming_snake_case()
    }
    pub fn get_state_changes_checked_placeholders_vec() -> Vec<String> {
        vec![
            Self::CompleteWithTheRestOfStateChanges.to_placeholder(),
            Self::CompleteWithAmount.to_placeholder(),
            Self::CompleteWithDestinationTokenAccount.to_placeholder(),
        ]
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
