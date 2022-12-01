use std::fs;
use std::path::Path;

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
auditor_names=[""]
audit_folder_path = ""
program_lib_path = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
auditor=""
"#;

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";
pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub const GIT_IGNORE_STR: &str = r#"BatAuditor.toml"#;

pub fn create_project() {
    let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);
    let gitignore_toml_path = Path::new(&".gitignore");

    if bat_toml_path.exists() {
        panic!(
            "Bat.toml file already exist in {:?}, aborting",
            bat_toml_path
        )
    };

    if auditor_toml_path.exists() {
        panic!(
            "BatAudit.toml file already exist in {:?}, aborting",
            auditor_toml_path
        )
    };

    if gitignore_toml_path.exists() {
        println!(
            ".gitignore file already exist in {:?}, please add BatAuditor.toml",
            gitignore_toml_path
        )
    };

    fs::write(bat_toml_path.clone(), BAT_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
    fs::write(auditor_toml_path.clone(), AUDITOR_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
    fs::write(gitignore_toml_path.clone(), GIT_IGNORE_STR).expect("Could not write to file!");
    println!("Bat.toml created at {:?}", bat_toml_path.clone());
    println!("BatAuditor.toml created at {:?}", auditor_toml_path.clone());
    println!(".gitignore created at {:?}", gitignore_toml_path.clone());
}
