use std::fmt::Debug;

use crate::batbelt::markdown::{MarkdownSection, MarkdownSectionHeader, MarkdownSectionLevel};
use crate::batbelt::sonar::SonarResult;
use inflector::Inflector;

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
enum EntrypointInfoSection {
    Signers,
    InstructionFilePath,
    HandlerFunction,
    ContextName,
    MutAccounts,
    FunctionParameters,
}

impl EntrypointInfoSection {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }
}
#[derive(Debug, Clone)]
pub struct EntrypointMetadata {
    pub name: String,
    pub signers: Vec<String>,
    pub instruction_file_path: String,
    pub handler_function: String,
    pub context_name: String,
    pub mut_accounts: Vec<Vec<String>>,
    pub function_parameters: Vec<Vec<String>>,
}

impl EntrypointMetadata {
    pub fn new(
        name: String,
        signers: Vec<String>,
        instruction_file_path: String,
        handler_function: String,
        context_name: String,
        mut_accounts: Vec<Vec<String>>,
        function_parameters: Vec<Vec<String>>,
    ) -> Self {
        EntrypointMetadata {
            name,
            signers,
            instruction_file_path,
            handler_function,
            context_name,
            mut_accounts,
            function_parameters,
        }
    }

    pub fn get_markdown_section_content_string(&self) -> String {
        format!(
            "- context_name: {}\n- handler_function: {}\n- instruction_file_path: {}\n- signers: {}\n- mut_accounts: {}\n- function_parameters: {}",
            self.context_name,
            self.handler_function,
            self.instruction_file_path,
            self.get_signers_string(),
            self.get_mut_accounts_string(),
            self.get_function_parameters_string()
        )
    }

    fn get_signers_string(&self) -> String {
        let cs_signers = self.signers.join(",");
        format!("[{}]", cs_signers)
    }

    fn get_mut_accounts_string(&self) -> String {
        let cs_mut_accounts =
            self.mut_accounts
                .iter()
                .fold("".to_string(), |result, mut_account| {
                    if result.is_empty() {
                        format!("[{},{}]", mut_account[0], mut_account[1])
                    } else {
                        format!("{};[{},{}]", result, mut_account[0], mut_account[1])
                    }
                });
        format!("[{}]", cs_mut_accounts)
    }

    fn get_function_parameters_string(&self) -> String {
        let function_parameters =
            self.function_parameters
                .iter()
                .fold("".to_string(), |result, mut_account| {
                    if result.is_empty() {
                        format!("[{},{}]", mut_account[0], mut_account[1])
                    } else {
                        format!("{};[{},{}]", result, mut_account[0], mut_account[1])
                    }
                });
        format!("[{}]", function_parameters)
    }

    pub fn get_markdown_section(&self, section_hash: &str) -> MarkdownSection {
        let section_level_header = MarkdownSectionLevel::H2.get_header(&self.name);
        let section_header = MarkdownSectionHeader::new_from_header_and_hash(
            section_level_header,
            section_hash.to_string(),
            0,
        );
        let md_section = MarkdownSection::new(
            section_header,
            self.get_markdown_section_content_string(),
            0,
            0,
        );
        md_section
    }
    //
    pub fn from_markdown_section(md_section: MarkdownSection) -> Self {
        let name = md_section.section_header.title;
        let signers =
            Self::parse_metadata_info_section(&md_section.content, EntrypointInfoSection::Signers);
        let instruction_file_path = Self::parse_metadata_info_section(
            &md_section.content,
            EntrypointInfoSection::InstructionFilePath,
        );
        let handler_function = Self::parse_metadata_info_section(
            &md_section.content,
            EntrypointInfoSection::HandlerFunction,
        );
        let context_name = Self::parse_metadata_info_section(
            &md_section.content,
            EntrypointInfoSection::ContextName,
        );
        let mut_accounts = Self::parse_metadata_info_section(
            &md_section.content,
            EntrypointInfoSection::MutAccounts,
        );
        let function_parameters = Self::parse_metadata_info_section(
            &md_section.content,
            EntrypointInfoSection::FunctionParameters,
        );
        let signers = signers
            .trim_start_matches("[")
            .trim_end_matches("]")
            .split(",")
            .map(|signer| signer.to_string())
            .collect::<Vec<_>>();
        let mut_accounts = mut_accounts
            .trim_start_matches("[")
            .trim_end_matches("]")
            .split(";")
            .map(|mut_account| {
                vec![
                    mut_account.split(",").next().unwrap().to_string(),
                    mut_account.split(",").last().unwrap().to_string(),
                ]
            })
            .collect::<Vec<_>>();
        let function_parameters = function_parameters
            .trim_start_matches("[")
            .trim_end_matches("]")
            .split(";")
            .map(|fun_param| {
                vec![
                    fun_param.split(",").next().unwrap().to_string(),
                    fun_param.split(",").last().unwrap().to_string(),
                ]
            })
            .collect::<Vec<_>>();

        EntrypointMetadata::new(
            name,
            signers,
            instruction_file_path,
            handler_function,
            context_name,
            mut_accounts,
            function_parameters,
        )
    }

    fn parse_metadata_info_section(
        metadata_info_content: &str,
        entrypoint_section: EntrypointInfoSection,
    ) -> String {
        let section_prefix = entrypoint_section.get_prefix();
        let data = metadata_info_content
            .lines()
            .find(|line| line.contains(&section_prefix))
            .unwrap()
            .replace(&section_prefix, "")
            .trim()
            .to_string();
        data
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
            let is_mut = if content_lines.count() == 2 {
                let first_line = mut_account_result.content.lines().next().unwrap();
                let first_line = first_line
                    .trim()
                    .trim_start_matches("#[account(")
                    .trim_end_matches(")]");
                first_line.split(",").any(|spl| spl.trim() == "mut")
            } else {
                mut_account_result
                    .content
                    .lines()
                    .any(|line| line.trim().trim_end_matches(",") == "mut")
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
