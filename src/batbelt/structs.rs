use crate::batbelt::miro::MiroColor;
use std::fs;

#[derive(Debug)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
}

impl FileInfo {
    pub fn new(path: String, name: String) -> Self {
        FileInfo { path, name }
    }
}

#[derive(Clone, Copy)]
pub enum SignerType {
    Validated,
    NotValidated,
    NotSigner,
}

impl SignerType {
    pub fn get_sticky_note_color(&self) -> MiroColor {
        match self {
            SignerType::Validated => MiroColor::Red,
            SignerType::NotValidated => MiroColor::DarkBlue,
            SignerType::NotSigner => MiroColor::Gray,
        }
    }
}

pub struct SignerInfo {
    pub signer_text: String,
    pub sticky_note_id: String,
    pub user_figure_id: String,
    pub signer_type: SignerType,
}
