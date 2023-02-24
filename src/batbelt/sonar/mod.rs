use crate::batbelt;
use crate::batbelt::path::BatFile;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use inflector::Inflector;
use std::error::Error;
use std::fmt::{self, Debug};
use std::time::Duration;
use std::{fs, thread};

pub mod functions;
pub mod structs;

use error_stack::{Result, ResultExt};

#[derive(Debug)]
pub struct BatSonarError;

impl fmt::Display for BatSonarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sonar error")
    }
}

impl Error for BatSonarError {}

#[derive(Clone, Debug)]
pub struct BatSonar {
    pub content: String,
    pub result_type: SonarResultType,
    pub results: Vec<SonarResult>,
    open_filters: SonarFilter,
    end_of_open_filters: SonarFilter,
    closure_filters: SonarFilter,
}

impl BatSonar {
    fn new(content: &str, result_type: SonarResultType) -> Self {
        BatSonar {
            content: content.to_string(),
            results: vec![],
            result_type: result_type.clone(),
            open_filters: SonarFilter::Open(result_type.clone()),
            end_of_open_filters: SonarFilter::EndOfOpen(result_type.clone()),
            closure_filters: SonarFilter::Closure(result_type.clone()),
        }
    }

    pub fn new_scanned(content: &str, result_type: SonarResultType) -> Self {
        let mut new_sonar = BatSonar::new(&content, result_type.clone());
        new_sonar.scan_content_to_get_results();
        new_sonar
    }

    pub fn new_from_path(
        path: &str,
        starting_line_content: Option<&str>,
        result_type: SonarResultType,
    ) -> Self {
        let content = fs::read_to_string(path).unwrap();

        let mut new_sonar = BatSonar::new(&content, result_type.clone());

        if let Some(starting_content) = starting_line_content {
            let start_line_index = content
                .clone()
                .lines()
                .position(|line| line.contains(starting_content))
                .unwrap();
            let first_line = content
                .lines()
                .find(|line| line.contains(starting_content))
                .unwrap();
            let trailing_whitespaces = Self::get_trailing_whitespaces(first_line);
            let end_line_index = new_sonar
                .get_end_line_index(start_line_index, trailing_whitespaces, "")
                .unwrap();
            let new_content = new_sonar.get_result_content(start_line_index, end_line_index);
            new_sonar.content = new_content;
        }
        new_sonar.scan_content_to_get_results();
        new_sonar
    }
    pub fn get_entrypoints_results() -> Result<Self, BatSonarError> {
        let lib_file_path = batbelt::path::get_file_path(BatFile::ProgramLib, false)
            .change_context(BatSonarError)?;
        let entrypoints = BatSonar::new_from_path(
            &lib_file_path,
            Some("#[program]"),
            SonarResultType::Function,
        );
        Ok(entrypoints)
    }

    pub fn scan_content_to_get_results(&mut self) {
        let content_lines = self.content.lines();
        for (line_index, line) in content_lines.enumerate() {
            if self.check_is_opening(line) {
                if self.result_type.test_last_char_is_semicolon() {
                    let last_line_is_semicolon = line.chars().last().unwrap() == ';';
                    if last_line_is_semicolon {
                        continue;
                    }
                }
                let trailing_whitespaces = Self::get_trailing_whitespaces(line);
                let start_line_index = line_index;
                let end_line_index =
                    self.get_end_line_index(start_line_index, trailing_whitespaces, line);
                if end_line_index.is_none() {
                    continue;
                }
                let end_line_index = end_line_index.unwrap();
                let result_content = self.get_result_content(start_line_index, end_line_index);
                let mut sonar_result = SonarResult::new(
                    "",
                    &result_content,
                    trailing_whitespaces,
                    self.result_type.clone(),
                    start_line_index,
                    end_line_index,
                    true,
                );
                if !sonar_result.is_valid_result() {
                    continue;
                }
                // The context account filter duplicates the accouns starting with #[account(
                if (self.result_type == SonarResultType::ContextAccountsAll
                    || self.result_type == SonarResultType::ContextAccountsNoValidation)
                    && self.results.len() > 0
                {
                    let last_result = self.results.clone();
                    let last_result = last_result.last().unwrap();
                    let last_line = last_result.content.clone();
                    let last_line = last_line.lines().last().unwrap();
                    if last_line == sonar_result.content {
                        continue;
                    }
                }
                sonar_result.format_result();
                self.results.push(sonar_result);
            }
        }
    }

    pub fn display_looking_for_loader(result_type: SonarResultType) {
        let pb = ProgressBar::new_spinner();
        pb.enable_steady_tick(Duration::from_millis(200));
        pb.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                .tick_strings(&[
                    "📂                  〰️🦇",
                    "📂                  〰️🦇",
                    "📂                〰️  🦇",
                    "📂              〰️    🦇",
                    "📂            〰️      🦇",
                    "📂          〰️        🦇",
                    "📂        〰️          🦇",
                    "📂      〰️            🦇",
                    "📂    〰️              🦇",
                    "📂  〰️                🦇",
                    "📂〰️                  🦇",
                    "📂  〰️                🦇",
                    "📂    〰️              🦇",
                    "📂      〰️            🦇",
                    "📂        〰️          🦇",
                    "📂          〰️        🦇",
                    "📂            〰️      🦇",
                    "📂              〰️    🦇",
                    "📂                〰️  🦇",
                    "📂                  〰️🦇",
                ]),
        );
        pb.set_message(format!(
            "Looking for {} with {}...",
            result_type.to_string().to_plural().green(),
            "BatSonar".red(),
        ));
        thread::sleep(Duration::from_millis(3400));
        pb.finish_with_message("Done");
    }

    fn get_result_content(&self, start_line_index: usize, end_line_index: usize) -> String {
        let result_content = self.content.lines().collect::<Vec<_>>()
            [start_line_index..=end_line_index]
            .to_vec()
            .join("\n");
        result_content
    }

    fn get_end_line_index(
        &self,
        start_index: usize,
        trailing_whitespaces: usize,
        starting_line: &str,
    ) -> Option<usize> {
        if (self.result_type == SonarResultType::Validation
            || self.result_type == SonarResultType::ContextAccountsAll
            || self.result_type == SonarResultType::ContextAccountsNoValidation)
            && self.starting_line_contains_closure_filter(starting_line)
        {
            return Some(start_index);
        }
        let closing_line_candidates = self.get_closing_lines_candidates(trailing_whitespaces);
        if self.result_type.is_context_accounts_sonar_result_type() {
            let closing_index = self.content.clone().lines().enumerate().position(|line| {
                closing_line_candidates
                    .iter()
                    .any(|candidate| line.1.contains(candidate))
                    && line.0 > start_index
            });
            return closing_index;
        }
        let closing_index = self.content.clone().lines().enumerate().position(|line| {
            closing_line_candidates
                .iter()
                .any(|candidate| line.1 == candidate)
                && line.0 > start_index
        });
        return closing_index;
    }

    fn starting_line_contains_closure_filter(&self, starting_line: &str) -> bool {
        self.closure_filters
            .get_filters()
            .iter()
            .any(|filter| starting_line.contains(filter))
    }

    fn get_closing_lines_candidates(&self, trailing_whitespaces: usize) -> Vec<String> {
        self.closure_filters
            .get_filters()
            .iter()
            .map(|filter| format!("{}{}", " ".repeat(trailing_whitespaces), filter))
            .collect()
    }

    pub fn get_trailing_whitespaces(line: &str) -> usize {
        let trailing_whitespaces: usize = line
            .chars()
            .take_while(|ch| ch.is_whitespace() && *ch != '\n')
            .map(|ch| ch.len_utf8())
            .sum();
        trailing_whitespaces
    }

    fn check_is_opening(&self, line: &str) -> bool {
        let open_filters = self.open_filters.get_filters();
        let end_of_open_filters = self.end_of_open_filters.get_filters();
        if !open_filters.iter().any(|filter| line.contains(filter)) {
            return false;
        }
        if !end_of_open_filters
            .iter()
            .any(|filter| line.contains(filter))
        {
            return false;
        }
        // Check if open is the preffix of the line
        for filter in open_filters {
            let suffix_strip = line.trim().strip_prefix(filter);
            if let Some(_) = suffix_strip {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Debug)]
pub struct SonarResult {
    pub name: String,
    pub content: String,
    pub trailing_whitespaces: usize,
    pub result_type: SonarResultType,
    pub start_line_index: usize,
    pub end_line_index: usize,
    pub is_public: bool,
}

impl SonarResult {
    pub fn new(
        name: &str,
        content: &str,
        trailing_whitespaces: usize,
        result_type: SonarResultType,
        start_line_index: usize,
        end_line_index: usize,
        is_public: bool,
    ) -> Self {
        let new_result = SonarResult {
            name: name.to_string(),
            content: content.to_string(),
            trailing_whitespaces,
            result_type,
            start_line_index,
            end_line_index,
            is_public,
        };
        new_result
    }

    pub fn is_valid_result(&self) -> bool {
        match self.result_type {
            SonarResultType::IfValidation => self.is_valid_if_validation(),
            SonarResultType::ContextAccountsOnlyValidation => self.is_valid_ca_only_validation(),
            _ => true,
        }
    }

    pub fn format_result(&mut self) {
        match self.result_type {
            SonarResultType::Function => self.get_name(),
            SonarResultType::Struct => self.get_name(),
            SonarResultType::Module => self.get_name(),
            SonarResultType::ContextAccountsAll => self.get_name(),
            SonarResultType::ContextAccountsOnlyValidation => {
                self.get_name();
                self.format_ca_only_validations()
            }
            SonarResultType::ContextAccountsNoValidation => {
                self.get_name();
                self.format_ca_no_validations()
            }
            _ => {}
        }
    }

    fn get_name(&mut self) {
        match self.result_type {
            SonarResultType::Function | SonarResultType::Struct | SonarResultType::Module => {
                let first_line = self.content.clone();
                let first_line = first_line.lines().next().unwrap();
                let mut first_line_tokenized = first_line.trim().split(" ");
                let is_public = first_line_tokenized.next().unwrap().contains("pub");
                let is_crate = first_line_tokenized.next().unwrap().contains("(crate)");
                if is_crate {
                    first_line_tokenized.next().unwrap();
                }
                let name_candidate = first_line_tokenized.next().unwrap();
                let name = name_candidate
                    .split("<")
                    .next()
                    .unwrap()
                    .split("(")
                    .next()
                    .unwrap();
                self.name = name.to_string();
                self.is_public = is_public;
            }
            SonarResultType::ContextAccountsAll
            | SonarResultType::ContextAccountsOnlyValidation => {
                let content = self.content.clone();
                let mut last_line = content.lines().last().unwrap().trim().split(" ");
                last_line.next().unwrap();
                let name = last_line.next().unwrap().replace(":", "");
                self.name = name.to_string();
            }
            _ => {}
        }
    }

    fn format_ca_only_validations(&mut self) {
        let content = self.content.clone();
        // single line, only filter the first line
        if content.clone().lines().count() == 2 {
            let first_line = content.lines().next().unwrap();
            let first_line_formatted = first_line
                .trim_start()
                .trim_start_matches("#[account(")
                .trim_end_matches(")]");
            let first_line_tokenized = first_line_formatted.split(",");
            let first_line_filtered = first_line_tokenized
                .filter(|token| {
                    self.result_type
                        .get_context_accounts_only_validations_filters()
                        .iter()
                        .any(|filter| token.contains(filter))
                })
                .fold("".to_string(), |result, token| {
                    if result.is_empty() {
                        token.to_string()
                    } else {
                        format!("{},{}", result, token)
                    }
                });
            let last_line = content.lines().last().unwrap();
            self.content = format!(
                "{}#[account({})]\n{}",
                " ".repeat(self.trailing_whitespaces),
                first_line_filtered,
                last_line
            )
        } else {
            // multiline account
            let ca_filters = self
                .result_type
                .get_context_accounts_only_validations_filters();
            let lines_count = content.lines().count();
            // remove first and last line
            let filtered_lines = content.lines().collect::<Vec<_>>()[1..lines_count - 1]
                .to_vec()
                .join("\n")
                .split(",\n")
                .filter(|line| ca_filters.iter().any(|filter| line.contains(filter)))
                .map(|line| line.trim_end_matches(")]").to_string())
                .collect::<Vec<String>>()
                .join("\n");
            let first_line = content.lines().next().unwrap();
            let last_line = content.lines().last().unwrap();
            let formatted_content = format!(
                "{}\n{}\n{})]\n{}",
                first_line,
                filtered_lines,
                " ".repeat(self.trailing_whitespaces),
                last_line
            );
            self.content = formatted_content
        }
    }

    fn format_ca_no_validations(&mut self) {
        let content = self.content.clone();
        if !content.contains("#[account(") {
            return;
        }
        // single line, only filter the first line
        if content.clone().lines().count() == 2 {
            let first_line = content.lines().next().unwrap();
            let first_line_formatted = first_line
                .trim_start()
                .trim_start_matches("#[account(")
                .trim_end_matches(")]");
            let first_line_tokenized = first_line_formatted.split(",");
            let first_line_filtered = first_line_tokenized
                .filter(|token| {
                    !self
                        .result_type
                        .get_context_accounts_only_validations_filters()
                        .iter()
                        .any(|filter| token.contains(filter))
                })
                .collect::<Vec<_>>();
            let last_line = content.lines().last().unwrap();
            if first_line_filtered.len() == 0 {
                self.content = last_line.to_string();
            } else {
                let result_filtered = first_line_filtered.join(",");
                self.content = format!(
                    "{}#[account({})]\n{}",
                    " ".repeat(self.trailing_whitespaces),
                    result_filtered,
                    last_line
                )
            }
        } else {
            // multiline account
            let ca_filters = self
                .result_type
                .get_context_accounts_only_validations_filters();
            let lines_count = content.lines().count();
            // remove first and last line
            let filtered_lines = content.lines().collect::<Vec<_>>()[1..lines_count - 1]
                .to_vec()
                .join("\n")
                .split(",\n")
                .filter(|line| {
                    !ca_filters.iter().any(|filter| line.contains(filter)) && line.trim() != ")]"
                })
                .map(|line| line.trim_end_matches(")]").to_string())
                .collect::<Vec<String>>();
            let first_line = content.lines().next().unwrap();
            let last_line = content.lines().last().unwrap();
            if filtered_lines.len() == 0 {
                self.content = last_line.to_string();
            } else if filtered_lines.len() == 1 {
                // only 1 line, convert multiline to single line
                let formatted_content = format!(
                    "{}{})]\n{}",
                    first_line.trim_end_matches("\n"),
                    filtered_lines[0].trim().trim_end_matches("\n"),
                    last_line
                );
                self.content = formatted_content
            } else {
                let filtered_lines = filtered_lines.join(",\n");
                let formatted_content = format!(
                    "{}\n{})]\n{}",
                    first_line,
                    filtered_lines,
                    // " ".repeat(self.trailing_whitespaces),
                    last_line
                );
                self.content = formatted_content
            }
        }
    }

    fn is_valid_ca_only_validation(&self) -> bool {
        self.result_type
            .get_context_accounts_only_validations_filters()
            .iter()
            .any(|filter| self.content.contains(filter))
    }

    fn is_valid_if_validation(&self) -> bool {
        let bat_sonar = BatSonar::new_scanned(&self.content, SonarResultType::Validation);
        !bat_sonar.results.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, strum_macros::Display)]
pub enum SonarResultType {
    Function,
    Struct,
    Module,
    If,
    IfValidation,
    Validation,
    ContextAccountsAll,
    ContextAccountsOnlyValidation,
    ContextAccountsNoValidation,
}

impl SonarResultType {
    pub fn get_context_accounts_sonar_result_types(&self) -> Vec<SonarResultType> {
        vec![
            SonarResultType::ContextAccountsAll,
            SonarResultType::ContextAccountsOnlyValidation,
            SonarResultType::ContextAccountsNoValidation,
        ]
    }

    pub fn is_context_accounts_sonar_result_type(&self) -> bool {
        self.get_context_accounts_sonar_result_types()
            .iter()
            .any(|ca_type| self == ca_type)
    }

    fn get_context_accounts_only_validations_filters(&self) -> Vec<&'static str> {
        vec!["has_one", "constraint"]
    }

    fn test_last_char_is_semicolon(&self) -> bool {
        vec![SonarResultType::Function].contains(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SonarFilter {
    Open(SonarResultType),
    EndOfOpen(SonarResultType),
    Closure(SonarResultType),
}

impl SonarFilter {
    pub fn get_filters(&self) -> Vec<&str> {
        match self {
            SonarFilter::Open(SonarResultType::Function) => vec!["fn", "pub fn", "pub(crate) fn"],
            SonarFilter::EndOfOpen(SonarResultType::Function) => vec!["("],
            SonarFilter::Closure(SonarResultType::Function) => vec!["}"],
            SonarFilter::Open(SonarResultType::Struct) => vec!["struct", "pub struct"],
            SonarFilter::EndOfOpen(SonarResultType::Struct) => vec!["{"],
            SonarFilter::Closure(SonarResultType::Struct) => vec!["}"],
            SonarFilter::Open(SonarResultType::Module) => vec!["mod", "pub mod"],
            SonarFilter::EndOfOpen(SonarResultType::Module) => vec!["{"],
            SonarFilter::Closure(SonarResultType::Module) => vec!["}"],
            SonarFilter::Open(SonarResultType::If) => vec!["if"],
            SonarFilter::EndOfOpen(SonarResultType::If) => vec!["{"],
            SonarFilter::Closure(SonarResultType::If) => vec!["}"],
            SonarFilter::Open(SonarResultType::IfValidation) => vec!["if"],
            SonarFilter::EndOfOpen(SonarResultType::IfValidation) => vec!["{"],
            SonarFilter::Closure(SonarResultType::IfValidation) => vec!["}"],
            SonarFilter::Open(SonarResultType::Validation) => {
                vec!["require", "valid", "assert", "verify"]
            }
            SonarFilter::EndOfOpen(SonarResultType::Validation) => vec!["("],
            SonarFilter::Closure(SonarResultType::Validation) => vec![");", ")?;", ")"],
            SonarFilter::Open(SonarResultType::ContextAccountsAll) => {
                vec!["pub", "#[account"]
            }
            SonarFilter::EndOfOpen(SonarResultType::ContextAccountsAll) => vec!["(", ">,"],
            SonarFilter::Closure(SonarResultType::ContextAccountsAll) => vec!["pub", "}"],
            SonarFilter::Open(SonarResultType::ContextAccountsNoValidation) => {
                vec!["pub", "#[account"]
            }
            SonarFilter::EndOfOpen(SonarResultType::ContextAccountsNoValidation) => vec!["(", ">,"],
            SonarFilter::Closure(SonarResultType::ContextAccountsNoValidation) => vec!["pub", "}"],
            SonarFilter::Open(SonarResultType::ContextAccountsOnlyValidation) => {
                vec!["#[account"]
            }
            SonarFilter::EndOfOpen(SonarResultType::ContextAccountsOnlyValidation) => vec!["("],
            SonarFilter::Closure(SonarResultType::ContextAccountsOnlyValidation) => vec!["pub"],
        }
    }
}

#[test]
fn test_get_functions() {
    let expected_second_function = format!(
        "        pub fn create_game_2<'info>(
        ) -> Result<()> {{
            handle_create_game_2(&ctx, key_index, free_create)
        }}"
    );
    let expected_first_function = format!(
        "{}\n{}\n{}\n{}",
        "       pub fn create_game_1<'info>() -> Result<()> {",
        expected_second_function,
        "           handle_create_game_1(&ctx, key_index, free_create)",
        "       }"
    );
    let expected_third_function = format!(
        "        pub fn create_fleet(
            sector: [i64; 2],
        ) -> Result<()> {{
            handle_create_fleet(&ctx, key_index, stats.into(), sector)
        }}"
    );

    let content = format!("{}\n\n{}", expected_first_function, expected_third_function);
    let mut bat_sonar = BatSonar::new_scanned(&content, SonarResultType::Function);
    bat_sonar.scan_content_to_get_results();
    let first_result = bat_sonar.results[0].clone();
    let second_result = bat_sonar.results[1].clone();
    let third_result = bat_sonar.results[2].clone();
    assert_eq!(first_result.content, expected_first_function);
    assert_eq!(first_result.name, "create_game_1");
    assert_eq!(second_result.content, expected_second_function);
    assert_eq!(second_result.name, "create_game_2");
    assert_eq!(third_result.content, expected_third_function);
    assert_eq!(third_result.name, "create_fleet");
}

#[test]
fn test_get_structs() {
    let expected_first_struct = format!(
        "            pub struct StructName {{
                handle_create_game_2(&ctx, key_index, free_create)
            }}"
    );
    let expected_first_function = format!(
        "{}\n{}\n{}\n{}",
        "       pub fn create_game_1<'info>() -> Result<()> {",
        expected_first_struct,
        "           handle_create_game_1(&ctx, key_index, free_create)",
        "       }"
    );
    let expected_second_struct = format!(
        "        struct create_fleet {{
            sector: [i64; 2],
        ) -> Result<()> {{
            handle_create_fleet(&ctx, key_index, stats.into(), sector)
        }}"
    );

    let content = format!("{}\n\n{}", expected_first_function, expected_second_struct);
    let mut bat_sonar = BatSonar::new_scanned(&content, SonarResultType::Struct);
    bat_sonar.scan_content_to_get_results();
    let first_result = bat_sonar.results[0].clone();
    let second_result = bat_sonar.results[1].clone();
    assert_eq!(first_result.content, expected_first_struct);
    assert_eq!(first_result.name, "StructName");
    assert_eq!(second_result.content, expected_second_struct);
    assert_eq!(second_result.name, "create_fleet");
}
#[test]
fn test_get_modules() {
    let expected_first_mod = format!(
        "            pub mod modName {{
                handle_create_game_2(&ctx, key_index, free_create)
            }}"
    );
    let expected_first_function = format!(
        "{}\n{}\n{}\n{}",
        "       pub fn create_game_1<'info>() -> Result<()> {",
        expected_first_mod,
        "           handle_create_game_1(&ctx, key_index, free_create)",
        "       }"
    );
    let expected_second_mod = format!(
        "        mod create_fleet {{
            sector: [i64; 2],
        ) -> Result<()> {{
            handle_create_fleet(&ctx, key_index, stats.into(), sector)
        }}"
    );

    let content = format!("{}\n\n{}", expected_first_function, expected_second_mod);
    let bat_sonar = BatSonar::new_scanned(&content, SonarResultType::Module);
    let first_result = bat_sonar.results[0].clone();
    let second_result = bat_sonar.results[1].clone();
    assert_eq!(first_result.content, expected_first_mod);
    assert_eq!(first_result.name, "modName");
    assert_eq!(second_result.content, expected_second_mod);
    assert_eq!(second_result.name, "create_fleet");
}

#[test]
fn test_get_function_body() {
    // let function_signature = "pub fn cancel_impulse<'info>(ctx: Context<'_, '_, '_, 'info, CancelImpulse<'info>>, key_index: Option<u16>)";
    let function = "pub fn cancel_impulse<'info>()->Result<String, String> { body }";
    let body = function.split("{").collect::<Vec<_>>()[1]
        .split("}")
        .next()
        .unwrap();
    println!("body {:#?}", body)
}
#[test]
fn test_get_if() {
    let test_text = "
    if thing > 1 {
        thing is correct
    } else if {
        thing might not be correct
    } else {
        thing is cool
    }

    this is not an if, even knowing i'm writing if {
        and it looks like an if
    }

    if the_if_dont_get_else {
        and is detected
    }
    ";
    let bat_sonar = BatSonar::new_scanned(test_text, SonarResultType::If);
    println!("sonar \n{:#?}", bat_sonar);
}
#[test]
fn test_get_validation() {
    let test_text = "
    require_eq!(
        this is a valid require
    );

    require_eq!(
        this is not a valid require
    );
    ";
    let bat_sonar = BatSonar::new_scanned(test_text, SonarResultType::Validation);
    println!("sonar \n{:#?}", bat_sonar);
}
#[test]
fn test_get_context_accounts() {
    let test_text = "
    #[derive(Accounts, Debug)]
    pub struct thing<'info> {
        pub acc_1: Signer<'info>,
    
        pub acc_2: AccountLoader<'info, Pf>,
    
        #[account(mut)]
        pub acc_3: Signer<'info>,
    
        #[account(
            mut,
            has_one = thing,
        )]
        pub acc_4: AccountLoader<'info, Rc>,
    
        #[account(
            has_one = thing,
        )]
        pub acc_5: AccountLoader<'info, A>,
    
        pub acc_6: Account<'info, Mint>,
    
        pub acc_7: Program<'info, B>,
    }
    ";
    let bat_sonar = BatSonar::new_scanned(test_text, SonarResultType::ContextAccountsAll);
    assert_eq!(bat_sonar.results.len(), 7, "incorrect results length");
    println!("sonar \n{:#?}", bat_sonar);
}

#[test]
fn test_get_context_accounts_no_validations() {
    let test_text = "
    #[derive(Accounts, Debug)]
    pub struct thing<'info> {
        pub acc_1: Signer<'info>,
    
        #[account(has_one = thing)]
        pub acc_2: AccountLoader<'info, Pf>,
    
        #[account(mut)]
        pub acc_3: Signer<'info>,
    
        #[account(
            mut,
            has_one = thing,
        )]
        pub acc_4: AccountLoader<'info, Rc>,
    
        #[account(
            has_one = thing,
        )]
        pub acc_5: AccountLoader<'info, A>,
    
        pub acc_6: Account<'info, Mint>,
    
        pub acc_7: Program<'info, B>,
    }
    ";
    let bat_sonar = BatSonar::new_scanned(test_text, SonarResultType::ContextAccountsNoValidation);
    assert_eq!(bat_sonar.results.len(), 7, "incorrect results length");
}

#[test]
fn test_context_accounts_only_validations() {
    let test_text = "
    #[derive(Accounts, Debug)]
    pub struct thing<'info> {
        pub acc_1: Signer<'info>,
    
        pub acc_2: AccountLoader<'info, Pf>,
    
        #[account(mut, has_one = thing_to_test)]
        pub acc_3: Signer<'info>,
    
        #[account(
            mut,
            has_one
                =
                    thing,
        )]
        pub acc_4: AccountLoader<'info, Rc>,
    
        #[account(
            mut,
            has_one
                =
                    thing,)]
        pub acc_5: AccountLoader<'info, Rc>,
    
        #[account(
            has_one = thing,
        )]
        pub acc_5: AccountLoader<'info, A>,
    
        #[account(
            has_one = thing,)]
        pub acc_6: AccountLoader<'info, A>,
    
        pub acc_7: Account<'info, Mint>,
    
        pub acc_8: Program<'info, B>,
    }
    ";
    let accounts = BatSonar::new_scanned(test_text, SonarResultType::ContextAccountsOnlyValidation);

    // Only crafting_process, token_from and mint includes #[account
    assert_eq!(accounts.results.len(), 3, "incorrect length");
}

#[test]
fn test_if_validation() {
    // vec!["require", "valid", "assert", "verify"]
    let test_text = "
    if this_is_a_validation {
        require_gt!(1,2)
    } else {
        thing 21
    }

    if this_is_a_validation == 2 {
        assert!(1,2)
    }
    
    if this_is_not_a_validation {
        thing 1
    } else {
        thing 21
    }


    ";
    let accounts = BatSonar::new_scanned(test_text, SonarResultType::IfValidation);

    // Only crafting_process, token_from and mint includes #[account
    assert_eq!(accounts.results.len(), 3, "incorrect length");
}
