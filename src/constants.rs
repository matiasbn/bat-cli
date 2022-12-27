pub const CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER: &str = "BRIEF_DESCRIPTION_OF_THE_FUNCTIONALITY";
pub const CODE_OVERHAUL_NOTES_PLACEHOLDER: &str = "SOME_NOTES_ABOUT_THIS_ENTRYPOINT";
pub const CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER: &str = "MIRO_FRAME_LINK_HERE";
pub const CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER: &str = "CONTEXT_ACCOUNTS_PLACEHOLDER";
pub const CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER: &str = "FUNCTION_PARAMETER_PLACEHOLDER";
pub const CODE_OVERHAUL_VALIDATION_PLACEHOLDER: &str = "VALIDATIONS_PLACEHOLDER";
pub const BASE_REPOSTORY_URL: &str = "git@git.kudelski.com:TVRM/bat-base-repository.git";
pub const BASE_REPOSTORY_NAME: &str = "bat-base-repository";

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
project_name = ""
auditor_names = [""]
audit_folder_path = "."
program_lib_path = ""
project_repository_url = ""
[optional]
program_instructions_path = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name = ""
"#;
