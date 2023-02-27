use crate::batbelt::path::BatFile;
use crate::batbelt::templates::TemplateError;
use error_stack::{Result, ResultExt};
use inflector::Inflector;

pub struct FindingTemplate;

impl FindingTemplate {
    pub fn new_finding_file(
        finding_name: &str,
        is_informational: bool,
    ) -> Result<(), TemplateError> {
        let finding_title = finding_name.to_sentence_case();
        let content = if is_informational {
            Self::get_informational_content(&finding_title)
        } else {
            Self::get_finding_content(&finding_title)
        };
        BatFile::FindingToReview {
            file_name: finding_name.to_string(),
        }
        .write_content(false, &content)
        .change_context(TemplateError)?;
        Ok(())
    }

    pub fn get_finding_content(finding_title: &str) -> String {
        let content = format!(
            r#"## {}

**Severity:** High

**Status:** Open

| Impact | Likelihood | Difficulty |
| :----: | :--------: | :--------: |
|  High  |   Medium   |    Low     |

### Description {{-}}

Fill the description

### Impact {{-}}

Fill the impact

### Evidence {{-}}

<figure style="display:block">
    <img style="max-width:100%" src="../../figures/finding-name-1.png"/>
</figure>
<figure style="display:block">
    <img style="max-width:100%" src="../../figures/finding-name-2.png"/>
</figure>

Add a description of the evidence here

### Recommendation {{-}}

Add recommendations

### Affected resources {{-}}

- N/A

### Reference {{-}}

- N/A
"#,
            finding_title
        );
        content
    }
    pub fn get_informational_content(finding_title: &str) -> String {
        let content = format!(
            r#"## {}

**Severity:** Informational

**Status:** Open

### Description {{-}}

Add a description

### Evidence {{-}}

<figure style="display:block">
    <img style="max-width:100%" src="../../figures/observation-1.png"/>
</figure>
<figure style="display:block">
    <img style="max-width:100%" src="../../figures/observation-1.png"/>
</figure>

Add a description of the evidence here

### Recommendation {{-}}

Add some recomendations

### Affected resources {{-}}

- Add affected resources

### Reference {{-}}

- N/A
"#,
            finding_title
        );
        content
    }
}

#[test]
fn test_title_parser() {
    let test_text = "hello_how Are-you";
    let file_name = test_text.to_snake_case();
    let title = test_text.to_sentence_case();
    println!("file_name: {}", file_name);
    println!("title: {}", title);
}

#[test]
fn test_content() {
    let test_text = "hello_how Are-you".to_sentence_case();
    let finding_content = FindingTemplate::get_finding_content(&test_text);
    let info_content = FindingTemplate::get_informational_content(&test_text);
    println!("finding: {}", finding_content);
    println!("info: {}", info_content);
}
