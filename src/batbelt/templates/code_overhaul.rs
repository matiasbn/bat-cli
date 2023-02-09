use crate::batbelt::markdown::MarkdownFile;
use inflector::Inflector;

pub struct CodeOverhaulTemplate {
    pub title: String,
    pub state_changes: String,
    pub notes: String,
    pub signers: Vec<String>,
    pub function_parameters: Vec<String>,
    pub context_accounts: String,
    pub validations: String,
}

impl CodeOverhaulTemplate {
    pub fn new(
        title: String,
        state_changes: String,
        notes: String,
        signers: Vec<String>,
        function_parameters: Vec<String>,
        context_accounts: String,
        validations: String,
    ) -> Self {
        Self {
            title,
            state_changes,
            notes,
            signers,
            function_parameters,
            context_accounts,
            validations,
        }
    }
    pub fn template_to_markdown_file(path: &str) -> MarkdownFile {
        let content = Self::get_template().clone();
        let template = MarkdownFile::new_from_path_and_content(path, content);
        template
    }

    pub fn get_template() -> String {
        format!(
            "{}

            - {}
             
            {}

            - {}

            {}
            
            {}
            
            {}
            
            {}
            ",
            CodeOverhaulSection::StateChanges.to_markdown_header(),
            CoderOverhaulTemplatePlaceholders::CompleteWithStateChanges.to_placeholder(),
            CodeOverhaulSection::Notes.to_markdown_header(),
            CoderOverhaulTemplatePlaceholders::CompleteWithNotes.to_placeholder(),
            CodeOverhaulSection::Signers.to_markdown_header(),
            CodeOverhaulSection::FunctionParameters.to_markdown_header(),
            CodeOverhaulSection::ContextAccounts.to_markdown_header(),
            CodeOverhaulSection::Validations.to_markdown_header(),
        )
        .lines()
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
    }
}

#[derive(strum_macros::Display)]
pub enum CodeOverhaulSection {
    StateChanges,
    Notes,
    Signers,
    FunctionParameters,
    ContextAccounts,
    Validations,
}

impl CodeOverhaulSection {
    pub fn to_markdown_header(&self) -> String {
        format!("# {}:", self.to_string().to_sentence_case())
    }

    pub fn to_title(&self) -> String {
        format!("{}:", self.to_string().to_sentence_case())
    }
}

#[derive(strum_macros::Display)]
pub enum CoderOverhaulTemplatePlaceholders {
    NoSignersDetected,
    NoValidationsDetected,
    NoFunctionParametersDetected,
    CompleteWithStateChanges,
    CompleteWithNotes,
    CompleteWithSignerDescription,
}

impl CoderOverhaulTemplatePlaceholders {
    pub fn to_placeholder(&self) -> String {
        self.to_string().to_snake_case().to_uppercase()
    }
}

#[test]
fn test_to_title() {
    let expected = "Signers:";
    let title = CodeOverhaulSection::Signers.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");

    let expected = "Context accounts:";
    let title = CodeOverhaulSection::ContextAccounts.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");

    let expected = "Validations:";
    let title = CodeOverhaulSection::Validations.to_title();
    println!("title {:#?}", title);
    assert_eq!(expected, title, "Incorrect title");
}
