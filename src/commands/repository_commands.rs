use crate::batbelt;
use crate::batbelt::command_line::execute_command;

use crate::batbelt::git::GitCommit;

use crate::batbelt::bat_dialoguer::BatDialoguer;
use crate::batbelt::path::BatFolder;
use crate::batbelt::BatEnumerator;
use crate::commands::{BatCommandEnumerator, CommandError, CommandResult};
use clap::Subcommand;
use colored::Colorize;
use error_stack::{IntoReport, Report, Result, ResultExt};
use std::process::Command;

#[derive(
    Subcommand, Debug, strum_macros::Display, PartialEq, Clone, strum_macros::EnumIter, Default,
)]
pub enum RepositoryCommand {
    /// Merges all the branches into develop branch, and then merge develop into the rest of the branches
    #[default]
    UpdateBranches,
    /// Delete local branches
    DeleteLocalBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Fetch remote branches
    FetchRemoteBranches {
        /// select all options as true
        #[arg(short, long)]
        select_all: bool,
    },
    /// Commits the open_questions, finding_candidate and threat_modeling notes
    UpdateNotes,
    /// Creates a commit for an updated code-overhaul file
    UpdateCodeOverhaulFile,
    /// Creates a commit for the code_overhaul_summary.md file
    UpdateCodeOverhaulSummary,
}

impl BatEnumerator for RepositoryCommand {}

impl BatCommandEnumerator for RepositoryCommand {
    fn execute_command(&self) -> CommandResult<()> {
        self.check_develop_exists()?;
        match self {
            RepositoryCommand::UpdateBranches => {
                self.merge_all_to_develop()?;
                self.merge_develop_to_all()
            }
            RepositoryCommand::FetchRemoteBranches { select_all } => {
                self.fetch_remote_branches(*select_all)
            }
            RepositoryCommand::DeleteLocalBranches { select_all } => {
                self.delete_local_branches(*select_all)
            }
            RepositoryCommand::UpdateNotes => GitCommit::Notes
                .create_commit()
                .change_context(CommandError),
            RepositoryCommand::UpdateCodeOverhaulFile => self.execute_update_co_file(),
            RepositoryCommand::UpdateCodeOverhaulSummary => self.update_code_overhaul_summary(),
        }
    }

    fn check_metadata_is_initialized(&self) -> bool {
        false
    }

    fn check_correct_branch(&self) -> bool {
        match self {
            RepositoryCommand::UpdateNotes => true,
            _ => false,
        }
    }
}

impl RepositoryCommand {
    fn update_code_overhaul_summary(&self) -> CommandResult<()> {
        GitCommit::UpdateCOSummary
            .create_commit()
            .change_context(CommandError)?;
        println!("Commit created for code_overhaul_summary.md file");
        Ok(())
    }

    fn merge_all_to_develop(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        self.checkout_branch("develop")?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch '{}' into develop", branch_name);
            execute_command("git", &["merge", &branch_name, "-m", &message], false)
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn merge_develop_to_all(&self) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        for branch_name in branches_list {
            log::debug!("branch_name: {}", branch_name);
            let message = format!("Merge branch develop into '{}'", branch_name);
            log::debug!("Merge message: {}", message);
            self.checkout_branch(&branch_name)?;
            println!("Merging develop into {}", branch_name.green());
            Command::new("git")
                .args(["merge", "develop", "-m", &message])
                .output()
                .into_report()
                .change_context(CommandError)?;
            // execute_command("git", &["merge", "develop", "-m", &message])
            //     .change_context(CommandError)?;
        }
        self.checkout_branch("develop")?;
        Ok(())
    }

    fn fetch_remote_branches(&self, select_all: bool) -> Result<(), CommandError> {
        let branches_list = self.get_remote_branches_filtered()?;
        let prompt_test = format!("Select the branches {}", "to fetch".green());
        let selections = batbelt::bat_dialoguer::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Fetching {}", selected_branch.green());
            log::debug!("selected_branch to fetch: {}", selected_branch);
            execute_command(
                "git",
                &["checkout", selected_branch.trim_start_matches("origin/")],
                false,
            )
            .change_context(CommandError)?;
        }
        self.checkout_branch("develop")?;
        Ok(())
    }

    fn delete_local_branches(&self, select_all: bool) -> Result<(), CommandError> {
        let branches_list = self.get_local_branches_filtered()?;
        self.checkout_branch("develop")?;
        let prompt_test = format!("Select the branches {}", "to delete".red());
        let selections = batbelt::bat_dialoguer::multiselect(
            &prompt_test,
            branches_list.clone(),
            Some(&vec![select_all; branches_list.len()]),
        )
        .change_context(CommandError)?;
        for selection in selections {
            let selected_branch = &branches_list.clone()[selection];
            println!("Deleting {}", selected_branch.green());
            log::debug!("selected_branch to delete: {}", selected_branch);
            execute_command("git", &["branch", "-D", selected_branch], false)
                .change_context(CommandError)?;
        }
        Ok(())
    }

    fn check_develop_exists(&self) -> Result<(), CommandError> {
        let branches_list = batbelt::git::get_local_branches().change_context(CommandError)?;
        if !branches_list
            .lines()
            .any(|line| line.trim_start_matches('*').trim_start() == "develop")
        {
            log::debug!("branches_list:\n{}", branches_list);
            return Err(Report::new(CommandError).attach_printable("develop branch not found"));
        }
        Ok(())
    }

    fn get_local_branches_filtered(&self) -> Result<Vec<String>, CommandError> {
        let branches_list = batbelt::git::get_local_branches().change_context(CommandError)?;
        log::debug!("local_branches from batbelt::git: \n{}", branches_list);
        let list = branches_list
            .lines()
            .filter_map(|branch| {
                let branch_name = branch.trim().trim_start_matches('*').trim();
                if branch_name != "main" && branch_name != "develop" {
                    Some(branch_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        log::debug!("filtered branches_list: \n{:#?}", list);
        Ok(list)
    }

    fn get_remote_branches_filtered(&self) -> Result<Vec<String>, CommandError> {
        let branches_list = batbelt::git::get_remote_branches().change_context(CommandError)?;
        log::debug!("remote_branches from batbelt::git: \n{}", branches_list);
        let list = branches_list
            .lines()
            .filter_map(|branch| {
                let branch_name = branch.trim().trim_start_matches('*').trim();
                if branch_name != "origin/main"
                    && branch_name != "origin/develop"
                    && branch_name.split(" ->").next().unwrap() != "origin/HEAD"
                {
                    Some(branch_name.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        log::debug!("filtered remote_branches: \n{:#?}", list);
        Ok(list)
    }

    fn execute_update_co_file(&self) -> CommandResult<()> {
        println!("Select the code-overhaul file to finish:");
        let finished_files_names = BatFolder::CodeOverhaulFinished
            .get_all_files_names(true, None, None)
            .change_context(CommandError)?;

        if finished_files_names.is_empty() {
            return Err(Report::new(CommandError).attach_printable(format!(
                "{}",
                "no finished files in code-overhaul folder".red()
            )));
        }

        let selection = BatDialoguer::select(
            "Select the code-overhaul file to update:".to_string(),
            finished_files_names.clone(),
            None,
        )
        .change_context(CommandError)?;

        let finished_file_name = finished_files_names[selection].clone();

        GitCommit::UpdateCO {
            entrypoint_name: finished_file_name,
        }
        .create_commit()
        .change_context(CommandError)?;
        Ok(())
    }

    fn checkout_branch(&self, branch_name: &str) -> Result<(), CommandError> {
        execute_command("git", &["checkout", branch_name], false).change_context(CommandError)?;
        Ok(())
    }
}

#[test]
fn test_get_remote_branches_filtered() {
    let remote_branches =
        RepositoryCommand::get_remote_branches_filtered(&RepositoryCommand::UpdateBranches)
            .unwrap();
    println!("remote_branches:\n{:#?}", remote_branches)
}

#[test]
fn test_get_local_branches_filtered() {
    let local_branches =
        RepositoryCommand::get_local_branches_filtered(&RepositoryCommand::UpdateBranches).unwrap();
    println!("local_branches:\n{:#?}", local_branches)
}

// use std::fs;
//
// use crate::batbelt::command_line::vs_code_open_file_in_current_window;
// use crate::{
//     batbelt::{
//         self,
//         bash::execute_command,
//         git::{create_git_commit, GitCommit},
//         helpers::get::{
//             get_only_files_from_folder, get_string_between_two_index_from_string,
//             get_string_between_two_str_from_string,
//         },
//         path::{get_file_path, get_folder_path, FilePathType, FolderPathType},
//     },
//     config::BatConfig,
// };
//
// pub const FINDING_CODE_PREFIX: &str = "KS";
// pub const RESULT_FINDINGS_SECTION_HEADER: &str = "# Findings result";
// pub const RESULT_FINDINGS_TABLE_OF_FINDINGS_HEADER: &str = "## Table of findings";
// pub const RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER: &str = "## List of Findings";
// pub const RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER_ROBOT: &str = "# Findings";
// pub const RESULT_CODE_OVERHAUL_SECTION_HEADER: &str = "# Code overhaul result";
//
// pub const HTML_TABLE_STYLE: &str = "<style>
//
//
// tr th {
//     background: #043456;
//     color:white;
//     width: 2%;
//     text-align: center;
//     border: 2px solid black;
// }
//
// .alg tr {
//     width: 2%;
//     text-align: center;
//     border: 2px solid black;
// }
// .alg thead tr th:nth-of-type(2) {
//     width: 9%;
//     text-align: center;
//     border: 2px solid black;
// }
//
// tr td {
//     background: white;
//     width: 2%;
//     text-align: center;
//     border: 2px solid black;
// }
// .high {
//     background: #fd0011;
//     border: 2px solid yellow;
//     text-align: center;
//     color: white;
// }
// .medium {
//     background: #f58b45;
//     border: 2px solid yellow;
//     text-align: center;
//     color: white;
// }
// .low {
//     background: #16a54d;
//     border: 20px solid yellow;
//     text-align: center;
//     color: white;
// }
// .informational {
//     background: #0666b4;
//     border: 2px solid yellow;
//     text-align: center;
//     color: white;
// }
// .open {
//     background: #16a54d;
//     border: 2px solid yellow;
//     text-align: center;
//     color: white;
// }
//
// .list th{
//     background: #043456;
//     color: white
// }
// .list td{
//     color: black
// }
//
// </style>";
//
// pub const HTML_LIST_OF_FINDINGS_HEADER: &str = "<table class='list'>
// <thead>
//     <tr>
//         <th style='width:2%'>#</th>  <th>Severity</th>  <th style='width:10%'>Description</th>  <th>Status</th>
//     </tr>
// </thead>
// <tbody>
//     RESULT_TABLE_PLACEHOLDER
// </tbody>
// </table>\n";
//
// pub const RESULT_TABLE_PLACEHOLDER: &str = "RESULT_TABLE_PLACEHOLDER";
//
// #[derive(PartialEq, Debug, Clone)]
// enum StatusLevel {
//     Open,
// }
//
// impl StatusLevel {
//     pub fn from_str(status_str: &str) -> Self {
//         let severity = status_str.to_lowercase();
//         let severity_binding = severity.as_str();
//         match severity_binding {
//             "open" => StatusLevel::Open,
//             _ => panic!("incorrect status level {}", severity_binding),
//         }
//     }
//     pub fn to_string(&self) -> String {
//         match self {
//             StatusLevel::Open => "Open".to_string(),
//         }
//     }
//
//     pub fn get_hex_color(&self) -> String {
//         match self {
//             Self::Open => "#16a54d".to_string(),
//         }
//     }
// }
//
// #[derive(PartialEq, Debug, Clone)]
// enum FindingLevel {
//     High,
//     Medium,
//     Low,
//     Informational,
// }
//
// impl FindingLevel {
//     pub fn from_str(severity_str: &str) -> Self {
//         let severity = severity_str.to_lowercase();
//         let severity_binding = severity.as_str();
//         match severity_binding {
//             "high" => FindingLevel::High,
//             "medium" => FindingLevel::Medium,
//             "low" => FindingLevel::Low,
//             "informational" => FindingLevel::Informational,
//             _ => panic!("incorrect severity level {}", severity_binding),
//         }
//     }
//
//     pub fn to_string(&self) -> String {
//         match self {
//             FindingLevel::High => "High".to_string(),
//             FindingLevel::Medium => "Medium".to_string(),
//             FindingLevel::Low => "Low".to_string(),
//             FindingLevel::Informational => "Informational".to_string(),
//         }
//     }
//
//     pub fn get_hex_color(&self) -> String {
//         match self {
//             FindingLevel::High => "#fd0011".to_string(),
//             FindingLevel::Medium => "#f58b45".to_string(),
//             FindingLevel::Low => "#16a54d".to_string(),
//             FindingLevel::Informational => "#0666b4".to_string(),
//         }
//     }
// }
//
// #[derive(PartialEq, Debug, Clone)]
// pub struct Finding {
//     code: String,
//     title: String,
//     severity: FindingLevel,
//     impact: Option<FindingLevel>,
//     likelihood: Option<FindingLevel>,
//     difficulty: Option<FindingLevel>,
//     status: StatusLevel,
//     content: String,
//     index: usize,
// }
//
// impl Finding {
//     pub fn new_from_path(finding_path: &str, index: usize) -> Self {
//         let finding_content = fs::read_to_string(finding_path).unwrap();
//         Self::new_from_str(&finding_content, index)
//     }
//
//     pub fn new_from_str(finding_content: &str, index: usize) -> Self {
//         let content = Self::format_finding_content_header_with_finding_code(finding_content, index);
//         let (code, title, severity_str, status, impact, likelihood, difficulty) =
//             Self::parse_finding_data(&content);
//         let severity = FindingLevel::from_str(&severity_str);
//         Finding {
//             code,
//             title,
//             severity,
//             status,
//             content,
//             index,
//             impact,
//             likelihood,
//             difficulty,
//         }
//     }
//
//     pub fn format_markdown_to_html(&mut self) {
//         let severity_index = self
//             .content
//             .lines()
//             .position(|line| line.contains("**Severity:**"))
//             .unwrap();
//         let description_index = self
//             .content
//             .lines()
//             .position(|line| line.contains("### Description"))
//             .unwrap();
//         let data_content = get_string_between_two_index_from_string(
//             self.content.clone(),
//             severity_index,
//             description_index - 1,
//         )
//         .unwrap();
//         let html_content = self.parse_finding_table_html();
//         self.content = self.content.replace(&data_content, &html_content);
//     }
//
//     fn parse_finding_data(
//         finding_content: &str,
//     ) -> (
//         String,
//         String,
//         String,
//         StatusLevel,
//         Option<FindingLevel>,
//         Option<FindingLevel>,
//         Option<FindingLevel>,
//     ) {
//         let finding_content_lines = finding_content.lines();
//         let finding_content_first_line = finding_content_lines.clone().next().unwrap();
//
//         let finding_code = finding_content_first_line
//             .clone()
//             .strip_prefix(&format!("## "))
//             .unwrap()
//             .split(" ")
//             .next()
//             .unwrap()
//             .replace(":", "");
//         let finding_description = finding_content_first_line
//             .strip_prefix(&format!("## {finding_code}: "))
//             .unwrap()
//             .trim();
//         let finding_severity = finding_content_lines
//             .clone()
//             .find(|line| line.contains("**Severity:**"))
//             .unwrap()
//             .strip_prefix("**Severity:** ")
//             .unwrap();
//
//         let finding_status = finding_content_lines
//             .clone()
//             .find(|line| line.contains("**Status:**"))
//             .unwrap()
//             .strip_prefix("**Status:** ")
//             .unwrap();
//         let finding_status = StatusLevel::from_str(finding_status);
//         if FindingLevel::from_str(finding_severity) == FindingLevel::Informational {
//             return (
//                 finding_code.to_string(),
//                 finding_description.to_string(),
//                 finding_severity.to_string(),
//                 finding_status,
//                 None,
//                 None,
//                 None,
//             );
//         }
//         let finding_table = get_string_between_two_str_from_string(
//             finding_content.to_string(),
//             "**Status:**",
//             "### Description",
//         )
//         .unwrap();
//         let severities = ["High", "Medium", "Low"];
//         let status = finding_table
//             .lines()
//             .find(|line| severities.iter().any(|severity| line.contains(severity)))
//             .unwrap();
//         let status_splited: Vec<&str> = status
//             .split("|")
//             .map(|spl| spl.trim())
//             .filter(|spl| severities.iter().any(|severity| spl.contains(severity)))
//             .collect();
//         let impact = FindingLevel::from_str(&status_splited[0]);
//         let likelihood = FindingLevel::from_str(&status_splited[1]);
//         let difficulty = FindingLevel::from_str(&status_splited[2]);
//         (
//             finding_code.to_string(),
//             finding_description.to_string(),
//             finding_severity.to_string(),
//             finding_status,
//             Some(impact),
//             Some(likelihood),
//             Some(difficulty),
//         )
//     }
//
//     pub fn format_finding_content_header_with_finding_code(
//         finding_content: &str,
//         index: usize,
//     ) -> String {
//         let mut finding_content_lines = finding_content.lines();
//         let content_first_line = finding_content_lines.next().unwrap();
//         let text_to_replace = format!(
//             "## {}-{}:",
//             FINDING_CODE_PREFIX,
//             if index < 9 {
//                 format!("0{}", index + 1)
//             } else {
//                 format!("{}", index + 1)
//             }
//         );
//         let formatted_header = content_first_line.replace("##", &text_to_replace);
//         let formatted_finding_content =
//             finding_content.replace(content_first_line, &formatted_header);
//         formatted_finding_content
//     }
//
//     pub fn parse_table_of_findings_table_row(&self) -> String {
//         format!(
//             "|{}|{:#?}|{}|{:#?}|",
//             self.code, self.severity, self.title, self.status
//         )
//     }
//
//     pub fn parse_list_of_findings_table_row_html(&self) -> String {
//         // <th>#</th>  <th>Severity</th>  <th>Description</th>  <th>Status</th>
//         let severity = format!(
//             "<span style='color:{};'>{:#?}</span>",
//             self.severity.get_hex_color(),
//             self.severity
//         );
//         let status = format!(
//             "<span style='color:{};'>{:#?}</span>",
//             self.status.get_hex_color(),
//             self.status
//         );
//         format!(
//             "<tr><td>{}</td>  <td>{}</td>  <td>{}</td>  <td>{}</td></tr>",
//             self.code, severity, self.title, status
//         )
//     }
//
//     pub fn parse_finding_content_for_audit_folder_path(&self) -> String {
//         self.content.replace("../../figures", "./figures")
//     }
//
//     pub fn parse_finding_content_for_root_path(&self) -> String {
//         let audit_result_figures_path = get_folder_path(FolderPathType::AuditResultFigures, false);
//         self.content
//             .replace("../../figures", &audit_result_figures_path)
//     }
//
//     pub fn parse_finding_table_html(&self) -> String {
//         let Finding {
//             severity,
//             impact,
//             likelihood,
//             difficulty,
//             status,
//             ..
//         } = self;
//         if severity.clone() == FindingLevel::Informational {
//             format!("<div style='width:50%; margin: auto'>
//             <table class='alg'>
//                 <thead>
//                 <tr>
//                     <th style='font-weight:bold'>Severity</th>    <th class='informational'>Informational</th>
//                 </thead>
//                 </tr>
//                 <tbody>
//                 <tr>
//                     <td style='background: #043456; color: white; font-weight:bold'>Status</td>    <td class='{}'>{}</td>
//                 </tr>
//                 </tbody>
//             </table>
//         </div>\n",status.to_string().to_lowercase(), status.to_string()
//             )
//         } else {
//             let difficulty_style = match difficulty.clone().unwrap() {
//                 FindingLevel::Low => "high",
//                 FindingLevel::Medium => "medium",
//                 FindingLevel::High => "low",
//                 _ => unimplemented!(),
//             };
//
//             format!(
//                 "
// <div style='width:50%; margin: auto'>
//     <table class='alg'>
//         <thead>
//         <tr>
//             <th style='font-weight:bold'>Severity</th>    <th class='{}'>{}</th>
//         </thead>
//         </tr>
//         <tbody>
//         <tr>
//             <td style='background: #043456; color: white; font-weight:bold'>Status</td>    <td class='{}'>{}</td>
//         </tr>
//         </tbody>
//     </table>
// </div>
// <table>
//     <thead>
//     <tr>
//         <th>Impact</th>    <th>Likelihood</th>    <th>Difficulty</th>
//     </tr>
//     </thead>
//     <tbody>
//     <tr>
//         <td class='{}'>{}</td>    <td class='{}'>{}</td>    <td class='{}'>{}</td>
//     </tr>
//     </tbody>
// </table>\n",
//                 severity.to_string().to_lowercase(),
//                 severity.to_string(),
//                 status.to_string().to_lowercase(),
//                 status.to_string(),
//                 impact.clone().unwrap().to_string().to_lowercase(),
//                 impact.clone().unwrap().to_string(),
//                 likelihood.clone().unwrap().to_string().to_lowercase(),
//                 likelihood.clone().unwrap().to_string(),
//                 difficulty_style,
//                 difficulty.clone().unwrap().to_string(),
//             )
//         }
//     }
// }
// pub fn findings_result(generate_html: bool) -> Result<(), String> {
//     // get the audit_result path
//     let audit_result_temp_path =
//         batbelt::path::get_folder_path(FolderPathType::AuditResultTemp, false);
//     let audit_result_figures_path =
//         batbelt::path::get_folder_path(FolderPathType::AuditResultFigures, true);
//     let notes_folder = batbelt::path::get_folder_path(FolderPathType::Notes, true);
//
//     // create a temp folder for the findings
//     batbelt::bash::execute_command("mkdir", &[&audit_result_temp_path]).unwrap();
//     // delete figures folder
//     batbelt::bash::execute_command("rm", &["-rf", &audit_result_figures_path]).unwrap();
//     // create figures folder
//     batbelt::bash::execute_command("mkdir", &[&audit_result_figures_path]).unwrap();
//
//     // copy all the data to the audit_result folder
//     let auditor_names = BatConfig::get_validated_config()?.required.auditor_names;
//     for auditor in auditor_names {
//         let auditor_notes_path = format!("{}/{}-notes", notes_folder, auditor);
//         let auditor_accepted_findings_path = format!("{}/findings/accepted", auditor_notes_path);
//
//         let findings_files = get_only_files_from_folder(auditor_accepted_findings_path)?;
//         // for each auditor, copy all the findings to the temp folder
//         for finding_file in findings_files {
//             batbelt::bash::execute_command("cp", &[&finding_file.path, &audit_result_temp_path])
//                 .unwrap();
//         }
//
//         // for each auditor, copy all the figures to the audit_result figures folder
//         let auditor_figures_path = format!("{}/figures", auditor_notes_path);
//         let figures_files = get_only_files_from_folder(auditor_figures_path)?;
//         for figure_file in figures_files {
//             batbelt::bash::execute_command("cp", &[&figure_file.path, &audit_result_figures_path])
//                 .unwrap();
//         }
//     }
//     let findings_result_file_path =
//         get_file_path(batbelt::path::FilePathType::FindingsResult, true);
//     // remove previous findings_result.md file
//     execute_command("rm", &[&findings_result_file_path]).unwrap();
//     // create new findings_result.md file
//     execute_command("touch", &[&findings_result_file_path]).unwrap();
//     // create new findings_result.md file
//     let findings_temp_files = get_only_files_from_folder(audit_result_temp_path.clone())?;
//     let mut table_of_findings: String = if generate_html {
//         format!("{RESULT_FINDINGS_SECTION_HEADER}\n\n{RESULT_FINDINGS_TABLE_OF_FINDINGS_HEADER}\n{HTML_LIST_OF_FINDINGS_HEADER}\n")
//     } else {
//         format!("{RESULT_FINDINGS_SECTION_HEADER}\n\n{RESULT_FINDINGS_TABLE_OF_FINDINGS_HEADER}\n|#|Severity|Description|Status|\n| :---: | :------: | :----------: | :------------: |")
//     };
//     let mut subfolder_findings_content: String =
//         format!("\n{RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER}\n\n");
//     let mut robot_content: String = format!("{RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER_ROBOT}\n");
//     let mut root_findings_content: String =
//         format!("\n{RESULT_FINDINGS_LIST_OF_FINDINGS_HEADER}\n\n");
//     let mut html_rows: Vec<String> = vec![];
//     for (finding_file_index, finding_file) in findings_temp_files.into_iter().enumerate() {
//         // for every finding file, replace the figures path
//         let mut finding = Finding::new_from_path(&finding_file.path, finding_file_index);
//         if generate_html {
//             finding.format_markdown_to_html();
//         }
//         html_rows.push(finding.parse_list_of_findings_table_row_html());
//
//         subfolder_findings_content = format!(
//             "{}\n{}\n---\n",
//             subfolder_findings_content,
//             finding
//                 .clone()
//                 .parse_finding_content_for_audit_folder_path()
//         );
//         robot_content = format!(
//             "{}\n{}\n---\n",
//             robot_content,
//             finding
//                 .clone()
//                 .parse_finding_content_for_audit_folder_path()
//         );
//         root_findings_content = format!(
//             "{}\n{}\n---\n",
//             root_findings_content,
//             finding.parse_finding_content_for_root_path()
//         );
//         if !generate_html {
//             let table_of_contents_row = finding.parse_table_of_findings_table_row();
//             table_of_findings = format!("{}\n{}", table_of_findings, table_of_contents_row);
//         }
//     }
//     if generate_html {
//         table_of_findings =
//             table_of_findings.replace(RESULT_TABLE_PLACEHOLDER, &html_rows.join("\n"))
//     }
//     // get content for root and sub folder
//     let root_content = if generate_html {
//         format!(
//             "{}\n{}\n\n\n\n{HTML_TABLE_STYLE}",
//             table_of_findings, root_findings_content
//         )
//     } else {
//         format!("{}\n{}", table_of_findings, root_findings_content)
//     };
//     let audit_folder_content = if generate_html {
//         format!(
//             "{}\n{}\n\n\n\n{HTML_TABLE_STYLE}",
//             table_of_findings, subfolder_findings_content
//         )
//     } else {
//         format!("{}\n{}", table_of_findings, subfolder_findings_content)
//     };
//
//     let robot_findings_path = get_file_path(FilePathType::FindingsRobotResult, false);
//     fs::write(&robot_findings_path, robot_content).unwrap();
//
//     // write to root
//     helpers::update_audit_result_root_content(&root_content)?;
//     // write to audit_result folder
//     fs::write(&findings_result_file_path, audit_folder_content).unwrap();
//     // remove temp folder
//     execute_command("rm", &["-rf", &audit_result_temp_path]).unwrap();
//     let audit_result_file_path = get_file_path(FilePathType::AuditResult, true);
//     vs_code_open_file_in_current_window(&findings_result_file_path)?;
//     vs_code_open_file_in_current_window(&audit_result_file_path)?;
//
//     let prompt_text = "Do you want to create the commit already?";
//     let user_decided_to_create_commit = batbelt::cli_inputs::select_yes_or_no(prompt_text)?;
//     if user_decided_to_create_commit {
//         create_git_commit(GitCommit::AuditResult, None)?;
//     }
//     Ok(())
// }
//
// pub fn results_commit() -> Result<(), String> {
//     create_git_commit(GitCommit::AuditResult, None)?;
//     Ok(())
// }
//
// mod helpers {
//     use crate::batbelt::helpers::get::get_string_between_two_str_from_path;
//
//     use super::*;
//
//     pub fn update_audit_result_root_content(root_content: &str) -> Result<(), String> {
//         let audit_result_file_path = get_file_path(FilePathType::AuditResult, true);
//         let audit_result_content = fs::read_to_string(&audit_result_file_path).unwrap();
//         let findings_result_content = get_string_between_two_str_from_path(
//             audit_result_file_path.clone(),
//             RESULT_FINDINGS_SECTION_HEADER,
//             RESULT_CODE_OVERHAUL_SECTION_HEADER,
//         )?;
//         let updated_content = audit_result_content.replace(&findings_result_content, root_content);
//         fs::write(audit_result_file_path, updated_content).unwrap();
//         Ok(())
//     }
// }
//
// #[test]
//
// fn test_format_header_with_finding_code_with_index_smaller_than_9() {
//     let finding_content = "## Super bad finding \n rest of description";
//     let finding_index = 2;
//     let expected_content = "## KS-03 Super bad finding \n rest of description";
//     let finding = Finding::new_from_str(finding_content, finding_index);
//     assert_eq!(expected_content.to_string(), finding.content);
// }
//
// #[test]
// fn test_format_header_with_finding_code_with_index_bigger_than_9() {
//     let finding_content = "## Super bad finding \n rest of description";
//     let finding_index = 10;
//     let expected_content = "## KS-11 Super bad finding \n rest of description";
//     let finding = Finding::new_from_str(finding_content, finding_index);
//     assert_eq!(expected_content.to_string(), finding.content);
// }
//
// #[test]
// fn test_parse_finding_data() {
//     let finding_content = "## This is the description \n\n**Severity:** High\n\n**Status:** Open\n\n| Impact | Likelihood | Difficulty |\n| :----: | :--------: | :--------: |\n|  High  |    Medium    |    Low     |\n\n### Description {-}\n\n";
//     let finding = Finding::new_from_str(finding_content, 0);
//     assert_eq!(
//         (
//             finding.code,
//             finding.title,
//             finding.severity,
//             finding.status,
//             finding.impact.clone().unwrap(),
//             finding.likelihood.clone().unwrap(),
//             finding.difficulty.clone().unwrap(),
//         ),
//         (
//             "KS-01".to_string(),
//             "This is the description".to_string(),
//             FindingLevel::High,
//             StatusLevel::Open,
//             FindingLevel::High,
//             FindingLevel::Medium,
//             FindingLevel::Low,
//         )
//     );
// }
//
// #[test]
// fn test_parse_finding_table_row() {
//     let finding_content =
//         "## KS-01 This is the description \n\n**Severity:** High\n\n**Status:** Open";
//     let finding = Finding::new_from_str(finding_content, 0);
//     let finding_table_row = finding.parse_table_of_findings_table_row();
//     assert_eq!(
//         finding_table_row,
//         "|KS-01|High|This is the description|Open|"
//     );
// }
//
// #[test]
// fn test_get_html_content() {
//     let finding_content = "## This is the description \n\n**Severity:** High\n\n**Status:** Open\n\n| Impact | Likelihood | Difficulty |\n| :----: | :--------: | :--------: |\n|  High  |    Medium    |    Low     |\n\n### Description {-}\n\n";
//     let finding = Finding::new_from_str(finding_content, 0);
//     let finding_table_row = finding.parse_finding_table_html();
//     println!("table {:#?}", finding_table_row);
// }
//
// #[test]
// fn test_update_content() {
//     let finding_content = "## This is the description \n\n**Severity:** High\n\n**Status:** Open\n\n| Impact | Likelihood | Difficulty |\n| :----: | :--------: | :--------: |\n|  High  |    Medium    |    Low     |\n\n### Description {-}\n\n";
//     let mut finding = Finding::new_from_str(finding_content, 0);
//     finding.format_markdown_to_html();
//     println!("table {}", finding.content);
//     // assert_eq!(
//     //     finding_table_row,
//     //     "|KS-01|High|This is the description|Open|"
//     // );
// }
