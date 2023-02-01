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
                let co_template = &Self::get_co_template_content();
                println!("temp {}", co_template);
                MarkdownFile::new_from_path_and_content(path, &Self::get_co_template_content())
            }
            _ => unimplemented!(),
        }
    }

    fn get_co_template_content() -> String {
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
            code_overhaul::CodeOverhaulSections::WhatItDoes.to_title(),
            code_overhaul::CodeOverhaulSections::Notes.to_title(),
            code_overhaul::CodeOverhaulSections::Signers.to_title(),
            code_overhaul::CodeOverhaulSections::FunctionParameters.to_title(),
            code_overhaul::CodeOverhaulSections::ContextAccounts.to_title(),
            code_overhaul::CodeOverhaulSections::Validations.to_title(),
            code_overhaul::CodeOverhaulSections::AccountsValidations.to_title(),
            code_overhaul::CodeOverhaulSections::Prerequisites.to_title(),
            code_overhaul::CodeOverhaulSections::MiroBoardFrame.to_title(),
            code_overhaul::CodeOverhaulSections::InstructionFilePath.to_title(),
        )
        .lines()
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }
}
pub mod code_overhaul {
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
}

#[test]
fn test_placeholder_to_string() {
    let co_template = MarkdownTemplate::CodeOverhaul.new(".");
    println!("co_template {:#?}", co_template);

    // let no_validation_placeholder =
    //     CodeOverhaulPlaceholder::NoValidationFoundPlaceholder.to_placeholder();
    // assert_eq!(no_validation_placeholder, expected);
}
