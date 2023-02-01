pub mod functions;
pub mod structs;

#[derive(Clone, Debug)]
pub struct BatSonar {
    pub content: String,
    pub result_type: SonarResultType,
    pub result: Vec<SonarResult>,
    pub open_filters: SonarFilter,
    pub end_of_open_filters: SonarFilter,
    pub closure_filters: SonarFilter,
}

impl BatSonar {
    pub fn new(content: &str, result_type: SonarResultType) -> Self {
        BatSonar {
            content: content.to_string(),
            result: vec![],
            result_type: result_type.clone(),
            open_filters: SonarFilter::Open(result_type.clone()),
            end_of_open_filters: SonarFilter::EndOfOpen(result_type.clone()),
            closure_filters: SonarFilter::Closure(result_type.clone()),
        }
    }

    pub fn scan_content_to_get_results(&mut self) {
        let content_lines = self.content.lines();
        for (line_index, line) in content_lines.enumerate() {
            if self.check_is_open(line) {
                let trailing_whitespaces = Self::get_trailing_whitespaces(line);
                let start_line_index = line_index;
                let end_line_index =
                    self.get_end_line_index(start_line_index, trailing_whitespaces);
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
                sonar_result.parse_result(line);
                self.result.push(sonar_result);
            }
        }
    }

    pub fn get_result_content(&self, start_line: usize, end_line: usize) -> String {
        let result_content = self.content.lines().collect::<Vec<_>>()[start_line..=end_line]
            .to_vec()
            .join("\n");
        result_content
    }

    pub fn get_end_line_index(&self, start_index: usize, trailing_whitespaces: usize) -> usize {
        let closing_line_candidates = self.get_closing_lines_candidates(trailing_whitespaces);
        let closing_index = self
            .content
            .clone()
            .lines()
            .enumerate()
            .position(|line| {
                closing_line_candidates
                    .iter()
                    .any(|candidate| line.1 == candidate)
                    && line.0 > start_index
            })
            .unwrap();
        closing_index
    }

    pub fn get_closing_lines_candidates(&self, trailing_whitespaces: usize) -> Vec<String> {
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

    pub fn check_is_open(&self, line: &str) -> bool {
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
    name: String,
    content: String,
    trailing_whitespaces: usize,
    result_type: SonarResultType,
    start_line: usize,
    end_line: usize,
    is_public: bool,
}

impl SonarResult {
    pub fn new(
        name: &str,
        content: &str,
        trailing_whitespaces: usize,
        result_type: SonarResultType,
        start_line: usize,
        end_line: usize,
        is_public: bool,
    ) -> Self {
        SonarResult {
            name: name.to_string(),
            content: content.to_string(),
            trailing_whitespaces,
            result_type,
            start_line,
            end_line,
            is_public,
        }
    }

    pub fn parse_result(&mut self, first_line: &str) {
        let mut first_line_tokenized = first_line.trim().split(" ");
        let is_public = first_line_tokenized.next().unwrap() == "pub";
        if is_public {
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
}

#[derive(Clone, Debug)]
pub enum SonarResultType {
    Function,
    Struct,
    Module,
}

#[derive(Clone, Debug)]
pub enum SonarFilter {
    Open(SonarResultType),
    EndOfOpen(SonarResultType),
    Closure(SonarResultType),
}

impl SonarFilter {
    pub fn get_filters(&self) -> Vec<&str> {
        match self {
            SonarFilter::Open(SonarResultType::Function) => vec!["fn", "pub fn"],
            SonarFilter::EndOfOpen(SonarResultType::Function) => vec!["("],
            SonarFilter::Closure(SonarResultType::Function) => vec!["}"],
            SonarFilter::Open(SonarResultType::Struct) => vec!["struct", "pub struct"],
            SonarFilter::EndOfOpen(SonarResultType::Struct) => vec!["{"],
            SonarFilter::Closure(SonarResultType::Struct) => vec!["}"],
            SonarFilter::Open(SonarResultType::Module) => vec!["mod", "pub mod"],
            SonarFilter::EndOfOpen(SonarResultType::Module) => vec!["{"],
            SonarFilter::Closure(SonarResultType::Module) => vec!["}"],
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
    let mut bat_sonar = BatSonar::new(&content, SonarResultType::Function);
    bat_sonar.scan_content_to_get_results();
    let first_result = bat_sonar.result[0].clone();
    let second_result = bat_sonar.result[1].clone();
    let third_result = bat_sonar.result[2].clone();
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
    let mut bat_sonar = BatSonar::new(&content, SonarResultType::Struct);
    bat_sonar.scan_content_to_get_results();
    let first_result = bat_sonar.result[0].clone();
    let second_result = bat_sonar.result[1].clone();
    assert_eq!(first_result.content, expected_first_struct);
    assert_eq!(first_result.name, "StructName");
    assert_eq!(second_result.content, expected_second_struct);
    assert_eq!(second_result.name, "create_fleet");

    println!("first_result {:#?}", first_result);
    println!("second_result {:#?}", second_result);
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
    let mut bat_sonar = BatSonar::new(&content, SonarResultType::Module);
    bat_sonar.scan_content_to_get_results();
    let first_result = bat_sonar.result[0].clone();
    let second_result = bat_sonar.result[1].clone();
    assert_eq!(first_result.content, expected_first_mod);
    assert_eq!(first_result.name, "modName");
    assert_eq!(second_result.content, expected_second_mod);
    assert_eq!(second_result.name, "create_fleet");

    println!("first_result {:#?}", first_result);
    println!("second_result {:#?}", second_result);
}
