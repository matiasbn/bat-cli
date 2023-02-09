// Code overhaul template file
pub const CODE_OVERHAUL_WHAT_IT_DOES_PLACEHOLDER: &str = "WHAT_IT_DOES_HERE";
pub const CODE_OVERHAUL_NOTES_PLACEHOLDER: &str = "NOTES_HERE";
pub const CODE_OVERHAUL_MIRO_FRAME_LINK_PLACEHOLDER: &str = "MIRO_FRAME_LINK_PLACEHOLDER";
pub const CODE_OVERHAUL_ENTRYPOINT_PLACEHOLDER: &str = "ENTRYPOINT_PLACEHOLDER";
pub const CODE_OVERHAUL_CONTEXT_ACCOUNT_PLACEHOLDER: &str = "CONTEXT_ACCOUNT_PLACEHOLDER";
pub const CODE_OVERHAUL_VALIDATIONS_PLACEHOLDER: &str = "VALIDATIONS_PLACEHOLDER";
pub const CODE_OVERHAUL_HANDLER_PLACEHOLDER: &str = "HANDLER_PLACEHOLDER";
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

// Audit information file
pub const AUDIT_RESULT_FILE_NAME: &str = "audit_result.md";

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
miro_board_id = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor_name = ""
miro_oauth_access_token = ""
vs_code_integration = false
"#;

pub const ENTRYPOINT_PNG_NAME: &str = "entrypoint.png";
pub const HANDLER_PNG_NAME: &str = "handler.png";
pub const CONTEXT_ACCOUNTS_PNG_NAME: &str = "context_accounts.png";
pub const VALIDATIONS_PNG_NAME: &str = "validations.png";

pub static CO_FIGURES: &[&str] = &[
    ENTRYPOINT_PNG_NAME,
    HANDLER_PNG_NAME,
    CONTEXT_ACCOUNTS_PNG_NAME,
    VALIDATIONS_PNG_NAME,
];

// miro config

pub const MIRO_FRAME_WIDTH: u64 = 3392;
pub const MIRO_FRAME_HEIGHT: u64 = 1908;
pub const MIRO_BOARD_COLUMNS: i64 = 5;
pub const MIRO_INITIAL_X: i64 = 4800;
pub const MIRO_INITIAL_Y: i64 = 0;
pub const MIRO_INITIAL_X_ACCOUNTS_FRAME: i64 = -3197;
pub const MIRO_INITIAL_Y_ACCOUNTS_FRAME: i64 = 0;
pub const MIRO_INITIAL_X_ACCOUNTS_STICKY_NOTE: i64 = 372;
pub const MIRO_INITIAL_Y_ACCOUNTS_STICKY_NOTE: i64 = 268;
pub const MIRO_OFFSET_X_ACCOUNTS_STICKY_NOTE: i64 = 526;
pub const MIRO_OFFSET_Y_ACCOUNTS_STICKY_NOTE: i64 = 307;
pub const MIRO_WIDTH_ACCOUNTS_STICKY_NOTE: u64 = 374;
// pub const MIRO_HEIGHT_ACCOUNTS_STICKY_NOTE: u64 = 243;
pub const MIRO_WIDTH_ACCOUNTS_FRAME: u64 = 2500;
pub const MIRO_HEIGHT_ACCOUNTS_FRAME: u64 = 1320;
pub const MIRO_ACCOUNTS_STICKY_NOTE_COLUMNS: u64 = 4;
