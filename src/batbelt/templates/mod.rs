pub mod code_overhaul_template;
use std::{error::Error, fmt};

#[derive(Debug)]
pub struct TemplateError;

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Template error")
    }
}

impl Error for TemplateError {}
