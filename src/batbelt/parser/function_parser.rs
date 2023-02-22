use crate::batbelt::metadata::functions_metadata::FunctionMetadata;
use crate::batbelt::parser::ParserError;
use error_stack::Result;

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
    pub dependencies: Option<FunctionMetadata>,
}

impl FunctionParser {
    pub fn new_from_metadata(function_metadata: FunctionMetadata) -> Result<Self, ParserError> {
        let name = function_metadata.name.clone();
        let content = function_metadata
            .to_source_code(None)
            .get_source_code_content();
        let signature = Self::get_function_signature(&content);
        let body = Self::get_function_body(&content);
        let parameters = Self::get_function_parameters(&content)?;
        Ok(Self {
            name,
            content,
            signature,
            body,
            parameters,
            dependencies: None,
        })
    }

    pub fn get_function_parameters(
        function_content: &str,
    ) -> Result<Vec<FunctionParameterParser>, ParserError> {
        let content_lines = function_content.lines();
        let function_signature = Self::get_function_signature(&function_content);
        //Function parameters
        // single line function
        // info!("function content: \n {}", function_content);
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
                return Ok(vec![]);
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
        Ok(result)
    }

    pub fn get_function_signature(function_content: &str) -> String {
        let function_signature = function_content.clone();
        let function_signature = function_signature
            .split("{")
            .next()
            .unwrap()
            .split("->")
            .next()
            .unwrap();
        function_signature.trim().to_string()
    }

    pub fn get_function_body(function_content: &str) -> String {
        let function_body = function_content.clone();
        let mut body = function_body.split("{");
        body.next();
        let body = body.collect::<Vec<_>>().join("{");
        body.trim_end_matches("}").trim().to_string()
    }
}
