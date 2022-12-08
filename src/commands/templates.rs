use crate::{config::BatConfig};

pub fn update_templates() {
    // clone repository
    let BatConfig { required: _, .. } = BatConfig::get_validated_config();

    // clone_base_repository();

    // delete templates folder

    // move template to now location
}
