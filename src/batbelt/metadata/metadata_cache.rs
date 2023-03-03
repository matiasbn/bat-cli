use crate::batbelt::metadata::{
    BatMetadataParser, BatMetadataType, MetadataError, MetadataId, MetadataResult,
};
use crate::batbelt::path::BatFile;
use crate::batbelt::BatEnumerator;

use colored::Colorize;
use error_stack::{FutureExt, IntoReport, Report, ResultExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(
    Default,
    Debug,
    PartialEq,
    Clone,
    Copy,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum MetadataCacheType {
    #[default]
    Function,
    Struct,
    Trait,
}
impl BatEnumerator for MetadataCacheType {}

impl MetadataCacheType {
    pub fn get_metadata_cache_by_id(&self, metadata_id: String) -> MetadataResult<MetadataCache> {
        let mut new_metadata = MetadataCache::new(metadata_id, *self);
        new_metadata.read_cache_by_id()?;
        Ok(new_metadata)
    }

    pub fn get_bat_file(&self) -> BatFile {
        BatFile::MetadataCacheFile {
            metadata_cache_type: BatMetadataType::Struct,
        }
    }
}
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct MetadataCacheContent {
    pub metadata_cache_content_type: String,
    pub cache_values: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetadataCache {
    pub metadata_id: String,
    pub metadata_cache_type: MetadataCacheType,
    pub metadata_cache_content: Vec<MetadataCacheContent>,
}

impl MetadataCache {
    pub fn new(metadata_id: MetadataId, metadata_cache_type: MetadataCacheType) -> Self {
        Self {
            metadata_id,
            metadata_cache_type,
            metadata_cache_content: vec![],
        }
    }

    pub fn read_cache_by_id(&mut self) -> MetadataResult<()> {
        let content_values = Self::read_json_content(self.metadata_cache_type)?;
        let metadata_cache_value = content_values[&self.metadata_id].clone();

        if !metadata_cache_value.is_array() {
            return Err(Report::new(MetadataError).attach_printable(format!(
                "MetadataCacheContent not found for type: {} with metadata_id {}",
                self.metadata_cache_type.to_string().red(),
                self.metadata_id.red()
            )));
        }

        let metadata_cache: Vec<MetadataCacheContent> =
            serde_json::from_str(&metadata_cache_value.to_string())
                .into_report()
                .change_context(MetadataError)?;
        self.metadata_cache_content = metadata_cache;
        Ok(())
    }

    pub fn save_to_file(&self) -> MetadataResult<()> {
        let json = json!({self.metadata_id.clone(): self.metadata_cache_content.clone()});
        let json_content = serde_json::to_string_pretty(&json)
            .into_report()
            .change_context(MetadataError)?;
        self.metadata_cache_type
            .get_bat_file()
            .write_content(false, &json_content)
            .change_context(MetadataError)?;
        // log::debug!("new_file_created:\n metadata_id: {}\n{}")
        Ok(())
    }

    pub fn insert_cache(&mut self, cache_content: MetadataCacheContent) -> MetadataResult<()> {
        return match self.read_cache_by_id() {
            Ok(_) => {
                match self.metadata_cache_content.iter().position(|content| {
                    content.metadata_cache_content_type == cache_content.metadata_cache_content_type
                }) {
                    None => {
                        self.metadata_cache_content.push(cache_content);
                        Ok(())
                    }
                    Some(match_index) => {
                        let mut cloned_cache_values = cache_content
                            .clone()
                            .cache_values
                            .into_iter()
                            .filter(|cache_vale| {
                                self.metadata_cache_content[match_index]
                                    .cache_values
                                    .iter()
                                    .any(|self_value| self_value == cache_vale)
                            })
                            .collect::<Vec<_>>();
                        self.metadata_cache_content[match_index]
                            .cache_values
                            .append(&mut cloned_cache_values);
                        return Ok(());
                    }
                }
            }
            Err(error) => {
                return Err(error);
            }
        };
    }

    pub fn create_new_key(
        _cache_content: MetadataCacheContent,
        _metadata_cache_type: MetadataCacheType,
    ) -> MetadataResult<()> {
        // let bat_file_content = metadata_cache_type
        //     .get_bat_file()
        //     .read_content(false)
        //     .change_context(MetadataError)?;
        // let mut content_values: Value = serde_json::from_str(&bat_file_content)
        //     .into_report()
        //     .change_context(MetadataError)?;
        // let mut json_map = content_values.as_object();
        // if json_map.is_some() {
        //     let mut new_map = json_map.clone().unwrap();
        //     new_map.insert(
        //         cache_content.metadata_cache_content_type.clone(),
        //         cache_content.cache_values.into(),
        //     );
        // }
        Ok(())
    }

    fn read_json_content(metadata_cache_type: MetadataCacheType) -> MetadataResult<Value> {
        let bat_file_content = metadata_cache_type
            .get_bat_file()
            .read_content(false)
            .change_context(MetadataError)?;
        let content_values: Value = serde_json::from_str(&bat_file_content)
            .into_report()
            .change_context(MetadataError)?;
        Ok(content_values)
    }
}

#[cfg(debug_assertions)]
mod metadata_cache_test {
    use crate::batbelt::metadata::metadata_cache::MetadataCacheContent;
    use crate::batbelt::metadata::BatMetadataType;
    use crate::batbelt::path::{BatFile, BatFolder};
    use crate::config::{BatAuditorConfig, BatConfig};
    use serde_json::json;
    use std::fs;
    use std::process::Command;

    #[test]
    fn test_get_metadata_by_id() {
        let bat_toml_file = BatFile::BatToml;
        let bat_toml_path = bat_toml_file.get_path(false).unwrap();
        let bat_auditor_toml_file = BatFile::BatAuditorToml;
        let bat_auditor_toml_path = bat_auditor_toml_file.get_path(false).unwrap();
        assert_fs::NamedTempFile::new(&bat_toml_path).unwrap();
        assert_fs::NamedTempFile::new(&bat_auditor_toml_path).unwrap();

        let bat_config = BatConfig {
            initialized: true,
            project_name: "".to_string(),
            client_name: "".to_string(),
            commit_hash_url: "".to_string(),
            starting_date: "".to_string(),
            miro_board_url: "".to_string(),
            auditor_names: vec![],
            program_lib_path: "".to_string(),
            program_name: "".to_string(),
            project_repository_url: "".to_string(),
        };
        let bat_auditor_config = BatAuditorConfig {
            auditor_name: "test".to_string(),
            miro_oauth_access_token: "".to_string(),
            use_code_editor: false,
            code_editor: Default::default(),
        };

        bat_auditor_config.save().unwrap();
        bat_config.save().unwrap();
        let metadata_bat_folder = BatFolder::MetadataFolder;
        // create
        fs::create_dir_all(metadata_bat_folder.get_path(false).unwrap()).unwrap();

        let metadata_bat_file = BatFile::MetadataCacheFile {
            metadata_cache_type: BatMetadataType::Struct,
        };

        let bat_path = metadata_bat_file.get_path(false).unwrap();

        let file =
            assert_fs::NamedTempFile::new(&metadata_bat_file.get_path(false).unwrap()).unwrap();

        let metadata_id = "1234";
        let metadata_cache_type_dependencies = "dependencies".to_string();
        let cache_values_dep = vec!["hello".to_string(), "how_are_you".to_string()];
        let metadata_cache_type_extdep = "external_dependencies".to_string();
        let cache_values_ext_dep = vec!["hello".to_string(), "how_are_you".to_string()];
        let metadata_content_dependencies = MetadataCacheContent {
            metadata_cache_content_type: metadata_cache_type_dependencies.clone(),
            cache_values: cache_values_dep,
        };
        let metadata_content_ext_dep = MetadataCacheContent {
            metadata_cache_content_type: metadata_cache_type_extdep.clone(),
            cache_values: cache_values_ext_dep,
        };
        let json_value =
            json!({ metadata_id: vec![metadata_content_dependencies, metadata_content_ext_dep] });
        let content = serde_json::to_string_pretty(&json_value).unwrap();

        // file.write_str(&content).unwrap().;
        fs::write(&bat_path, &content).unwrap();
        let result = Command::new("ls")
            .args(["-la", "./notes"])
            .output()
            .unwrap()
            .stdout;
        let str1 = String::from_utf8(result).unwrap();

        let result = Command::new("ls")
            .args(["-la", "./notes/test-notes/"])
            .output()
            .unwrap()
            .stdout;
        let str2 = String::from_utf8(result).unwrap();
        let result = Command::new("ls")
            .args(["-la", "./notes/test-notes/metadata/"])
            .output()
            .unwrap()
            .stdout;
        let str3 = String::from_utf8(result).unwrap();

        // let metadata_cache =
        //     MetadataCache::new_by_id(MetadataCacheType::Function, metadata_id.to_string()).unwrap();

        // let content_values: Value = serde_json::from_str(&content).unwrap();
        // let metadata_cache = content_values[metadata_id].clone();
        // let metadata_cache_struct: Vec<MetadataCacheContent> =
        //     serde_json::from_str(&metadata_cache.to_string()).unwrap();
        // println!("{:#?}", metadata_cache);
        // assert_eq!(metadata_cache_struct, metadata_content);
        let metadata_bat_file = BatFile::MetadataCacheFile {
            metadata_cache_type: BatMetadataType::Struct,
        };
        metadata_bat_file.remove_file().unwrap();
        let metadata_bat_folder = BatFolder::MetadataFolder;
        // create
        fs::remove_dir_all(metadata_bat_folder.get_path(false).unwrap()).unwrap();
    }
}
