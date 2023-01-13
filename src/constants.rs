// Code overhaul template file
pub const CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER: &str = "WHAT_IT_DOES_HERE";
pub const CODE_OVERHAUL_NOTES_PLACEHOLDER: &str = "NOTES_HERE";
pub const CODE_OVERHAUL_MIRO_BOARD_FRAME_PLACEHOLDER: &str = "MIRO_FRAME_LINK_HERE";
pub const CODE_OVERHAUL_SIGNERS_DESCRIPTION_PLACEHOLDER: &str = "SIGNERS_DESCRIPTION_PLACEHOLDER";
pub const CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER: &str = "ADD_A_DESCRIPTION_FOR_THIS_SIGNER";
pub const CODE_OVERHAUL_CONTEXT_ACCOUNTS_PLACEHOLDER: &str = "CONTEXT_ACCOUNTS_PLACEHOLDER";
pub const CODE_OVERHAUL_FUNCTION_PARAMETERS_PLACEHOLDER: &str = "FUNCTION_PARAMETER_PLACEHOLDER";
pub const CODE_OVERHAUL_NO_FUNCTION_PARAMETERS_FOUND_PLACEHOLDER: &str =
    "NO_FUNCTION_PARAMETERS_FOUND";
pub const CODE_OVERHAUL_ACCOUNTS_VALIDATION_PLACEHOLDER: &str = "ACCOUNTS_VALIDATIONS_PLACEHOLDER";
pub const CODE_OVERHAUL_PREREQUISITES_PLACEHOLDER: &str = "PREREQUISITES_PLACEHOLDER";
pub const CODE_OVERHAUL_NO_VALIDATION_FOUND_PLACEHOLDER: &str = "NO_VALIDATIONS_FOUND";

// Audit information file
pub const AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER: &str =
    "AUDIT_INFORMATION_PROJECT_NAME_PLACEHOLDER";
pub const AUDIT_INFORMATION_CLIENT_NAME_PLACEHOLDER: &str =
    "AUDIT_INFORMATION_CLIENT_NAME_PLACEHOLDER";
pub const AUDIT_INFORMATION_COMMIT_HASH_PLACEHOLDER: &str =
    "AUDIT_INFORMATION_COMMIT_HASH_PLACEHOLDER";
pub const AUDIT_INFORMATION_MIRO_BOARD_PLACEHOLER: &str = "AUDIT_INFORMATION_MIRO_BOARD_PLACEHOLER";
pub const AUDIT_INFORMATION_STARTING_DATE_PLACEHOLDER: &str =
    "AUDIT_INFORMATION_STARTING_DATE_PLACEHOLDER";

// Base repository
pub const BASE_REPOSTORY_URL: &str = "git@git.kudelski.com:TVRM/bat-base-repository.git";
pub const BASE_REPOSTORY_NAME: &str = "bat-base-repository";

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
auditor_names = [""]
project_name = ""
client_name = ""
commit_hash_url = ""
starting_date = ""
audit_folder_path = "."
program_lib_path = ""
project_repository_url = ""
miro_board_url = ""
miro_board_id = ""
[optional]
program_instructions_path = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name = ""
miro_oauth_access_token = ""
"#;

pub static CO_FIGURES: &'static [&str] = &[
    "entrypoint.png",
    "handler.png",
    "context_accounts.png",
    "validations.png",
];
