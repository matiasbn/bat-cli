pub mod path {
    use super::*;
    pub fn get_instruction_file_path_from_started_entrypoint_co_file(
        entrypoint_name: String,
    ) -> Result<String, String> {
        let co_file_path =
            get_auditor_code_overhaul_started_file_path(Some(entrypoint_name.clone()))?;
        let program_path = BatConfig::get_validated_config()?
            .required
            .program_lib_path
            .replace("/lib.rs", "")
            .replace("../", "");
        let started_file_string = fs::read_to_string(co_file_path.clone()).unwrap();
        let instruction_file_path = started_file_string
            .lines()
            .into_iter()
            .find(|f| f.contains(&program_path))
            .expect(&format!(
                "co file of {} does not contain the instruction path yet",
                entrypoint_name,
            ))
            .to_string();
        Ok(instruction_file_path)
    }
    pub fn get_audit_folder_path(file_name: Option<String>) -> Result<String, String> {
        if let Some(file_name_option) = file_name {
            Ok(canonicalize_path(
                BatConfig::get_validated_config()
                    .unwrap()
                    .required
                    .audit_folder_path
                    + "/"
                    + file_name_option.as_str(),
            )
            .unwrap())
        } else {
            Ok(canonicalize_path(
                BatConfig::get_validated_config()?
                    .required
                    .audit_folder_path,
            )
            .unwrap())
        }
    }

    pub fn get_readme_file_path() -> Result<String, String> {
        canonicalize_path(get_audit_folder_path(None)? + "/README.md")
    }

    pub fn get_program_lib_path() -> Result<String, String> {
        canonicalize_path(BatConfig::get_validated_config()?.required.program_lib_path)
    }

    pub fn get_notes_path() -> Result<String, String> {
        Ok(get_audit_folder_path(None)? + "/notes/")
    }

    pub fn get_auditor_notes_path() -> Result<String, String> {
        Ok(get_notes_path()? + &BatConfig::get_auditor_name()? + "-notes/")
    }

    // Findings paths
    pub fn get_auditor_findings_path() -> Result<String, String> {
        Ok(get_auditor_notes_path()? + "findings/")
    }

    pub fn get_auditor_findings_to_review_path(
        file_name: Option<String>,
    ) -> Result<String, String> {
        match file_name {
            Some(name) => {
                Ok(get_auditor_findings_path()? + "to-review/" + &name.replace(".md", "") + ".md")
            }
            None => Ok(get_auditor_findings_path()? + "to-review/"),
        }
    }

    pub fn get_auditor_findings_accepted_path(file_name: Option<String>) -> Result<String, String> {
        match file_name {
            Some(name) => {
                Ok(get_auditor_findings_path()? + "accepted/" + &name.replace(".md", "") + ".md")
            }
            None => Ok(get_auditor_findings_path()? + "accepted/"),
        }
    }

    pub fn get_auditor_findings_rejected_path(file_name: Option<String>) -> Result<String, String> {
        match file_name {
            Some(name) => {
                Ok(get_auditor_findings_path()? + "rejected/" + &name.replace(".md", "") + ".md")
            }
            None => Ok(get_auditor_findings_path()? + "rejected/"),
        }
    }

    // Code overhaul paths
    pub fn get_auditor_code_overhaul_path() -> Result<String, String> {
        Ok(get_auditor_notes_path()? + "code-overhaul/")
    }

    pub fn get_auditor_code_overhaul_to_review_path(
        file_name: Option<String>,
    ) -> Result<String, String> {
        match file_name {
            Some(name) => Ok(get_auditor_code_overhaul_path()?
                + "to-review/"
                + &name.replace(".md", "")
                + ".md"),
            None => Ok(get_auditor_code_overhaul_path()? + "to-review/"),
        }
    }

    pub fn get_auditor_code_overhaul_finished_path(
        file_name: Option<String>,
    ) -> Result<String, String> {
        match file_name {
            Some(name) => Ok(get_auditor_code_overhaul_path()?
                + "finished/"
                + &name.replace(".md", "")
                + ".md"),
            None => Ok(get_auditor_code_overhaul_path()? + "finished/"),
        }
    }

    pub fn get_auditor_code_overhaul_started_file_path(
        file_name: Option<String>,
    ) -> Result<String, String> {
        match file_name {
            Some(name) => {
                if MiroConfig::new().miro_enabled() {
                    let entrypoint_name = &name.replace(".md", "");
                    Ok(canonicalize_path(format!(
                        "{}/started/{entrypoint_name}/{entrypoint_name}.md",
                        get_auditor_code_overhaul_path()?
                    ))?)
                } else {
                    Ok(get_auditor_code_overhaul_path()?
                        + "started/"
                        + &name.replace(".md", "")
                        + ".md")
                }
            }
            None => Ok(get_auditor_code_overhaul_path()? + "started/"),
        }
    }

    // Templates path
    pub fn get_templates_path() -> Result<String, String> {
        Ok(get_audit_folder_path(None)? + "/templates")
    }

    pub fn get_finding_template_path() -> Result<String, String> {
        Ok(get_templates_path()? + "/finding.md")
    }

    pub fn get_informational_template_path() -> Result<String, String> {
        Ok(get_templates_path()? + "/informational.md")
    }

    pub fn get_code_overhaul_template_path() -> Result<String, String> {
        Ok(get_templates_path()? + "/code-overhaul.md")
    }

    // Threat modeling file
    pub fn get_auditor_threat_modeling_path() -> Result<String, String> {
        Ok(get_auditor_notes_path()? + "threat_modeling.md")
    }
}
