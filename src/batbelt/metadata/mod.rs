pub mod entrypoint;
pub mod functions;
pub mod miro;
pub mod source_code;
pub mod structs;

use crate::batbelt::{self};

use colored::Colorize;
use inflector::Inflector;

#[derive(strum_macros::Display)]
pub enum MetadataSection {
    Structs,
    Functions,
    Entrypoints,
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
