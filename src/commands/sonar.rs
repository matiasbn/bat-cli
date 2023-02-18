use error_stack::Result;

use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::commands;

use super::CommandError;

pub fn start_sonar() -> Result<(), CommandError> {
    BatSonar::display_looking_for_loader(SonarResultType::Struct);
    commands::metadata::structs()?;
    BatSonar::display_looking_for_loader(SonarResultType::Function);
    commands::metadata::functions()?;
    Ok(())
}
