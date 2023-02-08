pub mod entrypoints {
    use crate::batbelt::sonar::{BatSonar, SonarResultType};

    use crate::config::{BatConfig, RequiredConfig};

    pub fn get_entrypoints_names() -> Result<Vec<String>, String> {
        let BatConfig { required, .. } = BatConfig::get_validated_config()?;

        let RequiredConfig {
            program_lib_path, ..
        } = required;
        let bat_sonar = BatSonar::new_from_path(
            &program_lib_path,
            Some("#[program"),
            SonarResultType::Function,
        );
        let entrypoints_names = bat_sonar
            .results
            .iter()
            .map(|entrypoint| entrypoint.name.clone())
            .collect();
        Ok(entrypoints_names)
    }
}
