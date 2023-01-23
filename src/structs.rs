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

    pub fn read_content(&self) -> Result<String, String> {
        let file_info_content = fs::read_to_string(self.path.clone())
            .expect(format!("Error reading fileinfo for: {}", self.path).as_str());
        Ok(file_info_content)
    }
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
