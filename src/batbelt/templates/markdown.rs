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
    pub enum CodeOverhaulSection {
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

    impl CodeOverhaulSection {
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
            CodeOverhaulSection::WhatItDoes.to_title(),
            CodeOverhaulSection::Notes.to_title(),
            CodeOverhaulSection::Signers.to_title(),
            CodeOverhaulSection::FunctionParameters.to_title(),
            CodeOverhaulSection::ContextAccounts.to_title(),
            CodeOverhaulSection::Validations.to_title(),
            CodeOverhaulSection::AccountsValidations.to_title(),
            CodeOverhaulSection::Prerequisites.to_title(),
            CodeOverhaulSection::MiroBoardFrame.to_title(),
            CodeOverhaulSection::InstructionFilePath.to_title(),
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
    pub enum MetadataSection {
        Structs,
        Functions,
        Miro,
    }
}

#[test]
fn test_placeholder_to_string() {
    let co_template = MarkdownTemplate::CodeOverhaul.new(".");
    println!("co_template {:#?}", co_template);
}
