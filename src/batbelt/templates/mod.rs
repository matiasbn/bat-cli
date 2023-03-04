pub mod code_overhaul_template;
pub mod finding_template;
pub mod notes_template;
pub mod package_json_template;

use super::*;
use crate::batbelt;
use crate::batbelt::command_line::{execute_command, execute_command_with_child_process};
use crate::batbelt::metadata::{BatMetadata, BatMetadataType, MetadataError};
use crate::batbelt::path::{BatFile, BatFolder};
use crate::batbelt::templates::notes_template::NoteTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::batbelt::BatEnumerator;
use crate::config::BatConfig;
use error_stack::{IntoReport, Report, Result, ResultExt};
use inflector::Inflector;
use serde_json::json;
use std::path::Path;
use std::{env, error::Error, fmt, fs};

#[derive(Debug)]
pub struct TemplateError;

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Template error")
    }
}

impl Error for TemplateError {}

pub type TemplateResult<T> = Result<T, TemplateError>;

pub struct TemplateGenerator;

impl TemplateGenerator {
    pub fn create_project() -> Result<(), TemplateError> {
        Self::create_project_folder()?;
        let project_path = BatFolder::ProjectFolderPath
            .get_path(true)
            .change_context(TemplateError)?;
        execute_command_with_child_process(
            "mv",
            &[
                &BatFile::BatToml
                    .get_path(false)
                    .change_context(TemplateError)?,
                &project_path,
            ],
        )
        .change_context(TemplateError)?;
        env::set_current_dir(&project_path)
            .into_report()
            .change_context(TemplateError)?;
        Self::create_init_notes_folder()?;
        BatFile::GitIgnore
            .write_content(false, &Self::get_git_ignore_content())
            .change_context(TemplateError)?;
        Self::create_readme()?;
        Self::create_metadata_json()?;
        PackageJsonTemplate::create_package_with_init_script()?;
        BatFile::Batlog
            .create_empty(false)
            .change_context(TemplateError)?;
        Ok(())
    }
    pub fn get_git_ignore_content() -> String {
        ".idea\n\
        ./package.json\n\
        BatAuditor.toml\n\
        Batlog.log"
            .to_string()
    }

    pub fn create_metadata_json() -> TemplateResult<()> {
        let metadata_json_bat_file = BatFile::BatMetadataFile;
        let new_bat_metadata = BatMetadata::new_empty();
        metadata_json_bat_file
            .create_empty(false)
            .change_context(TemplateError)?;
        new_bat_metadata
            .save_metadata()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn create_project_folder() -> Result<(), TemplateError> {
        let bat_config = BatConfig::get_config().change_context(TemplateError)?;
        fs::create_dir(format!("./{}", bat_config.project_name))
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn create_init_notes_folder() -> Result<(), TemplateError> {
        fs::create_dir(format!("./notes"))
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn create_readme() -> Result<(), TemplateError> {
        let bat_config = BatConfig::get_config().change_context(TemplateError)?;
        let content = format!(
            r#"# Project Name

{}

# Commit Hash

{}

# Miro board

{}

# Starting Date

{}

# Ending Date 

{}
"#,
            bat_config.project_name,
            bat_config.commit_hash_url,
            bat_config.miro_board_url,
            bat_config.starting_date,
            TemplatePlaceholder::EmptyEndingDate.to_placeholder()
        );
        let path = format!("./README.md");
        fs::write(path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    pub fn create_auditor_folders() -> Result<(), TemplateError> {
        let auditor_notes_path = BatFolder::AuditorNotes
            .get_path(false)
            .change_context(TemplateError)?;
        Self::create_dir(&auditor_notes_path, false)?;
        // Code overhaul
        Self::create_code_overhaul_folders()?;

        // Figures
        let auditor_figues = BatFolder::AuditorFigures
            .get_path(false)
            .change_context(TemplateError)?;
        Self::create_dir(&auditor_figues, true)?;

        // findings
        Self::create_findings_folders()?;

        // metadata
        let auditor_metadata_path = BatFolder::MetadataFolder
            .get_path(false)
            .change_context(TemplateError)?;
        Self::create_dir(&auditor_metadata_path, false)?;
        NoteTemplate::create_notes_templates()?;
        Ok(())
    }

    fn create_code_overhaul_folders() -> Result<(), TemplateError> {
        let finished_co_path = BatFolder::CodeOverhaulFinished
            .get_path(false)
            .change_context(TemplateError)?;
        let started_co_path = BatFolder::CodeOverhaulStarted
            .get_path(false)
            .change_context(TemplateError)?;
        let to_review_co_path = BatFolder::CodeOverhaulToReview
            .get_path(false)
            .change_context(TemplateError)?;
        Self::create_dir(&finished_co_path, true)?;
        Self::create_dir(&started_co_path, true)?;
        Self::create_dir(&to_review_co_path, false)?;
        Ok(())
    }

    fn create_findings_folders() -> Result<(), TemplateError> {
        let accepted_path = BatFolder::FindingsAccepted
            .get_path(false)
            .change_context(TemplateError)?;
        let rejected_path = BatFolder::FindingsRejected
            .get_path(false)
            .change_context(TemplateError)?;
        let to_review_path = BatFolder::FindingsToReview
            .get_path(false)
            .change_context(TemplateError)?;
        Self::create_dir(&accepted_path, true)?;
        Self::create_dir(&rejected_path, true)?;
        Self::create_dir(&to_review_path, true)?;
        Ok(())
    }

    fn create_gitkeep(path: &str) -> Result<(), TemplateError> {
        let gitkeep_path = format!("{}/.gitkeep", path);
        execute_command("touch", &[&gitkeep_path], false).change_context(TemplateError)?;
        Ok(())
    }

    fn create_dir(path: &str, create_git_keep: bool) -> Result<(), TemplateError> {
        fs::create_dir_all(path)
            .into_report()
            .change_context(TemplateError)?;
        if create_git_keep {
            Self::create_gitkeep(path)?;
        }
        Ok(())
    }

    pub fn create_robot_file() -> Result<(), TemplateError> {
        let robot_file_path = BatFile::RobotFile
            .get_path(false)
            .change_context(TemplateError)?;
        if Path::new(&robot_file_path).is_file() {
            return Err(
                Report::new(TemplateError).attach_printable("Robot file already initialized")
            );
        }
        let ending_date =
            batbelt::bat_dialoguer::input("Ending date").change_context(TemplateError)?;
        let bat_readme = BatFile::Readme;
        let readme_content = bat_readme
            .read_content(true)
            .change_context(TemplateError)?;
        let updated_content = readme_content.replace(
            &TemplatePlaceholder::EmptyEndingDate.to_placeholder(),
            &ending_date,
        );
        bat_readme
            .write_content(true, &updated_content)
            .change_context(TemplateError)?;
        let robot_content = Self::robot_file_content(&ending_date)?;
        fs::write(robot_file_path, robot_content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn robot_file_content(ending_date: &str) -> Result<String, TemplateError> {
        let bat_config = BatConfig::get_config().change_context(TemplateError)?;
        let content = format!(
            r#"# EXECUTIVE SUMMARY

## Overview

{} engaged Kudelski Security to perform a code review of the {} program.

The assessment was conducted remotely by the Kudelski Security Team. Testing took place between {} and {}, and it was focused on the following objectives:

- Provide the customer with an assessment of their overall security posture and any risks that were discovered within the environment during the engagement.
- To provide a professional opinion on the maturity, adequacy, and efficiency of the security measures that are in place.
- To identify potential issues and include improvement recommendations based on the result of our tests.

During the Secure Code Review, we identified $findings_summary findings according to our Vulnerability Scoring System.

This report summarizes the engagement, tests performed, and details of the mentioned findings.

It also contains detailed descriptions of the discovered vulnerabilities, steps the Kudelski Security Teams took to identify and validate each issue,as well as any applicable recommendations for remediation.

## Key findings

The following are the major themes and issues identified during the testing period.

These, along with other items, within the findings section, should be prioritized for remediation to reduce to the risk they pose.

- KEY FINDING 1
- KEY FINDING 2
- KEY FINDING 3

## Scope and Rules of Engagement

Kudelski performed a Secure Code Review for {}.

The following table documents the targets in scope for the engagement. No additional systems or resources were in scope for this assessment.

COMPLETE WITH SCOPE AND RULES OF ENGAGEMENT OF THE DOCX FILE

## Findings summary

During the Secure Code Review, we identified $findings_summary findings according to our Vulnerability Scoring System.

The following chart displays the issues by severity:

$findings_summary_chart

The following table provides an overview of the findings:

$findings_table
"#,
            bat_config.client_name,
            bat_config
                .project_name
                .trim_end_matches("-audit")
                .to_snake_case(),
            bat_config.starting_date,
            ending_date,
            bat_config.client_name
        );
        Ok(content)
    }
}

#[derive(Debug, PartialEq, Clone, Copy, strum_macros::Display, strum_macros::EnumIter)]
pub enum TemplatePlaceholder {
    EmptyEndingDate,
}

impl TemplatePlaceholder {
    pub fn to_placeholder(&self) -> String {
        self.to_string().to_screaming_snake_case()
    }
}

#[cfg(debug_assertions)]
mod template_test {
    use crate::batbelt::templates::TemplateGenerator;

    #[test]
    fn test_get_gitignore_content() {
        println!("{}", TemplateGenerator::get_git_ignore_content());
    }
}
