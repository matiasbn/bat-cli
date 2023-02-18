use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, fs, path::Path, str};

use crate::batbelt::path::BatFile;
use error_stack::{IntoReport, Report, Result, ResultExt};

#[derive(Debug)]
pub struct BatConfigError;

impl fmt::Display for BatConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatConfig error")
    }
}

impl Error for BatConfigError {}

struct BatConfigParameterNotFound(&'static str);
struct BatAuditorConfigParameterNotFound(&'static str);

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct BatAuditorConfig {
    pub initialized: bool,
    pub auditor_name: String,
    pub miro_oauth_access_token: String,
    pub vs_code_integration: bool,
}

impl BatAuditorConfig {
    pub fn get_config() -> Result<Self, BatConfigError> {
        let bat_config: BatAuditorConfig = confy::load_path("BatAuditor.toml")
            .into_report()
            .change_context(BatConfigError)?;
        Ok(bat_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        Ok(confy::store("BatAuditor.toml", None, &self)
            .into_report()
            .change_context(BatConfigError)?)
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
    pub project_repository_url: String,
}

impl BatConfig {
    pub fn get_config() -> Result<Self, BatConfigError> {
        let bat_config: BatConfig = confy::load_path("Bat.toml")
            .into_report()
            .change_context(BatConfigError)?;
        Ok(bat_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        Ok(confy::store("Bat.toml", None, &self)
            .into_report()
            .change_context(BatConfigError)?)
    }

    // pub fn get_validated_config() -> Result<BatConfig, BatConfigError> {
    //     let bat_config = Self::get_bat_config()?;
    //     // Self::validate_bat_config(bat_config.clone(), true)?;
    //     Ok(bat_config)
    // }
    //
    // pub fn get_init_config() -> Result<BatConfig, BatConfigError> {
    //     let bat_config: BatConfig = Self::get_bat_config()?;
    //     // Self::validate_bat_config(bat_config.clone(), false)?;
    //     Ok(bat_config)
    // }
    //
    // fn get_bat_config() -> Result<BatConfig, BatConfigError> {
    //     // Bat.toml
    //     let bat_toml_path = Path::new("Bat.toml");
    //     if !bat_toml_path.is_file() {
    //         let message = format!("Bat.toml file not found at {bat_toml_path:?}");
    //         return Err(Report::new(BatConfigError).attach_printable(message));
    //     }
    //
    //     // BatAuditor.toml
    //     let auditor_toml_path = Path::new("BatAuditor.toml");
    //     if !auditor_toml_path.is_file() {
    //         // if BatAuditor does not exist, create it
    //         create_auditor_toml().change_context(BatConfigError)?;
    //         println!("BatAuditor.toml file not detected, creating")
    //     }
    //     let config: BatConfig = confy::load_path("Bat.toml")
    //         .into_report()
    //         .change_context(BatConfigError)
    //         .attach_printable("Error parsing BatAuditor.toml".to_string())?;
    //     Ok(config)
    // }

    // fn validate_bat_config(
    //     bat_config: BatConfig,
    //     validate_auditor: bool,
    // ) -> Result<(), BatConfigError> {
    //     let BatConfig {
    //         required, auditor, ..
    //     } = bat_config;
    //     // Validate required
    //     if required.project_name.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter project_name is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.client_name.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter client_name is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.commit_hash_url.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter commit_hash_url is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.starting_date.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter starting_date is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.miro_board_url.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter miro_board_url is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.program_lib_path.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter program_lib_path is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.auditor_names.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatConfigParameterNotFound(
    //                 "required parameter auditor_names is empty at Bat.toml",
    //             )),
    //         );
    //     }
    //     if required.project_repository_url.is_empty() {
    //         return Err(Report::new(BatConfigError).attach_printable(
    //             "required parameter project_repository_url is empty at Bat.toml",
    //         ));
    //     }
    //
    //     // Validate auditor
    //     if validate_auditor && auditor.auditor_name.is_empty() {
    //         return Err(
    //             Report::new(BatConfigError).attach(BatAuditorConfigParameterNotFound(
    //                 "required parameter auditor_name is empty at BatAuditor.toml",
    //             )),
    //         );
    //     }
    //     Ok(())
    // }
}

#[cfg(test)]
mod bat_test {
    use super::*;
    use crate::batbelt::bash::execute_command;

    use std::env;

    #[test]
    fn test_confy() {
        env::set_current_dir("../sage-audit").unwrap();
        execute_command("ls", &[]).unwrap();
        // let bat_toml_path = FilePathType::BatToml.get_path(false).unwrap();
        // println!("{:#?}", &FilePathType::BatToml.get_path(false).unwrap());
        let bat_config: BatConfig = confy::load_path("Bat.toml").unwrap();
        println!("hola")
    }
}
