pub mod entrypoints {
    use std::{
        borrow::BorrowMut,
        fs::File,
        io::{self, BufRead},
    };

    use crate::config::{BatConfig, RequiredConfig};

    pub fn get_entrypoints_names() -> Vec<String> {
        let BatConfig { required, .. } = BatConfig::get_validated_config();

        let RequiredConfig {
            program_lib_path, ..
        } = required;
        let lib_file = File::open(program_lib_path).unwrap();
        let mut lib_files_lines = io::BufReader::new(lib_file).lines().map(|l| l.unwrap());
        lib_files_lines
            .borrow_mut()
            .enumerate()
            .find(|(_, line)| *line == String::from("#[program]"))
            .unwrap();
        let mut program_lines = vec![String::from(""); 0];
        for (_, line) in lib_files_lines.borrow_mut().enumerate() {
            if line == "}" {
                break;
            }
            program_lines.push(line)
        }
        let entrypoints_names = program_lines
            .iter()
            .filter(|line| line.contains("pub pub fn"))
            .map(|line| line.replace("pub fn ", "").replace("<'info>", ""))
            .map(|line| String::from(line.split('(').collect::<Vec<&str>>()[0]))
            .map(|line| String::from(line.split_whitespace().collect::<Vec<&str>>()[0]))
            .collect::<Vec<String>>();
        entrypoints_names
    }

    fn initialize_code_overhaul_empty_images() {
        let entrypoints_names = get_entrypoints_names();
        let _entrypoints_figures_path = BatConfig::get_auditor_figures_entrypoints_path();
        for _name in entrypoints_names.iter() {
            // create a png file for every name
        }
    }
}