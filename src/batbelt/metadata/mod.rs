pub mod context_accounts_metadata;
pub mod entrypoint_metadata;
pub mod enums_source_code_metadata;
pub mod function_dependencies_metadata;
pub mod functions_source_code_metadata;
pub mod miro_metadata;
pub mod structs_source_code_metadata;
pub mod trait_metadata;
pub mod traits_source_code_metadata;

use colored::Colorize;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::{env, fmt};

use crate::batbelt::path::BatFile;

use inflector::Inflector;

use crate::batbelt::bat_dialoguer::BatDialoguer;

use crate::batbelt::metadata::context_accounts_metadata::ContextAccountsMetadata;
use crate::batbelt::metadata::entrypoint_metadata::EntrypointMetadata;
use crate::batbelt::metadata::function_dependencies_metadata::FunctionDependenciesMetadata;
use crate::batbelt::metadata::functions_source_code_metadata::{
    FunctionMetadataType, FunctionSourceCodeMetadata,
};
use crate::batbelt::metadata::miro_metadata::MiroCodeOverhaulMetadata;
use crate::batbelt::metadata::structs_source_code_metadata::{
    StructMetadataType, StructSourceCodeMetadata,
};
use crate::batbelt::metadata::trait_metadata::TraitMetadata;
use crate::batbelt::metadata::traits_source_code_metadata::{
    TraitMetadataType, TraitSourceCodeMetadata,
};
use crate::batbelt::parser::parse_formatted_path;
use crate::batbelt::parser::source_code_parser::SourceCodeParser;
use crate::batbelt::BatEnumerator;
use crate::Suggestion;
use error_stack::{FutureExt, IntoReport, Report, Result, ResultExt};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::enums_source_code_metadata::{
    EnumMetadataType, EnumSourceCodeMetadata,
};
use crate::config::{BatAuditorConfig, BatConfig};
use serde_json::{json, Value};
use strum::IntoEnumIterator;
use walkdir::DirEntry;

#[derive(Debug)]
pub struct MetadataError;

impl fmt::Display for MetadataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Metadata error")
    }
}

impl Error for MetadataError {}

pub type MetadataResult<T> = Result<T, MetadataError>;

pub type MetadataId = String;

#[derive(Serialize, Deserialize, Clone)]
pub enum BatMetadataCommit {
    RunSonarMetadataCommit,
    MiroMetadataCommit,
    UpdateMetadataVersion,
}

impl BatMetadataCommit {
    pub fn get_commit_message(&self) -> String {
        match self {
            BatMetadataCommit::RunSonarMetadataCommit => {
                "metadata: bat-cli sonar executed".to_string()
            }
            BatMetadataCommit::MiroMetadataCommit => "metadata: miro metadata updated".to_string(),
            BatMetadataCommit::UpdateMetadataVersion => {
                "metadata: BatMetadata.json updated to last version".to_string()
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BatMetadata {
    // #[serde(default = "project_name_default")]
    #[serde(default)]
    pub project_name: String,
    pub initialized: bool,
    pub source_code: SourceCodeMetadata,
    pub entry_points: Vec<EntrypointMetadata>,
    pub function_dependencies: Vec<FunctionDependenciesMetadata>,
    pub traits: Vec<TraitMetadata>,
    pub context_accounts: Vec<ContextAccountsMetadata>,
    pub miro: MiroMetadata,
}

// fn project_name_default() -> String {
//     let bat_config = BatConfig::get_config().unwrap();
//     bat_config.project_name
// }

impl BatMetadata {
    pub fn new_empty() -> Self {
        let bat_config = BatConfig::get_config().unwrap();
        Self {
            project_name: bat_config.project_name,
            initialized: false,
            source_code: Default::default(),
            entry_points: vec![],
            function_dependencies: vec![],
            traits: vec![],
            context_accounts: vec![],
            miro: Default::default(),
        }
    }

    pub fn create_metadata_id() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        s
    }

    // set = Some -> writes bool env
    // set = None -> reads env
    pub fn parse_external_metadata_env(set_value: Option<bool>) -> MetadataResult<bool> {
        let env_variable_name = "USE_EXTERNAL_METADATA";
        match set_value {
            // read
            None => {
                let env_var = env::var(env_variable_name);
                Ok(env_var.is_ok() && env_var.unwrap() == "true".to_string())
            }
            // write
            Some(new_value) => {
                let bat_auditor_config =
                    BatAuditorConfig::get_config().change_context(MetadataError)?;
                if new_value == true && bat_auditor_config.external_bat_metadata.is_empty() {
                    return Err(Report::new(MetadataError).attach_printable(
                        "external_bat_metadata vector is empty on BatAuditor.toml",
                    ))
                    .attach(Suggestion(format!(
                        "run {} to add external BatMetadata.json files",
                        "bat-cli reload".bright_green()
                    )));
                }
                env::set_var(env_variable_name, if new_value { "true" } else { "false" });
                Ok(new_value == true)
            }
        }
    }

    pub fn read_metadata() -> MetadataResult<Self> {
        if Self::parse_external_metadata_env(None)? {
            return Self::read_external_metadata();
        }
        let metadata_json_bat_file = BatFile::BatMetadataFile;
        let bat_metadata_value: Value = serde_json::from_str(
            &metadata_json_bat_file
                .read_content(true)
                .change_context(MetadataError)?,
        )
        .into_report()
        .change_context(MetadataError)?;
        let mut bat_metadata: BatMetadata = serde_json::from_value(bat_metadata_value)
            .into_report()
            .change_context(MetadataError)?;
        if bat_metadata.project_name.is_empty() {
            bat_metadata.project_name = BatConfig::get_config()
                .change_context(MetadataError)?
                .project_name;
            bat_metadata.save_metadata()?;
            GitCommit::UpdateMetadataJson {
                bat_metadata_commit: BatMetadataCommit::UpdateMetadataVersion,
            }
            .create_commit()
            .change_context(MetadataError)?;
        }
        Ok(bat_metadata)
    }

    pub fn read_external_metadata() -> MetadataResult<Self> {
        let bat_auditor_config = BatAuditorConfig::get_config().change_context(MetadataError)?;
        if bat_auditor_config.external_bat_metadata.is_empty() {
            return Err(Report::new(MetadataError)
                .attach_printable("external_bat_metadata vector is empty on BatAuditor.toml"))
            .attach(Suggestion(format!(
                "run {} to add external BatMetadata.json files",
                "bat-cli reload".bright_green()
            )));
        }
        let prompt_text = format!("Select the {} file to use:", "BatMetadata.json".green());
        let selection = BatDialoguer::select(
            prompt_text,
            bat_auditor_config.external_bat_metadata.clone(),
            None,
        )
        .change_context(MetadataError)?;
        let external_bat_metadata_selected =
            bat_auditor_config.external_bat_metadata[selection].clone();
        let metadata_json_bat_file = BatFile::Generic {
            file_path: external_bat_metadata_selected.clone(),
        };
        let bat_metadata_value: Value = serde_json::from_str(
            &metadata_json_bat_file
                .read_content(true)
                .change_context(MetadataError)?,
        )
        .into_report()
        .change_context(MetadataError)?;
        let bat_metadata: BatMetadata = serde_json::from_value(bat_metadata_value)
            .into_report()
            .attach_printable(format!(
                "{} file at path {} is incompatible with this bat-cli version",
                "BatMetadata.json".bright_green(),
                external_bat_metadata_selected.clone()
            ))
            .attach(Suggestion(format!(
                "run {} at {} to update the BatMetadata.json version",
                "bat-cli sonar".bright_green(),
                external_bat_metadata_selected.clone().bright_yellow()
            )))
            .change_context(MetadataError)?;
        Ok(bat_metadata)
    }

    pub fn save_metadata(&self) -> MetadataResult<()> {
        let bat_config = BatConfig::get_config().change_context(MetadataError)?;
        if self.project_name != bat_config.project_name {
            return Err(Report::new(MetadataError).attach_printable(format!(
                "Error saving {}, expected project_name {:#?}, got {:#?}",
                "BatMetadata.json".bright_green(),
                bat_config.project_name,
                self.project_name
            )));
        }
        let metadata_json_bat_file = BatFile::BatMetadataFile;
        let metadata_json = json!(&self);
        let metadata_json_pretty = serde_json::to_string_pretty(&metadata_json)
            .into_report()
            .change_context(MetadataError)?;
        metadata_json_bat_file
            .write_content(false, &metadata_json_pretty)
            .change_context(MetadataError)?;
        Ok(())
    }

    pub fn get_entrypoint_metadata_by_name(
        &self,
        entry_point_name: String,
    ) -> MetadataResult<EntrypointMetadata> {
        if self.entry_points.is_empty() {
            return Err(MetadataErrorReports::EntryPointsMetadataNotInitialized.get_error_report());
        }
        match self
            .entry_points
            .clone()
            .into_iter()
            .find(|ep| ep.name == entry_point_name)
        {
            None => Err(
                MetadataErrorReports::EntryPointNameNotFound { entry_point_name }
                    .get_error_report(),
            ),
            Some(ep) => Ok(ep),
        }
    }

    pub fn get_functions_dependencies_metadata_by_function_metadata_id(
        &self,
        function_metadata_id: String,
    ) -> MetadataResult<FunctionDependenciesMetadata> {
        if self.function_dependencies.is_empty() {
            return Err(
                MetadataErrorReports::FunctionDependenciesMetadataNotInitialized.get_error_report(),
            );
        }
        match self
            .function_dependencies
            .clone()
            .into_iter()
            .find(|ep| ep.function_metadata_id == function_metadata_id)
        {
            None => Err(MetadataErrorReports::FunctionDependenciesNotFound {
                function_metadata_id,
            }
            .get_error_report()),
            Some(metadata) => Ok(metadata),
        }
    }

    pub fn get_trait_metadata_by_trait_source_code_metadata_id(
        &self,
        trait_source_code_metadata_id: String,
    ) -> MetadataResult<TraitMetadata> {
        if self.function_dependencies.is_empty() {
            return Err(MetadataErrorReports::TraitsMetadataNotInitialized.get_error_report());
        }
        match self
            .traits
            .clone()
            .into_iter()
            .find(|meta| meta.trait_source_code_metadata_id == trait_source_code_metadata_id)
        {
            None => Err(MetadataErrorReports::TraitNotFound {
                trait_source_code_metadata_id,
            }
            .get_error_report()),
            Some(metadata) => Ok(metadata),
        }
    }

    pub fn get_context_accounts_metadata_by_struct_source_code_metadata_id(
        &self,
        struct_source_code_metadata_id: String,
    ) -> MetadataResult<ContextAccountsMetadata> {
        if self.context_accounts.is_empty() {
            return Err(
                MetadataErrorReports::ContextAccountsMetadataNotInitialized.get_error_report()
            );
        }
        match self
            .context_accounts
            .clone()
            .into_iter()
            .find(|meta| meta.struct_source_code_metadata_id == struct_source_code_metadata_id)
        {
            None => Err(MetadataErrorReports::ContextAccountsNotFound {
                struct_source_code_metadata_id,
            }
            .get_error_report()),
            Some(metadata) => Ok(metadata),
        }
    }

    pub fn check_metadata_is_initialized(&self) -> Result<(), MetadataError> {
        if !self.initialized {
            return Err(MetadataErrorReports::MetadataNotInitialized.get_error_report());
        }
        Ok(())
    }
}

enum MetadataErrorReports {
    MetadataNotInitialized,
    MetadataIdNotFound {
        metadata_id: MetadataId,
    },
    EntryPointsMetadataNotInitialized,
    EntryPointNameNotFound {
        entry_point_name: String,
    },
    FunctionDependenciesMetadataNotInitialized,
    FunctionDependenciesNotFound {
        function_metadata_id: MetadataId,
    },
    TraitsMetadataNotInitialized,
    TraitNotFound {
        trait_source_code_metadata_id: MetadataId,
    },
    ContextAccountsMetadataNotInitialized,
    ContextAccountsNotFound {
        struct_source_code_metadata_id: MetadataId,
    },
    MiroCodeOverhaulMetadataNotInitialized,
    MiroCodeOverhaulMetadataNotFound {
        entry_point_name: String,
    },
}

impl MetadataErrorReports {
    pub fn get_error_report(&self) -> Report<MetadataError> {
        let initialize_suggestion = Suggestion(format!(
            "Initialize the BatMetadata by running {}",
            "bat-cli sonar".green()
        ));

        let message = match self {
            MetadataErrorReports::MetadataNotInitialized => {
                "Metadata is not initialized".to_string()
            }
            MetadataErrorReports::MetadataIdNotFound { metadata_id } => {
                format!("Metadata not found for {}", metadata_id.red())
            }
            MetadataErrorReports::EntryPointsMetadataNotInitialized => {
                "Entry point metadata has not been initialized".to_string()
            }
            MetadataErrorReports::EntryPointNameNotFound { entry_point_name } => {
                format!(
                    "Entry point metadata not found for {}",
                    entry_point_name.red()
                )
            }
            MetadataErrorReports::FunctionDependenciesMetadataNotInitialized => {
                "Function dependencies metadata has not been initialized".to_string()
            }
            MetadataErrorReports::FunctionDependenciesNotFound {
                function_metadata_id,
            } => {
                format!(
                    "Entry point metadata not found for {} id",
                    function_metadata_id.red()
                )
            }
            MetadataErrorReports::TraitsMetadataNotInitialized => {
                "Traits metadata has not been initialized".to_string()
            }
            MetadataErrorReports::TraitNotFound {
                trait_source_code_metadata_id: trait_metadata_id,
            } => {
                format!(
                    "Trait metadata not found for {} id",
                    trait_metadata_id.red()
                )
            }
            MetadataErrorReports::ContextAccountsMetadataNotInitialized => {
                "Context accounts metadata has not been initialized".to_string()
            }
            MetadataErrorReports::ContextAccountsNotFound {
                struct_source_code_metadata_id,
            } => {
                format!(
                    "Context accounts metadata not found for {} id",
                    struct_source_code_metadata_id.red()
                )
            }
            MetadataErrorReports::MiroCodeOverhaulMetadataNotInitialized => {
                "Miro code-overhaul's metadata has not been initialized".to_string()
            }
            MetadataErrorReports::MiroCodeOverhaulMetadataNotFound { entry_point_name } => {
                format!(
                    "Miro code-overhaul's metadata not found for {:#?} entry point",
                    entry_point_name.red()
                )
            }
        };
        Report::new(MetadataError)
            .attach_printable(message)
            .attach(initialize_suggestion)
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct MiroMetadata {
    pub code_overhaul: Vec<MiroCodeOverhaulMetadata>,
}

impl MiroMetadata {
    pub fn new(code_overhaul: Vec<MiroCodeOverhaulMetadata>) -> Self {
        Self { code_overhaul }
    }

    pub fn get_co_metadata_by_entrypoint_name(
        entry_point_name: String,
    ) -> MetadataResult<MiroCodeOverhaulMetadata> {
        let bat_metadata = BatMetadata::read_metadata()?;
        if bat_metadata.miro.code_overhaul.is_empty() {
            return Err(
                MetadataErrorReports::MiroCodeOverhaulMetadataNotInitialized.get_error_report()
            );
        }
        match bat_metadata
            .miro
            .code_overhaul
            .into_iter()
            .find(|meta| meta.entry_point_name == entry_point_name)
        {
            None => {
                Err(
                    MetadataErrorReports::MiroCodeOverhaulMetadataNotFound { entry_point_name }
                        .get_error_report(),
                )
            }
            Some(co_meta) => Ok(co_meta),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct SourceCodeMetadata {
    pub functions_source_code: Vec<FunctionSourceCodeMetadata>,
    pub structs_source_code: Vec<StructSourceCodeMetadata>,
    pub traits_source_code: Vec<TraitSourceCodeMetadata>,
    pub enums_source_code: Vec<EnumSourceCodeMetadata>,
}

impl SourceCodeMetadata {
    pub fn get_function_by_id(
        &self,
        metadata_id: MetadataId,
    ) -> MetadataResult<FunctionSourceCodeMetadata> {
        let result = self
            .functions_source_code
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        match result {
            Some(f_metadata) => Ok(f_metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        }
    }

    pub fn get_struct_by_id(
        &self,
        metadata_id: MetadataId,
    ) -> MetadataResult<StructSourceCodeMetadata> {
        let result = self
            .structs_source_code
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        match result {
            Some(metadata) => Ok(metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        }
    }

    pub fn get_trait_by_id(
        &self,
        metadata_id: MetadataId,
    ) -> MetadataResult<TraitSourceCodeMetadata> {
        let result = self
            .traits_source_code
            .clone()
            .into_iter()
            .find(|meta| meta.metadata_id == metadata_id);
        match result {
            Some(metadata) => Ok(metadata),
            None => {
                Err(MetadataErrorReports::MetadataIdNotFound { metadata_id }.get_error_report())
            }
        }
    }

    pub fn update_functions(&self, new_vec: Vec<FunctionSourceCodeMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let mut metadata_vec = new_vec;
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.functions_source_code = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }

    pub fn update_structs(&self, new_vec: Vec<StructSourceCodeMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let mut metadata_vec = new_vec;
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.structs_source_code = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }

    pub fn update_traits(&self, new_vec: Vec<TraitSourceCodeMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let mut metadata_vec = new_vec;
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.traits_source_code = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }

    pub fn update_enums(&self, new_vec: Vec<EnumSourceCodeMetadata>) -> MetadataResult<()> {
        let mut bat_metadata = BatMetadata::read_metadata()?;
        let mut metadata_vec = new_vec;
        metadata_vec.sort_by_key(|metadata_item| metadata_item.name());
        bat_metadata.source_code.enums_source_code = metadata_vec;
        bat_metadata.save_metadata()?;
        Ok(())
    }

    pub fn get_filtered_structs(
        struct_name: Option<String>,
        struct_type: Option<StructMetadataType>,
    ) -> MetadataResult<Vec<StructSourceCodeMetadata>> {
        Ok(BatMetadata::read_metadata()?
            .source_code
            .structs_source_code
            .into_iter()
            .filter(|struct_metadata| {
                if struct_name.is_some() && struct_name.clone().unwrap() != struct_metadata.name {
                    return false;
                };
                if struct_type.is_some() && struct_type.unwrap() != struct_metadata.struct_type {
                    return false;
                };
                true
            })
            .collect::<Vec<_>>())
    }

    pub fn find_struct(
        struct_name: String,
        struct_type: StructMetadataType,
    ) -> MetadataResult<StructSourceCodeMetadata> {
        match BatMetadata::read_metadata()?
            .source_code
            .structs_source_code
            .into_iter()
            .find(|struct_metadata| {
                struct_metadata.struct_type == struct_type && struct_metadata.name == struct_name
            }) {
            None => Err(Report::new(MetadataError).attach_printable(format!(
                "Metadata not found for struct with name {} and struct type {}",
                struct_name, struct_type
            ))),
            Some(struct_metadata) => Ok(struct_metadata),
        }
    }

    pub fn get_filtered_functions(
        function_name: Option<String>,
        function_type: Option<FunctionMetadataType>,
    ) -> MetadataResult<Vec<FunctionSourceCodeMetadata>> {
        Ok(BatMetadata::read_metadata()?
            .source_code
            .functions_source_code
            .into_iter()
            .filter(|function_metadata| {
                if function_name.is_some()
                    && function_name.clone().unwrap() != function_metadata.name
                {
                    return false;
                };
                if function_type.is_some()
                    && function_type.unwrap() != function_metadata.function_type
                {
                    return false;
                };
                true
            })
            .collect::<Vec<_>>())
    }
    pub fn get_filtered_traits(
        trait_name: Option<String>,
        trait_type: Option<TraitMetadataType>,
    ) -> MetadataResult<Vec<TraitSourceCodeMetadata>> {
        Ok(BatMetadata::read_metadata()?
            .source_code
            .traits_source_code
            .into_iter()
            .filter(|trait_metadata| {
                if trait_name.is_some() && trait_name.clone().unwrap() != trait_metadata.name {
                    return false;
                };
                if trait_type.is_some() && trait_type.unwrap() != trait_metadata.trait_type {
                    return false;
                };
                true
            })
            .collect::<Vec<_>>())
    }
    pub fn get_filtered_enums(
        trait_name: Option<String>,
        trait_type: Option<EnumMetadataType>,
    ) -> MetadataResult<Vec<EnumSourceCodeMetadata>> {
        Ok(BatMetadata::read_metadata()?
            .source_code
            .enums_source_code
            .into_iter()
            .filter(|enum_metadata| {
                if trait_name.is_some() && trait_name.clone().unwrap() != enum_metadata.name {
                    return false;
                };
                if trait_type.is_some() && trait_type.unwrap() != enum_metadata.enum_type {
                    return false;
                };
                true
            })
            .collect::<Vec<_>>())
    }
}

#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    Default,
    strum_macros::Display,
    strum_macros::EnumIter,
    Serialize,
    Deserialize,
)]
pub enum BatMetadataType {
    #[default]
    Struct,
    Function,
    Trait,
    Enum,
}

impl BatMetadataType {
    pub fn prompt_metadata_type_selection() -> Result<Self, MetadataError> {
        let metadata_types_vec = BatMetadataType::get_type_vec();
        let metadata_types_colorized_vec = BatMetadataType::get_colorized_type_vec(true);
        // Choose metadata section selection
        let prompt_text = format!("Please select the {}", "Metadata type".bright_purple());
        let selection =
            BatDialoguer::select(prompt_text, metadata_types_colorized_vec, None).unwrap();
        let metadata_type_selected = &metadata_types_vec[selection];
        Ok(*metadata_type_selected)
    }
}

pub trait BatMetadataParser<U>
where
    Self: Sized + Clone,
    U: BatEnumerator,
{
    fn name(&self) -> String;
    fn path(&self) -> String;
    fn metadata_id(&self) -> MetadataId;
    fn start_line_index(&self) -> usize;
    fn end_line_index(&self) -> usize;
    fn metadata_sub_type(&self) -> U;
    fn get_bat_metadata_type() -> BatMetadataType;

    fn metadata_name() -> String;

    fn value_to_vec_string(value: Value) -> MetadataResult<Vec<String>> {
        Ok(value
            .as_array()
            .ok_or(MetadataError)
            .into_report()?
            .iter()
            .map(|val| val.as_str().ok_or(MetadataError).into_report())
            .collect::<Result<Vec<_>, MetadataError>>()?
            .into_iter()
            .map(|val| val.to_string())
            .collect::<Vec<_>>())
    }

    fn new(
        path: String,
        name: String,
        metadata_sub_type: U,
        start_line_index: usize,
        end_line_index: usize,
        metadata_id: MetadataId,
    ) -> Self;

    fn create_metadata_id() -> String {
        let s: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();
        s
    }

    fn to_source_code_parser(&self, optional_name: Option<String>) -> SourceCodeParser {
        SourceCodeParser::new(
            if let Some(function_name) = optional_name {
                function_name
            } else {
                self.name()
            },
            self.path(),
            self.start_line_index(),
            self.end_line_index(),
        )
    }
    fn create_metadata_from_dir_entry(entry: DirEntry) -> Result<Vec<Self>, MetadataError>;
}

impl BatEnumerator for BatMetadataType {}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum BatMetadataMarkdownContent {
    Path,
    Name,
    Type,
    StartLineIndex,
    EndLineIndex,
    MetadataId,
}

impl BatMetadataMarkdownContent {
    pub fn get_prefix(&self) -> String {
        format!("- {}:", self.to_snake_case())
    }

    pub fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    pub fn get_info_section_content<T: Display>(&self, content_value: T) -> String {
        format!("- {}: {}", self.to_snake_case(), content_value)
    }
}

// #[cfg(debug_assertions)]
// mod metadata_test {
//     use assert_fs::prelude::FileWriteStr;
//     use serde_json::{json, Value};
//     use std::fs;
//
//     const TEMP_PATH: &'static str = "./test.json";
//
//     // #[test]
//     // fn test_metadata() {
//     //     //save to json
//     //     let key = "hello";
//     //     let value = vec!["world".to_string()];
//     //     let json_content = json!({ key: value });
//     //
//     //     let pretty_content = serde_json::to_string_pretty(&json_content).unwrap();
//     //     assert_fs::NamedTempFile::new(TEMP_PATH).unwrap();
//     //     fs::write(TEMP_PATH, &pretty_content).unwrap();
//     //
//     //     let vec_value = read_key(key);
//     //     let vec_read = value_to_vec_string(vec_value);
//     //
//     //     assert_eq!(value, vec_read);
//     //
//     //     let value_2 = vec!["chai".to_string()];
//     //     let vec_value = json!(value_2);
//     //     save_key(key, vec_value);
//     //
//     //     let vec_value_read = read_key(key);
//     //     let vec_read = value_to_vec_string(vec_value_read);
//     //
//     //     assert_eq!(vec_read, value_2);
//     // }
// }
