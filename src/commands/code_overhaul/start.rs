use colored::Colorize;

use crate::batbelt::command_line::vs_code_open_file_in_current_window;

use crate::config::BatConfig;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};

use crate::batbelt::path::{FilePathType, FolderPathType};

use std::fs;

use crate::batbelt::bash::execute_command;

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::sonar::{get_function_parameters, BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::code_overhaul::{
    CodeOverhaulSection, CoderOverhaulTemplatePlaceholders,
};

use crate::batbelt::metadata::entrypoint::EntrypointMetadata;

use crate::batbelt::sonar::SonarResultType::ContextAccountsAll;
use std::string::String;

pub fn start_co_file() -> Result<(), String> {
    check_correct_branch()?;
    let bat_config = BatConfig::get_validated_config().unwrap();
    let to_review_path =
        batbelt::path::get_folder_path(FolderPathType::CodeOverhaulToReview, false);

    // get to-review files
    let mut review_files = fs::read_dir(to_review_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();
    review_files.sort();

    if review_files.is_empty() {
        panic!("no to-review files in code-overhaul folder");
    }
    let prompt_text = "Select the code-overhaul file to start:";
    let selection = batbelt::cli_inputs::select(prompt_text, review_files.clone(), None)?;

    // user select file
    let to_start_file_name = &review_files[selection].clone();
    let entrypoint_name = to_start_file_name.replace(".md", "");
    let to_review_file_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulToReview {
            file_name: to_start_file_name.clone(),
        },
        false,
    );

    let instruction_file_path =
        batbelt::helpers::get::get_instruction_file_with_prompts(&to_start_file_name)?;

    let program_lib_path = bat_config.required.program_lib_path;

    let entrypoint_functions = BatSonar::new_from_path(
        &program_lib_path,
        Some("#[program"),
        SonarResultType::Function,
    );
    let entrypoint_function = entrypoint_functions
        .results
        .iter()
        .find(|function| function.name == entrypoint_name)
        .unwrap();

    let parameters = get_function_parameters(entrypoint_function.content.clone());
    let context_name = parameters
        .iter()
        .find(|parameter| parameter.contains("Context<"))
        .unwrap()
        .split("Context<")
        .last()
        .unwrap()
        .split(">")
        .next()
        .unwrap();

    let instruction_file_content = fs::read_to_string(&instruction_file_path).unwrap();
    let instruction_file_functions =
        BatSonar::new_scanned(&instruction_file_content, SonarResultType::Function);
    let handler_function = instruction_file_functions
        .results
        .iter()
        .find(|function| {
            let function_parameters = get_function_parameters(function.content.clone());
            function_parameters
                .iter()
                .any(|parameter| parameter.contains(&context_name))
        })
        .unwrap();

    let metadata_path = batbelt::path::get_file_path(FilePathType::Metadata, true);
    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let structs_section = metadata_markdown
        .get_section(&MetadataSection::Structs.to_sentence_case())
        .unwrap();
    let structs_subsections = metadata_markdown.get_section_subsections(structs_section);
    let context_source_code = structs_subsections
        .iter()
        .filter(|subsection| subsection.section_header.title == context_name)
        .map(|section| StructMetadata::from_markdown_section(section.clone()))
        .find(|struct_metadata| struct_metadata.struct_type == StructMetadataType::ContextAccounts)
        .unwrap()
        .get_source_code();

    let started_path = batbelt::path::get_file_path(
        FilePathType::CodeOverhaulStarted {
            file_name: to_start_file_name.clone(),
        },
        false,
    );

    let mut started_markdown_file = MarkdownFile::new(&to_review_file_path);

    // Signers section
    let signers_section = started_markdown_file
        .get_section(&CodeOverhaulSection::Signers.to_title())
        .unwrap();
    let signers_section_content =
        get_signers_section_content(&context_source_code.clone().get_source_code_content());
    let mut new_signers_section = signers_section.clone();
    new_signers_section.content = signers_section_content.clone();
    started_markdown_file
        .replace_section(new_signers_section, signers_section, vec![])
        .unwrap();
    started_markdown_file.save().unwrap();

    // Function parameters section
    let function_parameter_section = started_markdown_file
        .get_section(&CodeOverhaulSection::FunctionParameters.to_title())
        .unwrap();
    let handler_function_parameters = get_function_parameters(handler_function.content.clone());
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
    let mut new_function_parameters_section = function_parameter_section.clone();
    new_function_parameters_section.content = function_parameters_content.clone();
    started_markdown_file
        .replace_section(
            new_function_parameters_section,
            function_parameter_section,
            vec![],
        )
        .unwrap();

    // Context accounts section
    let context_accounts_section = started_markdown_file
        .get_section(&CodeOverhaulSection::ContextAccounts.to_title())
        .unwrap();

    let context_accounts_content =
        get_context_account_section_content(&context_source_code.clone().get_source_code_content());
    let mut new_context_accounts_section = context_accounts_section.clone();
    new_context_accounts_section.content = context_accounts_content.clone();
    started_markdown_file
        .replace_section(
            new_context_accounts_section,
            context_accounts_section,
            vec![],
        )
        .unwrap();

    // Validations section
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
            .fold("".to_string(), |result, validation| {
                if result.is_empty() {
                    format!("- ```rust\n{}\n  ```\n", validation)
                } else {
                    format!("{}- ```rust\n{}\n  ```\n", result, validation)
                }
            })
    };
    let validations_section = started_markdown_file
        .get_section(&CodeOverhaulSection::Validations.to_title())
        .unwrap();

    let mut new_validations_section = validations_section.clone();
    new_validations_section.content = validations_content.clone();

    // println!("val con\n{}", validations_content.clone());
    started_markdown_file
        .replace_section(new_validations_section, validations_section, vec![])
        .unwrap();

    started_markdown_file.save().unwrap();

    execute_command("mv", &[&to_review_file_path, &started_path]).unwrap();

    println!("{to_start_file_name} file moved to started");

    // fill metadata
    let handler_function_name = handler_function.name.clone();
    let entrypoint_name = entrypoint_name.clone();
    let instruction_file_path = instruction_file_path.clone();
    let context_name = context_name.clone();

    // signers
    let signers = signers_section_content
        .lines()
        .map(|signer| {
            signer
                .trim_start_matches("- ")
                .split(":")
                .next()
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    let context_accounts = BatSonar::new_scanned(
        &context_source_code.get_source_code_content(),
        SonarResultType::ContextAccountsAll,
    );
    let mut_accounts = get_mut_accounts(context_accounts.results.clone());
    let function_parameters = if !handler_function_parameters.is_empty() {
        function_parameters_content
            .lines()
            .map(|line| {
                let name = line
                    .clone()
                    .strip_prefix("- ")
                    .unwrap()
                    .split(": ")
                    .next()
                    .unwrap()
                    .to_string();
                let param_type = line
                    .clone()
                    .strip_prefix("- ")
                    .unwrap()
                    .split(": ")
                    .last()
                    .unwrap()
                    .to_string();
                vec![name, param_type]
            })
            .collect::<Vec<_>>()
    } else {
        vec![vec![]]
    };

    let mut metadata_markdown = MarkdownFile::new(&metadata_path);
    let entrypoint_section = metadata_markdown
        .get_section(&MetadataSection::Entrypoints.to_sentence_case())
        .unwrap();
    let mut entrypoint_section_subsections =
        metadata_markdown.get_section_subsections(entrypoint_section.clone());

    let new_entrypoint = EntrypointMetadata::new(
        entrypoint_name,
        signers,
        instruction_file_path.clone(),
        handler_function_name,
        context_name.to_string(),
        mut_accounts,
        function_parameters,
    );
    let new_entrypoint_subsection =
        new_entrypoint.get_markdown_section(&entrypoint_section.section_header.section_hash);
    entrypoint_section_subsections.push(new_entrypoint_subsection);
    metadata_markdown
        .replace_section(
            entrypoint_section.clone(),
            entrypoint_section.clone(),
            entrypoint_section_subsections,
        )
        .unwrap();
    metadata_markdown.save().unwrap();

    create_git_commit(
        GitCommit::StartCO,
        Some(vec![to_start_file_name.to_string()]),
    )?;

    // open co file in VSCode
    vs_code_open_file_in_current_window(started_path.as_str())?;
    // open instruction file in VSCode
    vs_code_open_file_in_current_window(&instruction_file_path)?;

    Ok(())
}

fn get_mut_accounts(results: Vec<SonarResult>) -> Vec<Vec<String>> {
    let mut_accounts_results = results
        .iter()
        .filter(|account| account.content.contains("#[account(") && account.content.contains("mut"))
        .collect::<Vec<_>>();
    let mut result_vec: Vec<Vec<String>> = vec![];
    for mut_account_result in mut_accounts_results {
        let mut content_lines = mut_account_result.content.lines().clone();
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

pub fn get_context_account_section_content(context_accounts_content: &str) -> String {
    let accounts = BatSonar::new_scanned(
        context_accounts_content,
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

fn get_signers_section_content(context_lines: &str) -> String {
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
                CoderOverhaulTemplatePlaceholders::CompleteWithSignerDescription.to_placeholder()
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
        return "- No signers detected".to_string();
    }
    signers.join("\n")
}

#[test]
fn test_get_mut_accounts() {
    let example_results: Vec<SonarResult> = vec![
    SonarResult {
        name: "key".to_string(),
        content: "    pub key: Signer<'info>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 2,
        end_line_index: 2,
        is_public: true,
    },
    SonarResult {
        name: "profile".to_string(),
        content: "    pub profile: AccountLoader<'info, Profile>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 5,
        end_line_index: 5,
        is_public: true,
    },
    SonarResult {
        name: "funds_to".to_string(),
        content: "    #[account(mut)]\n    pub funds_to: UncheckedAccount<'info>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 9,
        end_line_index: 10,
        is_public: true,
    },
    SonarResult {
        name: "funds_to2".to_string(),
        content: "    #[account(mut, has_one = thing)]\n    pub funds_to2: UncheckedAccount<'info>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 9,
        end_line_index: 10,
        is_public: true,
    },
    SonarResult {
        name: "crafting_facility".to_string(),
        content: "    #[account(\n        mut,\n        has_one = domain @Errors::IncorrectDomain,\n        close = funds_to\n    )]\n    pub crafting_facility: AccountLoader<'info, CraftingFacility>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 13,
        end_line_index: 18,
        is_public: true,
    },
    SonarResult {
        name: "domain".to_string(),
        content: "    #[account(\n        has_one = profile @Errors::IncorrectProfileAddress,\n    )]\n    pub domain: AccountLoader<'info, Domain>,".to_string(),
        trailing_whitespaces: 4,
        result_type: ContextAccountsAll,
        start_line_index: 21,
        end_line_index: 24,
        is_public: true,
    },
];
    let mut_accounts = get_mut_accounts(example_results);

    println!("mut_accounts {:#?}", mut_accounts);
}
