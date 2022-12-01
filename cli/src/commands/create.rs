use std::fs;
use std::path::Path;

pub const BAT_TOML_INITIAL_CONFIG_STR: &str = r#"
[required]
auditor_names = [""]
audit_folder_path = ""
program_lib_path = ""
"#;
pub const AUDITOR_TOML_INITIAL_CONFIG_STR: &str = r#"
[auditor]
auditor=""
"#;

pub const BAT_TOML_INITIAL_PATH: &str = "Bat.toml";
pub const AUDITOR_TOML_INITIAL_PATH: &str = "BatAuditor.toml";

pub const GIT_IGNORE_STR: &str = r#"BatAuditor.toml"#;

pub fn create_project() {
    create_bat_toml();
    create_auditor_toml();
    create_gitignore();
}

fn create_bat_toml() {
    let bat_toml_path = Path::new(&BAT_TOML_INITIAL_PATH);

    if bat_toml_path.exists() {
        panic!(
            "Bat.toml file already exist in {:?}, aborting",
            bat_toml_path
        )
    };

    fs::write(bat_toml_path.clone(), BAT_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
    println!("Bat.toml created at {:?}", bat_toml_path.clone());
}

fn create_auditor_toml() {
    let auditor_toml_path = Path::new(&AUDITOR_TOML_INITIAL_PATH);

    if auditor_toml_path.exists() {
        panic!(
            "BatAudit.toml file already exist in {:?}, aborting",
            auditor_toml_path
        )
    };

    fs::write(auditor_toml_path.clone(), AUDITOR_TOML_INITIAL_CONFIG_STR)
        .expect("Could not write to file!");
    println!("BatAuditor.toml created at {:?}", auditor_toml_path.clone());
}

fn create_gitignore() {
    let gitignore_toml_path = Path::new(&".gitignore");

    if gitignore_toml_path.exists() {
        println!(
            ".gitignore file already exist in {:?}, please add BatAuditor.toml",
            gitignore_toml_path
        )
    };

    fs::write(gitignore_toml_path.clone(), GIT_IGNORE_STR).expect("Could not write to file!");
    println!(".gitignore created at {:?}", gitignore_toml_path.clone());
}
