// Code overhaul template file
pub const CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER: &str = "WHAT_IT_DOES_HERE";
pub const CODE_OVERHAUL_NOTES_PLACEHOLDER: &str = "NOTES_HERE";
pub const CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER: &str = "MIRO_FRAME_LINK_PLACEHOLDER";
pub const CODE_OVERHAUL_EMPTY_SIGNER_PLACEHOLDER: &str = "ADD_A_DESCRIPTION_FOR_THIS_SIGNER";
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
pub const BASE_REPOSTORY_URL: &str = "git@github.com:matiasbn/bat-base-repository.git";
pub const BASE_REPOSTORY_NAME: &str = "bat-base-repository";

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
auditor_names = [""]
project_name = ""
client_name = ""
commit_hash_url = ""
starting_date = ""
program_lib_path = ""
project_repository_url = ""
miro_board_url = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name = ""
miro_oauth_access_token = ""
vs_code_integration = false
"#;

// miro config

pub const MIRO_FRAME_WIDTH: u64 = 3392;
pub const MIRO_FRAME_HEIGHT: u64 = 1908;
pub const MIRO_BOARD_COLUMNS: i64 = 5;
pub const MIRO_INITIAL_X: i64 = 4800;
pub const MIRO_INITIAL_Y: i64 = 0;
