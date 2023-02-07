use std::fs;

pub mod functions;
pub mod structs;

#[derive(Clone, Debug)]
pub struct BatSonar {
    pub content: String,
    pub result_type: SonarResultType,
    pub results: Vec<SonarResult>,
    pub open_filters: SonarFilter,
    pub end_of_open_filters: SonarFilter,
    pub closure_filters: SonarFilter,
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
            let end_line_index =
                new_sonar.get_end_line_index(start_line_index, trailing_whitespaces);
            let new_content = new_sonar.get_result_content(start_line_index, end_line_index);
            new_sonar.content = new_content;
        }
        new_sonar.scan_content_to_get_results();
        new_sonar
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
                sonar_result.parse_sub_content();
                sonar_result.get_name_and_is_public(line);
                self.results.push(sonar_result);
            }
        }
        self.results
            .sort_by(|result_a, result_b| result_a.name.cmp(&result_b.name));
    }

    fn get_result_content(&self, start_line_index: usize, end_line_index: usize) -> String {
        let result_content = self.content.lines().collect::<Vec<_>>()
            [start_line_index..=end_line_index]
            .to_vec()
            .join("\n");
        result_content
    }

    fn get_end_line_index(&self, start_index: usize, trailing_whitespaces: usize) -> usize {
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

    fn check_is_open(&self, line: &str) -> bool {
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
    pub sub_content: SonarResultSubContent,
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
        SonarResult {
            name: name.to_string(),
            content: content.to_string(),
            trailing_whitespaces,
            result_type,
            start_line_index,
            end_line_index,
            is_public,
            sub_content: SonarResultSubContent::Empty,
        }
    }

    pub fn get_name_and_is_public(&mut self, first_line: &str) {
        let parsed_types = vec![
            SonarResultType::Function,
            SonarResultType::Struct,
            SonarResultType::Module,
        ];
        if parsed_types
            .iter()
            .any(|parsed| parsed.clone() == self.result_type)
        {
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
        } else {
            self.name = "NO_NAME".to_string();
        }
    }

    pub fn parse_sub_content(&mut self) -> SonarResultSubContent {
        match self.result_type {
            SonarResultType::Function => self.parse_function_subcontent(),
            _ => SonarResultSubContent::Empty,
        }
    }

    pub fn parse_function_subcontent(&mut self) -> SonarResultSubContent {
        let parameters = self.parse_function_parameters();
        self.sub_content = SonarResultSubContent::FunctionSubContent {
            parameters,
            body: self.content.clone().split("{").collect::<Vec<_>>()[1]
                .split("}")
                .next()
                .unwrap()
                .to_string(),
        };
        self.sub_content.clone()
    }

    fn parse_function_parameters(&self) -> Vec<String> {
        let content_lines = self.content.lines();
        let function_signature = self.content.clone();
        let function_signature = function_signature
            .split("{")
            .next()
            .unwrap()
            .split("->")
            .next()
            .unwrap();
        //Function parameters
        // single line function
        if content_lines.clone().next().unwrap().contains(")") {
            let function_signature_tokenized = function_signature.split("(").collect::<Vec<_>>()[1]
                .split(")")
                .next()
                .unwrap()
                .split(" ")
                .collect::<Vec<_>>();
            let mut parameters: Vec<String> = vec![];
            function_signature_tokenized.iter().enumerate().fold(
                "".to_string(),
                |total, current| {
                    if current.1.contains(":") {
                        if !total.is_empty() {
                            parameters.push(total);
                        }
                        current.1.to_string()
                    } else if current.0 == function_signature_tokenized.len() - 1 {
                        parameters.push(format!("{} {}", total, current.1));
                        total
                    } else {
                        format!("{} {}", total, current.1)
                    }
                },
            );
            parameters
        } else {
            //multiline
            // parameters contains :
            let filtered: Vec<String> = function_signature
                .lines()
                .filter(|line| line.contains(":"))
                .map(|line| line.trim().to_string())
                .collect();
            filtered
        }
    }
}

#[derive(Clone, Debug)]
pub enum SonarResultSubContent {
    FunctionSubContent {
        parameters: Vec<String>,
        body: String,
    },
    Empty,
}

impl SonarResultSubContent {
    pub fn parse(&self) -> (Vec<String>, String) {
        match self {
            Self::FunctionSubContent { parameters, body } => (parameters.clone(), body.clone()),
            Self::Empty => (vec!["".to_string()], "".to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SonarResultType {
    Function,
    Struct,
    Module,
    If,
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
            SonarFilter::Open(SonarResultType::If) => vec!["if"],
            SonarFilter::EndOfOpen(SonarResultType::If) => vec!["{"],
            SonarFilter::Closure(SonarResultType::If) => vec!["}"],
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
fn test_get_function_parameters() {
    let function_signature = "pub fn cancel_impulse<'info>(ctx: Context<'_, '_, '_, 'info, CancelImpulse<'info>>, key_index: Option<u16>)";
    let function_signature_tokenized = function_signature.split("(").collect::<Vec<_>>()[1]
        .split(")")
        .next()
        .unwrap()
        .split(" ")
        .collect::<Vec<_>>();
    let mut parameters: Vec<String> = vec![];
    function_signature_tokenized
        .iter()
        .enumerate()
        .fold("".to_string(), |total, current| {
            if current.1.contains(":") {
                if !total.is_empty() {
                    parameters.push(total);
                }
                current.1.to_string()
            } else if current.0 == function_signature_tokenized.len() - 1 {
                parameters.push(format!("{} {}", total, current.1));
                total
            } else {
                format!("{} {}", total, current.1)
            }
        });
    assert_eq!(
        parameters[0],
        "ctx: Context<'_, '_, '_, 'info, CancelImpulse<'info>>,"
    );
    assert_eq!(parameters[1], "key_index: Option<u16>");
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
