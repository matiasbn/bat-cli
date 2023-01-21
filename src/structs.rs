#[derive(Debug)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Copy)]
pub enum SignerType {
    Validated,
    NotValidated,
    NotSigner,
}
pub struct SignerInfo {
    pub signer_text: String,
    pub sticky_note_id: String,
    pub user_figure_id: String,
    pub signer_type: SignerType,
}
