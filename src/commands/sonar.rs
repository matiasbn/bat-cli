use crate::batbelt::sonar::{BatSonar, SonarResultType};
use crate::commands;

pub fn start_sonar() {
    BatSonar::display_looking_for_loader(SonarResultType::Struct);
    commands::metadata::structs().unwrap();
    BatSonar::display_looking_for_loader(SonarResultType::Function);
    commands::metadata::functions().unwrap();
}
