pub mod functions;
pub mod miro;
pub mod source_code;
pub mod structs;

use crate::batbelt::{self};

use colored::Colorize;
#[derive(strum_macros::Display)]
pub enum MetadataContent {
    Path,
    #[strum(serialize = "start_line_index")]
    StartLineIndex,
    #[strum(serialize = "end_line_index")]
    EndLineIndex,
}

impl MetadataContent {
    pub fn get_prefix(&self) -> &'static str {
        let content_str: String = self.to_string();
        let prefix = format!("- {}:", content_str.to_lowercase()).into_boxed_str();
        Box::leak(prefix)
    }
}

#[derive(strum_macros::Display)]
pub enum MetadataSection {
    Structs,
    Functions,
    Miro,
}

#[derive(strum_macros::Display)]
pub enum StructsSubSection {
    ContextAccounts,
    Account,
    Input,
    Other,
}

#[derive(strum_macros::Display)]
pub enum FunctionsSubSection {
    Handler,
    EntryPoint,
    Helper,
    Validator,
    Other,
}

pub trait MetadataSectionParser {
    fn section_str(&self) -> &'static str;
    fn get_index(&self) -> usize;
    fn get_sections_vec() -> Vec<String>;
}

impl MetadataSectionParser for MetadataSection {
    fn section_str(&self) -> &'static str {
        let section: String = self.to_string();
        Box::leak(section.into_boxed_str())
    }

    fn get_index(&self) -> usize {
        let section_str = self.section_str();
        let section_vec = Self::get_sections_vec();
        section_vec
            .into_iter()
            .position(|section| section == section_str)
            .unwrap()
    }

    fn get_sections_vec() -> Vec<String> {
        let metadata_sections_names: Vec<String> = vec![
            MetadataSection::Functions.to_string(),
            MetadataSection::Structs.to_string(),
        ];
        metadata_sections_names
    }
}

impl MetadataSectionParser for StructsSubSection {
    fn section_str(&self) -> &'static str {
        let section: String = self.to_string();
        Box::leak(section.into_boxed_str())
    }

    fn get_index(&self) -> usize {
        let section_str = self.section_str();
        let section_vec = Self::get_sections_vec();
        section_vec
            .into_iter()
            .position(|section| section == section_str)
            .unwrap()
    }

    fn get_sections_vec() -> Vec<String> {
        let metadata_sections_names: Vec<String> = vec![
            StructsSubSection::ContextAccounts.to_string(),
            StructsSubSection::Account.to_string(),
            StructsSubSection::Input.to_string(),
            StructsSubSection::Other.to_string(),
        ];
        metadata_sections_names
    }
}

impl MetadataSectionParser for FunctionsSubSection {
    fn section_str(&self) -> &'static str {
        let section: String = self.to_string();
        Box::leak(section.into_boxed_str())
    }

    fn get_index(&self) -> usize {
        let section_str = self.section_str();
        let section_vec = Self::get_sections_vec();
        section_vec
            .into_iter()
            .position(|section| section == section_str)
            .unwrap()
    }

    fn get_sections_vec() -> Vec<String> {
        let metadata_sections_names: Vec<String> = vec![
            FunctionsSubSection::EntryPoint.to_string(),
            FunctionsSubSection::Handler.to_string(),
            FunctionsSubSection::Validator.to_string(),
            FunctionsSubSection::Other.to_string(),
        ];
        metadata_sections_names
    }
}

// impl MetadataType {
//     pub fn from_title(title: &str) -> MetadataType {
//         match title {
//             "Structs" => MetadataType::Structs,
//             "Functions" => MetadataType::Function,
//             "Miro" => MetadataType::Miro,
//             _ => unimplemented!(),
//         }
//     }
// }

pub mod metadata_helpers {
    #[allow(unused_imports)]
    use super::*;

    pub fn prompt_user_update_section(section_name: &str) -> Result<(), String> {
        let user_decided_to_continue = batbelt::cli_inputs::select_yes_or_no(
            format!(
                "{}, are you sure you want to continue?",
                format!("{} in metadata.md is already initialized", section_name).bright_red()
            )
            .as_str(),
        )?;
        if !user_decided_to_continue {
            panic!(
                "User decided not to continue with the update process for {} metada",
                section_name.red()
            )
        }
        Ok(())
    }
}

#[test]
fn test_get_metadata_prefix() {
    let expected_path = "- path:";
    let expected_start_line_index = "- start_line_index:";
    let expected_end_line_index = "- end_line_index:";
    assert_eq!(MetadataContent::Path.get_prefix(), expected_path);
    assert_eq!(
        MetadataContent::StartLineIndex.get_prefix(),
        expected_start_line_index
    );
    assert_eq!(
        MetadataContent::EndLineIndex.get_prefix(),
        expected_end_line_index
    );
}

#[test]
fn test_get_struct_title() {
    let expected_prefix = "Structs";
    assert_eq!(MetadataSection::Structs.section_str(), expected_prefix)
}
