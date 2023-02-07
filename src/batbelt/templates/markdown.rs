use std::fmt::Display;

use crate::batbelt::markdown::{MarkdownFile, MarkdownSectionLevel};
use crate::batbelt::templates::markdown::code_overhaul_template::CodeOverhaulSection;
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
        StateChanges,
        Notes,
        Signers,
        FunctionParameters,
        ContextAccounts,
        Validations,
    }

    impl CodeOverhaulSection {
        pub fn to_markdown_header(&self) -> String {
            format!("# {}:", self.to_string().to_sentence_case())
        }

        pub fn to_title(&self) -> String {
            format!("{}:", self.to_string().to_sentence_case())
        }
    }

    pub fn get_co_template_content() -> String {
        format!(
            "{}

            -
             
            {}

            -

            {}
            
            {}
            
            {}
            
            {}
            ",
            CodeOverhaulSection::StateChanges.to_markdown_header(),
            CodeOverhaulSection::Notes.to_markdown_header(),
            CodeOverhaulSection::Signers.to_markdown_header(),
            CodeOverhaulSection::FunctionParameters.to_markdown_header(),
            CodeOverhaulSection::ContextAccounts.to_markdown_header(),
            CodeOverhaulSection::Validations.to_markdown_header(),
        )
        .lines()
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }
}

// pub mod metadata_template {
//     use super::*;
//
//     #[derive(strum_macros::Display)]
//     pub enum MetadataSection {
//         Structs,
//         Functions,
//         Miro,
//     }
// }

#[test]
fn test_placeholder_to_string() {
    let co_template = MarkdownTemplate::CodeOverhaul.new(".");
    println!("co_template {:#?}", co_template);
}

#[test]
fn test_to_title() {
    let expected = "Signers:";
    let signers_title = CodeOverhaulSection::Signers.to_title();
    println!("co_template {:#?}", signers_title);
    assert_eq!(expected, signers_title, "Incorrect title");
}
