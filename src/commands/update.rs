use error_stack::ResultExt;

use crate::batbelt::path::BatFile::GitIgnore;
use crate::batbelt::templates::code_overhaul_template::CodeOverhaulTemplate;
use crate::batbelt::templates::package_json_template::PackageJsonTemplate;
use crate::batbelt::templates::TemplateGenerator;
use crate::batbelt::{git::GitCommit, path::BatFolder};
use crate::commands::{CommandError, CommandResult};

pub fn update_repository() -> CommandResult<()> {
    println!("Updating to-review files in code-overhaul folder");
    // move new templates to to-review in the auditor notes folder
    // let to_review_path = utils::path::get_auditor_code_overhaul_to_review_path(None)?;
    let to_review_file_names = BatFolder::CodeOverhaulToReview
        .get_all_bat_files(false, None, None)
        .change_context(CommandError)?;
    // if the auditor to-review code overhaul folder exists
    for bat_file in to_review_file_names {
        bat_file.remove_file().change_context(CommandError)?;
        let file_path = bat_file.get_path(false).change_context(CommandError)?;
        let co_template = CodeOverhaulTemplate::new(
            &bat_file.get_file_name().change_context(CommandError)?,
            false,
        )
        .change_context(CommandError)?;
        let mut co_markdown = co_template
            .to_markdown_file(&file_path)
            .change_context(CommandError)?;
        co_markdown.save().change_context(CommandError)?;
    }

    // replace package.json
    println!("Updating package.json");
    PackageJsonTemplate::update_package_json().change_context(CommandError)?;
    GitIgnore { for_init: false }
        .write_content(true, &TemplateGenerator::get_git_ignore_content())
        .change_context(CommandError)?;
    GitCommit::UpdateTemplates
        .create_commit()
        .change_context(CommandError)?;
    println!("Templates successfully updated");
    Ok(())
}
