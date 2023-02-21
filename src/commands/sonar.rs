use crate::batbelt;
use crate::batbelt::git::GitCommit;
use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::metadata::structs_metadata::StructMetadata;
use crate::batbelt::metadata::BatMetadataType;
use error_stack::{Result, ResultExt};

use crate::batbelt::sonar::{BatSonar, SonarResultType};

use super::CommandError;

pub fn start_sonar() -> Result<(), CommandError> {
    BatSonar::display_looking_for_loader(SonarResultType::Struct);
    structs()?;
    BatSonar::display_looking_for_loader(SonarResultType::Function);
    functions()?;
    Ok(())
}

fn functions() -> Result<(), CommandError> {
    let mut functions_metadata_markdown = BatMetadataType::Functions
        .get_markdown()
        .change_context(CommandError)?;
    let functions_metadata =
        FunctionMetadata::get_functions_metadata_from_program().change_context(CommandError)?;
    let functions_markdown_content = functions_metadata
        .into_iter()
        .map(|function_metadata| function_metadata.get_markdown_section_content_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    functions_metadata_markdown.content = functions_markdown_content;
    functions_metadata_markdown
        .save()
        .change_context(CommandError)?;
    batbelt::git::create_git_commit(
        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Functions,
        },
        None,
    )
    .unwrap();
    Ok(())
}

fn structs() -> Result<(), CommandError> {
    let mut structs_metadata_markdown = BatMetadataType::Structs
        .get_markdown()
        .change_context(CommandError)?;
    let structs_metadata =
        StructMetadata::get_structs_metadata_from_program().change_context(CommandError)?;
    let structs_markdown_content = structs_metadata
        .into_iter()
        .map(|struct_metadata| struct_metadata.get_markdown_section_content_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    structs_metadata_markdown.content = structs_markdown_content;
    structs_metadata_markdown
        .save()
        .change_context(CommandError)?;
    batbelt::git::create_git_commit(
        GitCommit::UpdateMetadata {
            metadata_type: BatMetadataType::Structs,
        },
        None,
    )
    .unwrap();
    Ok(())
}
