use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::parser::ParserError;
use crate::batbelt::sonar::SonarResult;
use error_stack::{Report, Result, ResultExt};
use regex::Regex;

pub struct FunctionDependencyParser {
    pub is_external: bool,
    pub function_name: String,
    pub dependency_metadata_matches: Option<Vec<FunctionMetadata>>,
}

pub struct FunctionParameterParser {
    pub parameter_name: String,
    pub parameter_type: String,
}

pub struct FunctionParser {
    pub name: String,
    pub content: String,
    pub signature: String,
    pub body: String,
    pub parameters: Vec<FunctionParameterParser>,
    pub dependencies: Vec<FunctionDependencyParser>,
}

impl FunctionParser {
    fn new(name: String, content: String) -> Result<Self, ParserError> {
        let mut new_function_parser = Self {
            name,
            content,
            signature: "".to_string(),
            body: "".to_string(),
            parameters: vec![],
            dependencies: vec![],
        };
        new_function_parser.get_function_signature();
        new_function_parser.get_function_body();
        new_function_parser.get_function_parameters()?;
        new_function_parser.get_function_dependencies()?;
        Ok(new_function_parser)
    }

    pub fn new_from_metadata(function_metadata: FunctionMetadata) -> Result<Self, ParserError> {
        let name = function_metadata.name.clone();
        let content = function_metadata
            .to_source_code(None)
            .get_source_code_content();
        Ok(Self::new(name, content)?)
    }

    pub fn new_from_sonar_result(sonar_result: SonarResult) -> Result<Self, ParserError> {
        let name = sonar_result.name.clone();
        let content = sonar_result.content.clone();
        Ok(Self::new(name, content)?)
    }

    fn get_function_signature(&mut self) {
        let function_signature = self.content.clone();
        let function_signature = function_signature
            .split("{")
            .next()
            .unwrap()
            .split("->")
            .next()
            .unwrap();
        self.signature = function_signature.trim().to_string();
    }

    fn get_function_parameters(&mut self) -> Result<(), ParserError> {
        if self.content.is_empty() || self.signature.is_empty() {
            return Err(Report::new(ParserError).attach_printable(
                "Error parsing function, both content and signature needs to be initialized",
            ))?;
        }
        let content_lines = self.content.lines();
        let function_signature = self.signature.clone();

        //Function parameters
        // single line function
        let parameters = if content_lines.clone().next().unwrap().contains("{") {
            let function_signature_tokenized = function_signature
                .trim_start_matches("pub (crate) fn ")
                .trim_start_matches("pub fn ")
                .split("(")
                .last()
                .unwrap()
                .trim_end_matches(")")
                .split(" ")
                .collect::<Vec<_>>();
            if function_signature_tokenized.is_empty() || function_signature_tokenized[0].is_empty()
            {
                return Ok(());
            }
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
                .map(|line| line.trim().trim_end_matches(",").to_string())
                .collect();
            filtered
        };
        let result = parameters
            .into_iter()
            .map(|parameter| {
                let tokenized = parameter.split(": ");
                FunctionParameterParser {
                    parameter_name: tokenized.clone().next().unwrap().to_string(),
                    parameter_type: tokenized.clone().last().unwrap().to_string(),
                }
            })
            .collect::<Vec<_>>();
        self.parameters = result;
        Ok(())
    }

    fn get_function_body(&mut self) {
        let function_body = self.content.clone();
        let mut body = function_body.split("{");
        body.next();
        let body = body.collect::<Vec<_>>().join("{");
        self.body = body.trim_end_matches("}").trim().to_string();
    }

    fn get_function_dependencies(&mut self) -> Result<(), ParserError> {
        let function_metadata =
            FunctionMetadata::get_filtered_metadata(None, None).change_context(ParserError)?;
        let dependency_regex = Regex::new(r"[A-Za-z0-9_]+\(([A-Za-z0-9_,\s]*)\)").unwrap();
        let dependency_parser_vec = dependency_regex
            .find_iter(&self.body)
            .map(|reg_match| {
                let match_str = reg_match.as_str().to_string();
                let dependency_function_name = Self::get_function_name_from_signature(&match_str);
                let mut new_dep_parser = FunctionDependencyParser {
                    is_external: false,
                    function_name: dependency_function_name.clone(),
                    dependency_metadata_matches: None,
                };
                let match_function_metadata = function_metadata
                    .clone()
                    .into_iter()
                    .filter(|f_metadata| {
                        f_metadata.clone().name == dependency_function_name.clone()
                    })
                    .collect::<Vec<_>>();
                if !match_function_metadata.is_empty() {
                    new_dep_parser.dependency_metadata_matches = Some(match_function_metadata);
                } else {
                    new_dep_parser.is_external = true;
                }
                new_dep_parser
            })
            .collect::<Vec<FunctionDependencyParser>>();
        self.dependencies = dependency_parser_vec;
        Ok(())
    }

    pub fn get_function_name_from_signature(function_signature: &str) -> String {
        function_signature
            .trim_start_matches("pub ")
            .trim_start_matches("(crate) ")
            .trim_start_matches("fn ")
            .split("(")
            .next()
            .unwrap()
            .split("<")
            .next()
            .unwrap()
            .to_string()
    }
}

#[test]

fn test_get_function_dependencies_signatures() {
    let test_text = "
    match function_1(
        param1, 
        param2, 
        param3, 
        param4
    )? {
        Enum1::Variant1 => Ok(Enum1::Variant1),
        Enum1::Vartiant2((permission_key, permissions)) => {
            let permissions = function_3::dep1(permissions);
            Ok(Enum2::Variant1((
                param1,
                P::function3(param1),
                param_a,
            )))
        }
    }
 ";
    let test_text_dep_signatures = FunctionParser::get_function_dependencies(test_text).unwrap();
    println!("deps {:#?}", test_text_dep_signatures);
    // seeds = \(([\s,\t]{0,}[.\n.?][\s\S]{0,}[\s,\t]{0,}[.\n.?][\s,\t]{0,})\)
}
