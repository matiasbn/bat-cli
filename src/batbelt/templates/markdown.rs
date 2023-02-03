use std::fmt::Display;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSectionLevel};
use inflector::Inflector;

pub enum MarkdownTemplate {
    CodeOverhaul,
    Finding,
    Informational,
    Result,
    FindingCandidates,
    Metadata,
    OpenQuestions,
    ThreatModeling,
    Robot,
}

impl MarkdownTemplate {
    pub fn new(&self, path: &str) -> MarkdownFile {
        match self {
            Self::CodeOverhaul => {
                let content = code_overhaul_template::get_co_template_content().clone();
                let template = MarkdownFile::new_from_path_and_content(path, content);
                template
            }
            _ => unimplemented!(),
        }
    }
}

pub mod code_overhaul_template {
    use super::*;

    #[derive(strum_macros::Display)]
    pub enum CodeOverhaulSections {
        WhatItDoes,
        Notes,
        Signers,
        FunctionParameters,
        ContextAccounts,
        Validations,
        AccountsValidations,
        Prerequisites,
        MiroBoardFrame,
        InstructionFilePath,
    }

    impl CodeOverhaulSections {
        pub fn to_title(&self) -> String {
            self.to_string().to_sentence_case()
        }
    }

    pub fn get_co_template_content() -> String {
        format!(
            "# {}?:
            
            # {}:
            
            # {}:
            
            # {}:
            
            # {}:
            
            # {}:
            
            ## {}:
            
            ## {}:
            
            # {}:
            
            # {}:
            ",
            CodeOverhaulSections::WhatItDoes.to_title(),
            CodeOverhaulSections::Notes.to_title(),
            CodeOverhaulSections::Signers.to_title(),
            CodeOverhaulSections::FunctionParameters.to_title(),
            CodeOverhaulSections::ContextAccounts.to_title(),
            CodeOverhaulSections::Validations.to_title(),
            CodeOverhaulSections::AccountsValidations.to_title(),
            CodeOverhaulSections::Prerequisites.to_title(),
            CodeOverhaulSections::MiroBoardFrame.to_title(),
            CodeOverhaulSections::InstructionFilePath.to_title(),
        )
        .lines()
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }
}

pub mod metadata_template {
    use super::*;

    #[derive(strum_macros::Display)]
    pub enum MetadataSections {
        Structs,
        ContextAccounts,
        Accounts,
        Inputs,
        Functions,
        Handlers,
        Entrypoints,
        Helpers,
        Other,
        Miro,
        AccountsFrameUrl,
    }
}

#[test]
fn test_placeholder_to_string() {
    let co_template = MarkdownTemplate::CodeOverhaul.new(".");
    println!("co_template {:#?}", co_template);

    // let no_validation_placeholder =
    //     CodeOverhaulPlaceholder::NoValidationFoundPlaceholder.to_placeholder();
    // assert_eq!(no_validation_placeholder, expected);
}
