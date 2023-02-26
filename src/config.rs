use clap::Parser;
use figment::{
    providers::{Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, str};

use crate::batbelt::path::BatFile;
use error_stack::{FutureExt, IntoReport, Result, ResultExt};

#[derive(Debug)]
pub struct BatConfigError;

impl fmt::Display for BatConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BatConfig error")
    }
}

impl Error for BatConfigError {}

#[derive(Default, Debug, Serialize, Deserialize, Clone, Parser)]
pub struct BatAuditorConfig {
    pub auditor_name: String,
    pub miro_oauth_access_token: String,
    pub vs_code_integration: bool,
}

impl BatAuditorConfig {
    pub fn get_config() -> Result<Self, BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(true)
            .change_context(BatConfigError)?;
        let bat_config: BatAuditorConfig = Figment::new()
            .merge(Toml::file(path))
            .extract()
            .into_report()
            .change_context(BatConfigError)
            .attach_printable("Error parsing BatAuditor.toml")?;
        Ok(bat_config)
    }

    pub fn save(&self) -> Result<(), BatConfigError> {
        let path = BatFile::BatAuditorToml
            .get_path(false)
            .change_context(BatConfigError)?;
        Ok(confy::store_path(path, self)
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
        Ok(confy::store_path(path, self)
            .into_report()
            .change_context(BatConfigError)?)
    }
}

#[cfg(test)]
mod bat_test {
    use super::*;

    #[test]
    fn test_confy() -> Result<(), BatConfigError> {
        // env::set_current_dir("../sage-audit").unwrap();
        // execute_command("ls", &[]).unwrap();
        // let bat_toml_path = BatFile::BatToml
        //     .get_path(false)
        //     .change_context(BatConfigError)?;
        // // println!("{:#?}", &BatFile::BatToml.get_path(false)?);
        // let bat_config: BatConfig = confy::load_path("Bat.toml")
        //     .into_report()
        //     .change_context(BatConfigError)?;
        // let bat_auditor_config = BatConfig::get_config()?;
        // bat_auditor_config.project_name = "hola".to_string();
        // let bat_config = BatConfig {
        //     initialized: false,
        //     project_name: "chai".to_string(),
        //     client_name: "hola".to_string(),
        //     commit_hash_url: "askjdhajkdhakjhdakjhskjh".to_string(),
        //     starting_date: "asdljhaskdjhalkhdlka".to_string(),
        //     miro_board_url: "222".to_string(),
        //     auditor_names: vec![],
        //     program_lib_path: "".to_string(),
        //     project_repository_url: "".to_string(),
        // };
        // bat_config.save().unwrap();
        // confy::store_path("Bat.toml", bat_config).unwrap();
        // bat_auditor_config.save()?;
        Ok(())
    }
}
