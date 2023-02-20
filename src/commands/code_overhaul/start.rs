use std::fs;
use std::string::String;

use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};

use crate::batbelt;
use crate::batbelt::bash::execute_command;
use crate::batbelt::command_line::vs_code_open_file_in_current_window;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::functions_metadata::get_function_parameters;
use crate::batbelt::metadata::structs_metadata::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::BatMetadataType;
use crate::batbelt::parser::entrypoint_parser::EntrypointParser;
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::sonar::{BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::code_overhaul_template::{
    CodeOverhaulSection, CodeOverhaulTemplate, CoderOverhaulTemplatePlaceholders,
};
use crate::commands::CommandError;
use crate::config::BatConfig;

pub fn start_co_file() -> Result<(), CommandError> {
    let review_files = BatFolder::CodeOverhaulToReview
        .get_all_files(true, None, None)
        .change_context(CommandError)?
        .into_iter()
        .map(|entry| entry.file_name().to_str().unwrap().to_string())
        .collect::<Vec<_>>();

    if review_files.is_empty() {
        return Err(Report::new(CommandError)
            .attach_printable("no to-review files in code-overhaul folder"));
    }
    let prompt_text = "Select the code-overhaul file to start:";
    let selection = batbelt::cli_inputs::select(prompt_text, review_files.clone(), None)
        .change_context(CommandError)?;

    // user select file
    let to_start_file_name = &review_files[selection].clone();
    let entrypoint_name = to_start_file_name.trim_end_matches(".md");
    let to_review_file_path = BatFile::CodeOverhaulToReview {
        file_name: to_start_file_name.clone(),
    }
    .get_path(true)
    .change_context(CommandError)?;

    let started_path = BatFile::CodeOverhaulStarted {
        file_name: to_start_file_name.clone(),
    }
    .get_path(false)
    .change_context(CommandError)?;

    let started_template =
        CodeOverhaulTemplate::new(entrypoint_name, true).change_context(CommandError)?;
    let mut started_markdown = started_template
        .to_markdown_file(&started_path)
        .change_context(CommandError)?;

    started_markdown.save().change_context(CommandError)?;

    execute_command("rm", &[&to_review_file_path]).unwrap();

    println!("{to_start_file_name} file moved to started");

    create_git_commit(
        GitCommit::StartCO,
        Some(vec![to_start_file_name.to_string()]),
    )
    .change_context(CommandError)?;

    // open co file in VSCode
    vs_code_open_file_in_current_window(started_path.as_str())?;

    // open instruction file in VSCode
    if started_template.entrypoint_parser.is_some() {
        let ep_parser = started_template.entrypoint_parser.unwrap();
        if ep_parser.handler.is_some() {
            let handler = ep_parser.handler.unwrap();
            vs_code_open_file_in_current_window(&handler.path)?;
        }
    }
    Ok(())
}

// // fill metadata
// let handler_function_name = handler_function.name.clone();
// let entrypoint_name = entrypoint_name.clone();
// let instruction_file_path = instruction_file_path.clone();
// let context_name = context_name.clone();
//
// // signers
// let signers = signers_section_content
//     .lines()
//     .map(|signer| {
//         signer
//             .trim_start_matches("- ")
//             .split(":")
//             .next()
//             .unwrap()
//             .to_string()
//     })
//     .collect::<Vec<_>>();
// let context_accounts = BatSonar::new_scanned(
//     &context_source_code
//         .to_source_code(None)
//         .get_source_code_content(),
//     SonarResultType::ContextAccountsAll,
// );
// let mut_accounts = get_mut_accounts(context_accounts.results.clone());
// let function_parameters = if !handler_function_parameters.is_empty() {
//     function_parameters_content
//         .lines()
//         .map(|line| {
//             let name = line
//                 .clone()
//                 .strip_prefix("- ")
//                 .unwrap()
//                 .split(": ")
//                 .next()
//                 .unwrap()
//                 .to_string();
//             let param_type = line
//                 .clone()
//                 .strip_prefix("- ")
//                 .unwrap()
//                 .split(": ")
//                 .last()
//                 .unwrap()
//                 .to_string();
//             vec![name, param_type]
//         })
//         .collect::<Vec<_>>()
// } else {
//     vec![vec![]]
// };
//
// let mut metadata_markdown = MarkdownFile::new(&metadata_path);
// let entrypoint_section = metadata_markdown
//     .get_section(&BatMetadataSection::Entrypoints.to_sentence_case())
//     .unwrap();
// let mut entrypoint_section_subsections =
//     metadata_markdown.get_section_subsections(entrypoint_section.clone());
//
// let new_entrypoint = EntrypointMetadata::new(
//     entrypoint_name,
//     signers,
//     instruction_file_path.clone(),
//     handler_function_name,
//     context_name.to_string(),
//     mut_accounts,
//     function_parameters,
// );
// let new_entrypoint_subsection =
//     new_entrypoint.get_markdown_section(&entrypoint_section.section_header.section_hash);
// entrypoint_section_subsections.push(new_entrypoint_subsection);
// metadata_markdown
//     .replace_section(
//         entrypoint_section.clone(),
//         entrypoint_section.clone(),
//         entrypoint_section_subsections,
//     )
//     .unwrap();
// metadata_markdown.save().unwrap();

fn get_mut_accounts(results: Vec<SonarResult>) -> Vec<Vec<String>> {
    let mut_accounts_results = results
        .iter()
        .filter(|account| account.content.contains("#[account(") && account.content.contains("mut"))
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
        return format!(
            "- {}",
            CoderOverhaulTemplatePlaceholders::NoSignersDetected.to_placeholder(),
        );
    }
    signers.join("\n")
}

// #[test]
// fn test_get_mut_accounts() {
//     let example_results: Vec<SonarResult> = vec![
//     SonarResult {
//         name: "key".to_string(),
//         content: "    pub key: Signer<'info>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 2,
//         end_line_index: 2,
//         is_public: true,
//     },
//     SonarResult {
//         name: "profile".to_string(),
//         content: "    pub profile: AccountLoader<'info, Profile>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 5,
//         end_line_index: 5,
//         is_public: true,
//     },
//     SonarResult {
//         name: "funds_to".to_string(),
//         content: "    #[account(mut)]\n    pub funds_to: UncheckedAccount<'info>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 9,
//         end_line_index: 10,
//         is_public: true,
//     },
//     SonarResult {
//         name: "funds_to2".to_string(),
//         content: "    #[account(mut, has_one = thing)]\n    pub funds_to2: UncheckedAccount<'info>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 9,
//         end_line_index: 10,
//         is_public: true,
//     },
//     SonarResult {
//         name: "crafting_facility".to_string(),
//         content: "    #[account(\n        mut,\n        has_one = domain @Errors::IncorrectDomain,\n        close = funds_to\n    )]\n    pub crafting_facility: AccountLoader<'info, CraftingFacility>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 13,
//         end_line_index: 18,
//         is_public: true,
//     },
//     SonarResult {
//         name: "domain".to_string(),
//         content: "    #[account(\n        has_one = profile @Errors::IncorrectProfileAddress,\n    )]\n    pub domain: AccountLoader<'info, Domain>,".to_string(),
//         trailing_whitespaces: 4,
//         result_type: ContextAccountsAll,
//         start_line_index: 21,
//         end_line_index: 24,
//         is_public: true,
//     },
// ];
//     let mut_accounts = get_mut_accounts(example_results);

//     println!("mut_accounts {:#?}", mut_accounts);
// }
