use std::{fs, process::Command};

use crate::{
    command_line::vs_code_open_file_in_current_window,
    config::BatConfig,
    package::format,
    utils::{
        self,
        bash::execute_command_to_stdio,
        git::{create_git_commit, GitCommit},
        helpers::get::{get_only_files_from_folder, get_string_between_two_str_from_string},
        path::{get_file_path, get_folder_path, FilePathType, FolderPathType},
    },
};

pub const FINDING_CODE_PREFIX: &str = "KS";
pub const RESULT_FINDINGS_SECTION_HEADER: &str = "# Findings result";
pub const RESULT_FINDINGS_TABLE_OF_FINDINGS_HEADER: &str = "## Table of findings";
pub const RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER: &str = "## List of Findings";
pub const RESULT_CODE_OVERHAUL_SECTION_HEADER: &str = "# Code overhaul result";

pub const HTML_TABLE_FORMAT: &str = "<style>
tr th {
    background: white;
    color:black;
    width: 2%;
    text-align: center;
    border: 2px solid black;
}

.alg tr {
    width: 2%;
    text-align: center;
    border: 2px solid black;
}
.alg thead tr th:nth-of-type(2) {
    width: 9%;
    text-align: center;
    border: 2px solid black;
}

tr td {
    background: white;
    color:black;
    width: 2%;
    text-align: center;
    border: 2px solid black;
}
.high {
    background: #fd0011;
    border: 2px solid yellow;
    text-align: center;
    color: black;
} 
.medium {
    background: #f58b45;
    border: 2px solid yellow;
    text-align: center;
    color: black;
}
.low {
    background: #16a54d;
    border: 2px solid yellow;
    text-align: center;
    color: black;
}
.informational{
    background: #0666b4;
    border: 2px solid yellow;
    text-align: center;
    color: black;
}
.open{
    background: #16a54d;
    border: 2px solid yellow;
    text-align: center;
    color: black;
}
</style>";

#[derive(PartialEq, Debug, Clone)]
enum StatusLevel {
    Open,
}

impl StatusLevel {
    pub fn from_str(status_str: &str) -> Self {
        let severity = status_str.to_lowercase();
        let severity_binding = severity.as_str();
        match severity_binding {
            "open" => StatusLevel::Open,
            _ => panic!("incorrect status level {}", severity_binding),
        }
    }

    pub fn get_html_content(&self) -> String {
        match self {
            StatusLevel::Open => "<td class='open'>Open</td>".to_string(),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
enum FindingLevel {
    High,
    Medium,
    Low,
    Informational,
}

impl FindingLevel {
    pub fn from_str(severity_str: &str) -> Self {
        let severity = severity_str.to_lowercase();
        let severity_binding = severity.as_str();
        match severity_binding {
            "high" => FindingLevel::High,
            "medium" => FindingLevel::Medium,
            "low" => FindingLevel::Low,
            "informational" => FindingLevel::Informational,
            _ => panic!("incorrect severity level {}", severity_binding),
        }
    }

    pub fn get_hex_color(&self) -> String {
        match self {
            FindingLevel::High => "#fd0011".to_string(),
            FindingLevel::Medium => "#f58b45".to_string(),
            FindingLevel::Low => "#16a54d".to_string(),
            FindingLevel::Informational => "#0666b4".to_string(),
        }
    }
    pub fn get_html_content(&self) -> String {
        match self {
            FindingLevel::High => "<td class='high'>High</td>".to_string(),
            FindingLevel::Medium => "<td class='medium'>Medium</td>".to_string(),
            FindingLevel::Low => "<td class='low'>Low</td>".to_string(),
            FindingLevel::Informational => "<td class='low'>Low</td>".to_string(),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct Finding {
    code: String,
    title: String,
    severity: FindingLevel,
    impact: Option<FindingLevel>,
    likelihood: Option<FindingLevel>,
    difficulty: Option<FindingLevel>,
    status: StatusLevel,
    content: String,
    index: usize,
}

impl Finding {
    pub fn new_from_path(finding_path: &str, index: usize) -> Self {
        let finding_content = fs::read_to_string(finding_path).unwrap();
        Self::new_from_str(&finding_content, index)
    }

    pub fn new_from_str(finding_content: &str, index: usize) -> Self {
        let content = Self::format_finding_content_header_with_finding_code(finding_content, index);
        let (code, title, severity_str, status, impact, likelihood, difficulty) =
            Self::parse_finding_data(&content);
        let severity = FindingLevel::from_str(&severity_str);
        Finding {
            code,
            title,
            severity,
            status,
            content,
            index,
            impact,
            likelihood,
            difficulty,
        }
    }

    fn parse_finding_data(
        finding_content: &str,
    ) -> (
        String,
        String,
        String,
        StatusLevel,
        Option<FindingLevel>,
        Option<FindingLevel>,
        Option<FindingLevel>,
    ) {
        let finding_content_lines = finding_content.lines();
        let finding_content_first_line = finding_content_lines.clone().next().unwrap();

        let finding_code = finding_content_first_line
            .clone()
            .strip_prefix(&format!("## "))
            .unwrap()
            .split(" ")
            .next()
            .unwrap()
            .replace(":", "");
        let finding_description = finding_content_first_line
            .strip_prefix(&format!("## {finding_code}: "))
            .unwrap()
            .trim();
        let finding_severity = finding_content_lines
            .clone()
            .find(|line| line.contains("**Severity:**"))
            .unwrap()
            .strip_prefix("**Severity:** ")
            .unwrap();

        let finding_status = finding_content_lines
            .clone()
            .find(|line| line.contains("**Status:**"))
            .unwrap()
            .strip_prefix("**Status:** ")
            .unwrap();
        let finding_status = StatusLevel::from_str(finding_status);
        if FindingLevel::from_str(finding_severity) == FindingLevel::Informational {
            return (
                finding_code.to_string(),
                finding_description.to_string(),
                finding_severity.to_string(),
                finding_status,
                None,
                None,
                None,
            );
        }
        let finding_table = get_string_between_two_str_from_string(
            finding_content.to_string(),
            "**Status:**",
            "### Description",
        )
        .unwrap();
        let severities = ["High", "Medium", "Low"];
        let status = finding_table
            .lines()
            .find(|line| severities.iter().any(|severity| line.contains(severity)))
            .unwrap();
        let status_splited: Vec<&str> = status
            .split("|")
            .map(|spl| spl.trim())
            .filter(|spl| severities.iter().any(|severity| spl.contains(severity)))
            .collect();
        let impact = FindingLevel::from_str(&status_splited[0]);
        let likelihood = FindingLevel::from_str(&status_splited[1]);
        let difficulty = FindingLevel::from_str(&status_splited[2]);
        (
            finding_code.to_string(),
            finding_description.to_string(),
            finding_severity.to_string(),
            finding_status,
            Some(impact),
            Some(likelihood),
            Some(difficulty),
        )
    }

    pub fn format_finding_content_header_with_finding_code(
        finding_content: &str,
        index: usize,
    ) -> String {
        let mut finding_content_lines = finding_content.lines();
        let content_first_line = finding_content_lines.next().unwrap();
        let text_to_replace = format!(
            "## {}-{}:",
            FINDING_CODE_PREFIX,
            if index < 9 {
                format!("0{}", index + 1)
            } else {
                format!("{}", index + 1)
            }
        );
        let formatted_header = content_first_line.replace("##", &text_to_replace);
        let formatted_finding_content =
            finding_content.replace(content_first_line, &formatted_header);
        formatted_finding_content
    }

    pub fn parse_finding_table_row(&self) -> String {
        format!(
            "|{}|{:#?}|{}|{:?}|",
            self.code, self.severity, self.title, self.status
        )
    }

    pub fn parse_finding_content_for_audit_folder_path(&self) -> String {
        self.content.replace("../../figures", "./figures")
    }

    pub fn parse_finding_content_for_root_path(&self) -> String {
        let audit_result_figures_path = get_folder_path(FolderPathType::AuditResultFigures, false);
        self.content
            .replace("../../figures", &audit_result_figures_path)
    }

    pub fn parse_finding_table_html(&self) -> String {
        if self.severity == FindingLevel::Informational {
            format!(
                "<table>
                <thead>
                  <tr>
                      <th style='background:white; font-weight:bold'>Severity</th>    {}    
                </thead>
                  </tr>
                <tbody>
                  <tr>
                      <td style='background:white; font-weight:bold'>Status</td>    {}
                  </tr>
                </tbody>
              </table>",
                self.severity.get_html_content(),
                self.status.get_html_content(),
            )
        } else {
            format!(
                "<table>
                <thead>
                  <tr>
                      <th style='background:white; font-weight:bold'>Severity</th>    {}    
                </thead>
                  </tr>
                <tbody>
                  <tr>
                      <td style='background:white; font-weight:bold'>Status</td>    {}
                  </tr>
                </tbody>
              </table>
              <table>
                <thead>
                  <tr>
                      <th>Impact</th>    <th>Likelihood</th>    <th>Difficulty</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                      {}    {}    {}
                  </tr>
                </tbody>
              </table>",
                self.severity.get_html_content(),
                self.status.get_html_content(),
                self.clone().impact.unwrap().get_html_content(),
                self.clone().likelihood.unwrap().get_html_content(),
                self.clone().difficulty.unwrap().get_html_content(),
            )
        }
    }
}

pub fn findings_result() -> Result<(), String> {
    // get the audit_result path
    let audit_result_temp_path =
        utils::path::get_folder_path(FolderPathType::AuditResultTemp, false);
    let audit_result_figures_path =
        utils::path::get_folder_path(FolderPathType::AuditResultFigures, true);
    let notes_folder = utils::path::get_folder_path(FolderPathType::Notes, true);

    // create a temp folder for the findings
    utils::bash::execute_command_to_stdio("mkdir", &[&audit_result_temp_path]).unwrap();
    // delete figures folder
    utils::bash::execute_command_to_stdio("rm", &["-rf", &audit_result_figures_path]).unwrap();
    // create figures folder
    utils::bash::execute_command_to_stdio("mkdir", &[&audit_result_figures_path]).unwrap();

    // copy all the data to the audit_result folder
    let auditor_names = BatConfig::get_validated_config()?.required.auditor_names;
    for auditor in auditor_names {
        let auditor_notes_path = format!("{}/{}-notes", notes_folder, auditor);
        let auditor_accepted_findings_path = format!("{}/findings/accepted", auditor_notes_path);

        let findings_files = get_only_files_from_folder(auditor_accepted_findings_path)?;
        // for each auditor, copy all the findings to the temp folder
        for finding_file in findings_files {
            utils::bash::execute_command_to_stdio(
                "cp",
                &[&finding_file.path, &audit_result_temp_path],
            )
            .unwrap();
        }

        // for each auditor, copy all the figures to the audit_result figures folder
        let auditor_figures_path = format!("{}/figures", auditor_notes_path);
        let figures_files = get_only_files_from_folder(auditor_figures_path)?;
        for figure_file in figures_files {
            utils::bash::execute_command_to_stdio(
                "cp",
                &[&figure_file.path, &audit_result_figures_path],
            )
            .unwrap();
        }
    }
    let findings_result_file_path = get_file_path(utils::path::FilePathType::FindingsResult, true);
    // remove previous findings_result.md file
    execute_command_to_stdio("rm", &[&findings_result_file_path]).unwrap();
    // create new findings_result.md file
    execute_command_to_stdio("touch", &[&findings_result_file_path]).unwrap();
    // create new findings_result.md file
    let findings_temp_files = get_only_files_from_folder(audit_result_temp_path.clone())?;
    let mut table_of_findings: String = format!("{RESULT_FINDINGS_SECTION_HEADER}\n\n{RESULT_FINDINGS_TABLE_OF_FINDINGS_HEADER}\n|#|Severity|Description|Status|\n| :---: | :------: | :----------: | :------------: |");
    let mut subfolder_findings_content: String =
        format!("\n{RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER}\n\n");
    let mut root_findings_content: String =
        format!("\n{RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER}\n\n");
    for (finding_file_index, finding_file) in findings_temp_files.into_iter().enumerate() {
        // for every finding file, replace the figures path
        let finding = Finding::new_from_path(&finding_file.path, finding_file_index);
        let table_of_contents_row = finding.parse_finding_table_row();

        subfolder_findings_content = format!(
            "{}\n{}\n---\n",
            subfolder_findings_content,
            finding
                .clone()
                .parse_finding_content_for_audit_folder_path()
        );
        root_findings_content = format!(
            "{}\n{}\n---\n",
            root_findings_content,
            finding.parse_finding_content_for_root_path()
        );
        table_of_findings = format!("{}\n{}", table_of_findings, table_of_contents_row);
    }
    // get content for root and sub folder
    let root_content = format!(
        "{}\n{}\n\n\n\n{HTML_TABLE_FORMAT}",
        table_of_findings, root_findings_content
    );
    let audit_folder_content = format!(
        "{}\n{}\n\n\n{HTML_TABLE_FORMAT}",
        table_of_findings, subfolder_findings_content
    );

    // write to root
    helpers::update_audit_result_root_content(&root_content)?;
    // write to audit_result folder
    fs::write(&findings_result_file_path, audit_folder_content).unwrap();
    // remove temp folder
    execute_command_to_stdio("rm", &["-rf", &audit_result_temp_path]).unwrap();
    let audit_result_file_path = get_file_path(FilePathType::AuditResult, true);
    vs_code_open_file_in_current_window(&findings_result_file_path)?;
    vs_code_open_file_in_current_window(&audit_result_file_path)?;

    let prompt_text = "Do you want to create the commit already?";
    let user_decided_to_create_commit = utils::cli_inputs::select_yes_or_no(prompt_text)?;
    if user_decided_to_create_commit {
        create_git_commit(GitCommit::AuditResult, None)?;
    }
    Ok(())
}

pub fn results_commit() -> Result<(), String> {
    create_git_commit(GitCommit::AuditResult, None)?;
    Ok(())
}

mod helpers {
    use crate::utils::helpers::get::get_string_between_two_str_from_path;

    use super::*;

    pub fn update_audit_result_root_content(root_content: &str) -> Result<(), String> {
        let audit_result_file_path = get_file_path(FilePathType::AuditResult, true);
        let audit_result_content = fs::read_to_string(&audit_result_file_path).unwrap();
        let findings_result_content = get_string_between_two_str_from_path(
            audit_result_file_path.clone(),
            RESULT_FINDINGS_SECTION_HEADER,
            RESULT_CODE_OVERHAUL_SECTION_HEADER,
        )?;
        let updated_content = audit_result_content.replace(&findings_result_content, root_content);
        fs::write(audit_result_file_path, updated_content).unwrap();
        Ok(())
    }
}

#[test]

fn test_format_header_with_finding_code_with_index_smaller_than_9() {
    let finding_content = "## Super bad finding \n rest of description";
    let finding_index = 2;
    let expected_content = "## KS-03 Super bad finding \n rest of description";
    let finding = Finding::new_from_str(finding_content, finding_index);
    assert_eq!(expected_content.to_string(), finding.content);
}

#[test]
fn test_format_header_with_finding_code_with_index_bigger_than_9() {
    let finding_content = "## Super bad finding \n rest of description";
    let finding_index = 10;
    let expected_content = "## KS-11 Super bad finding \n rest of description";
    let finding = Finding::new_from_str(finding_content, finding_index);
    assert_eq!(expected_content.to_string(), finding.content);
}

#[test]
fn test_parse_finding_data() {
    let finding_content = "## This is the description \n\n**Severity:** High\n\n**Status:** Open\n\n| Impact | Likelihood | Difficulty |\n| :----: | :--------: | :--------: |\n|  High  |    Medium    |    Low     |\n\n### Description {-}\n\n";
    let finding = Finding::new_from_str(finding_content, 0);
    assert_eq!(
        (
            finding.code,
            finding.title,
            finding.severity,
            finding.status,
            finding.impact.clone().unwrap(),
            finding.likelihood.clone().unwrap(),
            finding.difficulty.clone().unwrap(),
        ),
        (
            "KS-01".to_string(),
            "This is the description".to_string(),
            FindingLevel::High,
            StatusLevel::Open,
            FindingLevel::High,
            FindingLevel::Medium,
            FindingLevel::Low,
        )
    );
}

#[test]
fn test_parse_finding_table_row() {
    let finding_content =
        "## KS-01 This is the description \n\n**Severity:** High\n\n**Status:** Open";
    let finding = Finding::new_from_str(finding_content, 0);
    let finding_table_row = finding.parse_finding_table_row();
    assert_eq!(
        finding_table_row,
        "|KS-01|High|This is the description|Open|"
    );
}

#[test]
fn test_get_html_content() {
    let finding_content = "## This is the description \n\n**Severity:** High\n\n**Status:** Open\n\n| Impact | Likelihood | Difficulty |\n| :----: | :--------: | :--------: |\n|  High  |    Medium    |    Low     |\n\n### Description {-}\n\n";
    let finding = Finding::new_from_str(finding_content, 0);
    let finding_table_row = finding.parse_finding_table_html();
    println!("table {:#?}", finding_table_row);
    assert_eq!(
        finding_table_row,
        "|KS-01|High|This is the description|Open|"
    );
}
