pub mod functions;
pub mod miro;
pub mod source_code;
pub mod structs;

use crate::batbelt::{self};

use colored::Colorize;
use inflector::Inflector;

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

impl MetadataSection {
    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }
}

#[derive(strum_macros::Display)]
pub enum FunctionsSubSection {
    Handler,
    EntryPoint,
    Helper,
    Validator,
    Other,
}

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
    assert_eq!(MetadataSection::Structs.to_string(), expected_prefix)
}
