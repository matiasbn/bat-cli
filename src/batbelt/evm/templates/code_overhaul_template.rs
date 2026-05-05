/// Code-overhaul template for Solidity entry points.
/// Adapted from the Solana template to reflect EVM-specific concepts.
pub const SOLIDITY_CO_TEMPLATE: &str = r#"# State changes:
# Notes:
# Access control:
# Parameters:
# Storage layout:
# External calls:
# Events emitted:
# Validations:
# Miro frame url:
"#;

/// Generate a populated code-overhaul file for a Solidity entry point.
pub fn generate_co_content(
    entry_point_name: &str,
    access_control: &[String],
    parameters: &[String],
    storage_vars: &[String],
    external_calls: &[String],
    events: &[String],
    validations: &[String],
) -> String {
    let mut content = String::new();

    content.push_str("# State changes:\n");
    content.push_str("# Notes:\n");

    content.push_str("# Access control:\n");
    for ac in access_control {
        content.push_str(&format!("- {}\n", ac));
    }

    content.push_str("# Parameters:\n");
    for param in parameters {
        content.push_str(&format!("- {}\n", param));
    }

    content.push_str("# Storage layout:\n");
    for var in storage_vars {
        content.push_str(&format!("- {}\n", var));
    }

    content.push_str("# External calls:\n");
    for call in external_calls {
        content.push_str(&format!("- {}\n", call));
    }

    content.push_str("# Events emitted:\n");
    for event in events {
        content.push_str(&format!("- {}\n", event));
    }

    content.push_str("# Validations:\n");
    for val in validations {
        content.push_str(&format!("- {}\n", val));
    }

    content.push_str("# Miro frame url:\n");

    content
}
