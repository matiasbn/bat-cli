use crate::batbelt::path::BatFile;
use crate::batbelt::templates::TemplateError;
use error_stack::{IntoReport, Result, ResultExt};
use std::fs;

pub struct NoteTemplate;

impl NoteTemplate {
    pub fn create_notes_templates() -> Result<(), TemplateError> {
        Self::create_finding_candidates_file()?;
        Self::create_open_questions_file()?;
        Self::create_threat_modeling_file()?;
        Ok(())
    }

    fn create_finding_candidates_file() -> Result<(), TemplateError> {
        let content = r#"# Finding candidates (a.k.a Smellies)

![Alt Text](https://media3.giphy.com/media/J2gHlRQQvFamqOWlJF/giphy.gif)

## accepted

- [ ] [finding candidate description](http://url-to-client-repository-with-corresponding-branch), little note to describe what to do with it

## rejected

- [ ] [finding candidate description](http://url-to-client-repository-with-corresponding-branch), little note to describe what to do with it

## to review

- [ ] [finding candidate description](http://url-to-client-repository-with-corresponding-branch), little note to describe what to do with it
"#;
        let finding_candidates_path = BatFile::FindingCandidates
            .get_path(false)
            .change_context(TemplateError)?;
        fs::write(finding_candidates_path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn create_open_questions_file() -> Result<(), TemplateError> {
        let content = r#"# Open questions

![Alt Text](http://38.media.tumblr.com/1e3486ff57a997da3ffeea759b8eccde/tumblr_nl4tn2cY1m1spm17no1_400.gif)

- [ ] [open question here](http://url-to-client-repository-with-corresponding-branch), little note to describe what to do with the questions
"#;
        let path = BatFile::OpenQuestions
            .get_path(false)
            .change_context(TemplateError)?;
        fs::write(path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }

    fn create_threat_modeling_file() -> Result<(), TemplateError> {
        let content = r#"# Threat modeling

![Alt Text](https://media.tenor.com/26GU1Sq64AcAAAAC/hacker.gif)

## Assets

### Accounts

-

### Others

-

## Actors

-

## Scenarios

-
"#;
        let path = BatFile::ThreatModeling
            .get_path(false)
            .change_context(TemplateError)?;
        fs::write(path, content)
            .into_report()
            .change_context(TemplateError)?;
        Ok(())
    }
}
