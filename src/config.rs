use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

use std::{error::Error, fmt, str};

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::command_line::CodeEditor;
use crate::batbelt::path::BatFile;
use crate::batbelt::BatEnumerator;

use colored::Colorize;
use error_stack::{FutureExt, IntoReport, Result, ResultExt};
use figment::error::Kind;

#[derive(Debug)]
pub struct BatConfigError;

impl fmt::Display for BatConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatConfig error")
    }
}

impl Error for BatConfigError {}

pub type BatConfigResult<T> = Result<T, BatConfigError>;

#[derive(Default, Debug, Serialize, Deserialize, Clone, PartialOrd, PartialEq)]
pub struct BatAuditorConfig {
    pub auditor_name: String,
    pub miro_oauth_access_token: String,
    pub use_code_editor: bool,
    pub code_editor: CodeEditor,
}

impl BatAuditorConfig {
    pub fn new_with_prompt() -> BatConfigResult<Self> {
        let mut bat_auditor_config = BatAuditorConfig {
            auditor_name: "".to_string(),
            miro_oauth_access_token: "".to_string(),
            use_code_editor: false,
            code_editor: Default::default(),
        };
        bat_auditor_config.prompt_auditor_name()?;
        bat_auditor_config.prompt_miro_integration()?;
        bat_auditor_config.prompt_code_editor_integration()?;
        bat_auditor_config.save()?;
        Ok(bat_auditor_config)
    }

    fn prompt_auditor_name(&mut self) -> BatConfigResult<()> {
        let bat_config = BatConfig::get_config()?;
        let auditor_names = bat_config.auditor_names;
        let prompt_text = "Select your name:".to_string();
        let selection = BatDialoguer::select(prompt_text, auditor_names.clone(), None)
            .change_context(BatConfigError)?;
        let auditor_name = auditor_names.get(selection).unwrap().clone();
        self.auditor_name = auditor_name;
        Ok(())
    }

    fn prompt_miro_integration(&mut self) -> BatConfigResult<()> {
        let prompt_text = "Do you want to use the Miro integration?";
        let include_miro = BatDialoguer::select_yes_or_no(prompt_text.to_string())
            .change_context(BatConfigError)?;
        let moat = if include_miro {
            let prompt_text = "Miro OAuth access token";
            BatDialoguer::input(prompt_text.to_string()).change_context(BatConfigError)?
        } else {
            "".to_string()
        };
        self.miro_oauth_access_token = moat;
        Ok(())
    }

    fn prompt_code_editor_integration(&mut self) -> BatConfigResult<()> {
        let prompt_text = format!(
            "Select a code editor, choose {} to disable:",
            CodeEditor::None.get_colored_name(false)
        );
        let editor_colorized_vec = CodeEditor::get_colorized_type_vec(false);
        let editor_integration = BatDialoguer::select(prompt_text, editor_colorized_vec, None)
            .change_context(BatConfigError)?;
        self.code_editor = CodeEditor::from_index(editor_integration);
        self.use_code_editor = self.code_editor != CodeEditor::None;
        Ok(())
    }

    pub fn get_config() -> Result<Self, BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let bat_config_result = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing BatAuditor.toml");
        return match bat_config_result {
            Ok(bat_auditor_config) => Ok(bat_auditor_config),
            Err(error) => {
                log::error!("Error parsing BatAuditor \n {}", error);
                let frames_errors = error
                    .frames()
                    .filter_map(|frame| {
                        let downcast = frame.downcast_ref::<figment::Error>();
                        if downcast.is_some() {
                            Some(downcast.unwrap())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                for frame_error in frames_errors {
                    match frame_error.clone().kind {
                        Kind::MissingField(missing_field) => {
                            println!("{} field missing on BatAuditor.toml", missing_field.red());
                        }
                        _ => {}
                    }
                }
                println!("\nCreating {} again\n", "BatAuditor.toml".bright_green());
                Self::new_with_prompt()?;
                let new_config = Self::get_config()?;
                return Ok(new_config);
            }
        };
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(false)
            .change_context(BatConfigError)?;
        confy::store_path(path, self)
            .into_report()
            .change_context(BatConfigError)
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BatConfig {
    pub initialized: bool,
    pub project_name: String,
    pub client_name: String,
    pub commit_hash_url: String,
    pub starting_date: String,
    pub miro_board_url: String,
    pub auditor_names: Vec<String>,
    pub program_lib_path: String,
    pub program_name: String,
    pub project_repository_url: String,
}

impl BatConfig {
    pub fn get_config() -> Result<Self, BatConfigError> {
        let path = BatFile::BatToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let bat_config: BatConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing Bat.toml")?;
        Ok(bat_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        let path = BatFile::BatToml
            .get_path(false)
            .change_context(BatConfigError)?;
        confy::store_path(path, self)
            .into_report()
            .change_context(BatConfigError)
    }
}
