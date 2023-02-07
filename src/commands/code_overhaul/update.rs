use colored::Colorize;
use dialoguer::console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use crate::batbelt::command_line::vs_code_open_file_in_current_window;

use crate::batbelt::constants::{
    CODE_OVERHAUL_INSTRUCTION_FILE_PATH_PLACEHOLDER, CODE_OVERHAUL_NOTES_PLACEHOLDER, CO_FIGURES,
    MIRO_BOARD_COLUMNS, MIRO_FRAME_HEIGHT, MIRO_FRAME_WIDTH, MIRO_INITIAL_X, MIRO_INITIAL_Y,
};
use crate::batbelt::miro::MiroConfig;
use crate::config::BatConfig;

use crate::batbelt;
use crate::batbelt::git::{check_correct_branch, create_git_commit, GitCommit};
use crate::batbelt::helpers::get::{
    get_finished_co_files, get_finished_co_files_info_for_results,
    get_table_of_contents_for_results,
};
use crate::batbelt::path::{FilePathType, FolderPathType};

use std::{env, fs};

use crate::batbelt::miro::frame::MiroFrame;

use crate::batbelt::markdown::MarkdownFile;
use crate::batbelt::metadata::structs::{StructMetadata, StructMetadataType};
use crate::batbelt::metadata::MetadataSection;
use crate::batbelt::sonar::{get_function_parameters, BatSonar, SonarResult, SonarResultType};
use crate::batbelt::templates::markdown::code_overhaul_template::CodeOverhaulSection;
use clap::builder::Str;
use std::path::Path;
use std::process::Command;
use std::string::String;

pub fn update_co_file() -> Result<(), String> {
    println!("Select the code-overhaul file to finish:");
    // let finished_path = utils::path::get_auditor_code_overhaul_finished_path(None)?;
    let finished_path = batbelt::path::get_folder_path(FolderPathType::CodeOverhaulFinished, true);
    // get to-review files
    let finished_files = fs::read_dir(finished_path)
        .unwrap()
        .map(|file| file.unwrap().file_name().to_str().unwrap().to_string())
        .filter(|file| file != ".gitkeep")
        .collect::<Vec<String>>();

    if finished_files.is_empty() {
        panic!("{}", "no finished files in code-overhaul folder".red());
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .items(&finished_files)
        .default(0)
        .with_prompt("Select the code-overhaul file to update:")
        .interact_on_opt(&Term::stderr())
        .unwrap();

    // user select file
    match selection {
        // move selected file to finished
        Some(index) => {
            let finished_file_name = finished_files[index].clone();
            check_correct_branch()?;
            create_git_commit(GitCommit::UpdateCO, Some(vec![finished_file_name]))?;
            Ok(())
        }
        None => panic!("User did not select anything"),
    }
}
