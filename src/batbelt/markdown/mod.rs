use crate::batbelt::templates::code_overhaul::{CodeOverhaulFile, CodeOverhaulSection};
use rand::distributions::Alphanumeric;
use rand::Rng;
use std::fs;

#[derive(thiserror::Error, Debug)]
pub enum MarkdownError {
    #[error("{sections_amount:?} H1 sections detected with name {section_name:?}")]
    DuplicatedH1Sections {
        sections_amount: usize,
        section_name: String,
    },
    #[error("no H1 sections detected with name {section_name:?}")]
    NoH1SectionFound { section_name: String },
    #[error("Sections hash dont match \n new_header: \n{section_header_new:#?} old_header: \n{section_header_old:#?} ")]
    ReplaceSectionHashMismatch {
        section_header_new: MarkdownSectionHeader,
        section_header_old: MarkdownSectionHeader,
    },
    #[error("Can only replace header with H1 level, section_header: \n{section_header:#?} ")]
    OnlyH1CanBeReplaced {
        section_header: MarkdownSectionHeader,
    },
    #[error("New section can only include subsections with level lower than parent section, \n parent_section: \n{parent_section_header:#?} \n target_section: \n{target_section_header:#?}")]
    ReplaceSectionSubsectionsLevel {
        parent_section_header: MarkdownSectionHeader,
        target_section_header: MarkdownSectionHeader,
    },
}

fn get_section_hash() -> String {
    let s: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    s
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownSectionHeader {
    pub section_header: String,
    pub title: String,
    pub prefix: String,
    pub section_level: MarkdownSectionLevel,
    pub section_hash: String,
    pub start_line_index: usize,
}

impl MarkdownSectionHeader {
    pub fn new(
        header: String,
        title: String,
        prefix: String,
        level: MarkdownSectionLevel,
        section_hash: String,
        start_line_index: usize,
    ) -> Self {
        MarkdownSectionHeader {
            section_header: header,
            title,
            prefix,
            section_level: level,
            section_hash,
            start_line_index,
        }
    }

    pub fn new_from_header_and_hash(
        header: String,
        section_hash: String,
        start_line_index: usize,
    ) -> Self {
        let mut header_split = header.trim().split(" ");
        let prefix = header_split.next().unwrap().to_string();
        let title = header_split.collect::<Vec<&str>>().join(" ");
        let level = MarkdownSectionLevel::from_prefix(&prefix);
        MarkdownSectionHeader::new(header, title, prefix, level, section_hash, start_line_index)
    }
}

#[derive(Debug, Clone)]
pub struct MarkdownFile {
    pub path: String,
    pub content: String,
    pub sections: Vec<MarkdownSection>,
}

// const PATH_SEPARATOR: &'static str = "/___/";

impl MarkdownFile {
    pub fn new(path: &str) -> Self {
        let md_file_content = fs::read_to_string(path).unwrap();
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: md_file_content.clone(),
            sections: vec![],
        };
        md_file.get_sections();
        md_file
    }

    pub fn new_from_path_and_content(path: &str, content: String) -> Self {
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: content.to_string(),
            sections: vec![],
        };
        md_file.get_sections();
        md_file
    }

    pub fn save(&mut self) -> Result<(), String> {
        fs::write(&self.path, &self.content).unwrap();
        Ok(())
    }

    pub fn get_section(&self, title: &str) -> Result<MarkdownSection, MarkdownError> {
        let mut section = self
            .sections
            .clone()
            .into_iter()
            .filter(|section| section.section_header.section_level == MarkdownSectionLevel::H1)
            .filter(|section| section.section_header.title == title);
        let section_count = section.clone().count();
        if section_count > 1 {
            return Err(MarkdownError::DuplicatedH1Sections {
                sections_amount: section_count,
                section_name: title.to_string(),
            });
        }
        if section_count == 0 {
            return Err(MarkdownError::NoH1SectionFound {
                section_name: title.to_string(),
            });
        }
        Ok(section.next().unwrap())
    }

    pub fn get_subsection(
        &self,
        title: &str,
        parent_header: MarkdownSectionHeader,
    ) -> MarkdownSection {
        let section = self
            .sections
            .clone()
            .into_iter()
            .find(|section| {
                section.section_header.title == title
                    && section.section_header.section_hash == parent_header.section_hash
            })
            .unwrap();
        section
    }

    pub fn get_section_subsections(&self, section: MarkdownSection) -> Vec<MarkdownSection> {
        let section: Vec<MarkdownSection> = self
            .sections
            .clone()
            .into_iter()
            .filter(|document_section| {
                document_section.section_header.section_hash == section.section_header.section_hash
                    && document_section.section_header.section_level
                        > section.section_header.section_level
            })
            .collect::<Vec<_>>();
        section
    }

    pub fn replace_section(
        &mut self,
        new_parent_section: MarkdownSection,
        old_parent_section: MarkdownSection,
        new_sections: Vec<MarkdownSection>,
    ) -> Result<(), MarkdownError> {
        if new_parent_section.section_header.section_hash
            != old_parent_section.section_header.section_hash
        {
            return Err(MarkdownError::ReplaceSectionHashMismatch {
                section_header_new: new_parent_section.section_header,
                section_header_old: old_parent_section.section_header,
            });
        }

        if new_parent_section.section_header.section_level != MarkdownSectionLevel::H1 {
            return Err(MarkdownError::OnlyH1CanBeReplaced {
                section_header: new_parent_section.section_header,
            });
        }

        for new_section in new_sections.clone() {
            if new_section.section_header.section_level
                <= new_parent_section.section_header.section_level
            {
                return Err(MarkdownError::ReplaceSectionSubsectionsLevel {
                    parent_section_header: new_parent_section.section_header,
                    target_section_header: new_section.section_header,
                });
            }
        }

        let md_old_subsections = self.get_section_subsections(old_parent_section.clone());
        let old_parent_section_index = self
            .sections
            .clone()
            .iter()
            .position(|section| section.section_header == old_parent_section.section_header)
            .unwrap();
        let mut new_sections = new_sections.clone();
        let mut new_sections_vec = vec![new_parent_section];
        new_sections_vec.append(&mut new_sections);
        if !md_old_subsections.is_empty() {
            let last_subsection = md_old_subsections.last().unwrap();
            let last_subsection_index = self
                .sections
                .iter()
                .position(|section| section.section_header == last_subsection.section_header)
                .unwrap();
            self.sections.splice(
                old_parent_section_index..=last_subsection_index,
                new_sections_vec,
            );
        } else {
            self.sections.splice(
                old_parent_section_index..=old_parent_section_index,
                new_sections_vec,
            );
        }
        let new_content = self
            .sections
            .iter()
            .fold("".to_string(), |result, section| {
                if result.is_empty() {
                    section.get_md_section_content()
                } else {
                    format!(
                        "{}\n\n{}",
                        result.clone().trim(),
                        section.get_md_section_content().clone().trim()
                    )
                }
            });
        self.content = new_content;
        self.get_sections();
        Ok(())
    }

    fn get_sections(&mut self) {
        let section_headers: Vec<MarkdownSectionHeader> = self.get_headers();
        let sections: Vec<MarkdownSection> = section_headers
            .iter()
            .map(|section_header| {
                let new_section = MarkdownSection::new_from_md_content_and_header(
                    section_header.clone(),
                    &self.content.clone(),
                );
                new_section
            })
            .collect();
        self.sections = sections;
    }

    fn get_headers(&self) -> Vec<MarkdownSectionHeader> {
        let headers_string: Vec<(String, usize)> = self
            .content
            .lines()
            .enumerate()
            .filter_map(|line| {
                if line.1.contains("#[account") {
                    return None;
                };
                let trailing_ws = Self::get_trailing_whitespaces(line.1);
                if trailing_ws == 0 {
                    if line.1.trim().split(" ").next().unwrap().contains("#") {
                        Some((line.1.to_string(), line.0))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        let mut section_hash = String::new();
        let headers = headers_string
            .iter()
            .map(|header_string| {
                let header_id = header_string.0.trim().split(" ").next().unwrap();
                if header_id == "#" {
                    section_hash = get_section_hash();
                }
                let header = MarkdownSectionHeader::new_from_header_and_hash(
                    header_string.0.clone(),
                    section_hash.clone(),
                    header_string.1,
                );
                header
            })
            .collect();
        headers
    }

    fn get_trailing_whitespaces(line: &str) -> usize {
        let trailing_whitespaces: usize = line
            .chars()
            .take_while(|ch| ch.is_whitespace() && *ch != '\n')
            .map(|ch| ch.len_utf8())
            .sum();
        trailing_whitespaces
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownSection {
    pub section_header: MarkdownSectionHeader,
    pub content: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl MarkdownSection {
    pub fn new(
        section_header: MarkdownSectionHeader,
        content: String,
        start_line_index: usize,
        end_line_index: usize,
    ) -> MarkdownSection {
        MarkdownSection {
            section_header,
            content,
            start_line_index,
            end_line_index,
        }
    }

    pub fn new_from_md_content_and_header(
        header: MarkdownSectionHeader,
        md_file_content: &str,
    ) -> MarkdownSection {
        let content_lines = md_file_content.lines();
        let start_line_index = header.start_line_index;
        let end_line_index_sec = content_lines.clone().enumerate().position(|line| {
            (line.1.trim().split(" ").next().unwrap().contains("#") && line.0 > start_line_index)
                && !line.1.contains("#[account")
        });
        let md_section = if let Some(end_line_index) = end_line_index_sec {
            let section_content = content_lines
                .clone()
                .map(|line| line.to_string())
                .collect::<Vec<_>>()[start_line_index + 1..end_line_index]
                .to_vec()
                .join("\n")
                .trim_start()
                .to_string();
            MarkdownSection::new(header, section_content, start_line_index, end_line_index)
        } else {
            // no end line means that the file ends with a section header
            MarkdownSection::new(header, "".to_string(), start_line_index, start_line_index)
        };
        md_section
    }

    pub fn get_md_section_content(&self) -> String {
        format!(
            "{}\n\n{}",
            self.section_header.section_header.trim(),
            self.content.clone().trim()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MarkdownSectionLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
}

impl MarkdownSectionLevel {
    pub fn get_subsection_level(&self) -> MarkdownSectionLevel {
        match self {
            MarkdownSectionLevel::H1 => MarkdownSectionLevel::H2,
            MarkdownSectionLevel::H2 => MarkdownSectionLevel::H3,
            MarkdownSectionLevel::H3 => MarkdownSectionLevel::H4,
            MarkdownSectionLevel::H4 => MarkdownSectionLevel::H5,
            MarkdownSectionLevel::H5 => unimplemented!(),
        }
    }

    pub fn get_prefix(&self) -> String {
        match self {
            MarkdownSectionLevel::H1 => "#".to_string(),
            MarkdownSectionLevel::H2 => "##".to_string(),
            MarkdownSectionLevel::H3 => "###".to_string(),
            MarkdownSectionLevel::H4 => "####".to_string(),
            MarkdownSectionLevel::H5 => "#####".to_string(),
        }
    }

    pub fn from_prefix(prefix: &str) -> Self {
        match prefix {
            "#" => MarkdownSectionLevel::H1,
            "##" => MarkdownSectionLevel::H2,
            "###" => MarkdownSectionLevel::H3,
            "####" => MarkdownSectionLevel::H4,
            _ => unimplemented!(),
        }
    }

    pub fn get_subsection_prefix(&self) -> String {
        match self {
            MarkdownSectionLevel::H1 => "##".to_string(),
            MarkdownSectionLevel::H2 => "###".to_string(),
            MarkdownSectionLevel::H3 => "####".to_string(),
            MarkdownSectionLevel::H4 => "#####".to_string(),
            MarkdownSectionLevel::H5 => "######".to_string(),
        }
    }

    pub fn get_supersection_prefix(&self) -> String {
        match self {
            MarkdownSectionLevel::H1 => "#".to_string(),
            MarkdownSectionLevel::H2 => "#".to_string(),
            MarkdownSectionLevel::H3 => "##".to_string(),
            MarkdownSectionLevel::H4 => "###".to_string(),
            MarkdownSectionLevel::H5 => "####".to_string(),
        }
    }

    pub fn get_header(&self, title: &str) -> String {
        format!("{} {}\n", self.get_prefix(), title)
    }

    pub fn get_subsection_header(&self, title: &str) -> String {
        format!("{} {}\n", self.get_subsection_prefix(), title)
    }
}

#[derive(Clone, Debug)]
struct TestMarkdownSection {
    pub header: MarkdownSectionHeader,
    pub content: String,
    pub sections: Vec<TestMarkdownSection>,
}

impl TestMarkdownSection {
    pub fn new(section_header: MarkdownSectionHeader) -> Self {
        let content = format!(
            "{}\n{} content {}",
            section_header.section_header, section_header.title, section_header.section_hash
        );
        TestMarkdownSection {
            header: section_header,
            content,
            sections: vec![],
        }
    }
    // sections: [(2,3), (1,2)] -> first subsection got 2 subsubsections, and each got 3 sections_4
    pub fn generate_subsections(&mut self, parent_ordinal_index: usize, sections: (usize, usize)) {
        let (number_of_sections_level_2, number_of_sections_level_3) = sections;
        for index_of_section_level_2 in 0..number_of_sections_level_2 {
            let level = MarkdownSectionLevel::H2;
            let section_header = Self::get_section_header(
                level,
                index_of_section_level_2,
                parent_ordinal_index,
                self.header.section_hash.clone(),
            );
            let mut section_sub_2 = TestMarkdownSection::new(section_header);
            for index_of_section_level_3 in 0..number_of_sections_level_3 {
                let level = MarkdownSectionLevel::H3;
                let section_header = Self::get_section_header(
                    level,
                    index_of_section_level_3,
                    parent_ordinal_index,
                    self.header.section_hash.clone(),
                );
                let mut section_sub_3 = TestMarkdownSection::new(section_header);
                section_sub_3.parse_content();
                section_sub_2.sections.push(section_sub_3);
            }
            section_sub_2.parse_content();
            self.sections.push(section_sub_2);
        }
        self.parse_content();
    }

    pub fn parse_section_title(
        ordinal_index: usize,
        parent_ordinal_index: usize,
        level: MarkdownSectionLevel,
    ) -> String {
        let ordinal = Self::get_ordinal_string(ordinal_index);

        match level {
            MarkdownSectionLevel::H1 => format!("{} section", ordinal),
            MarkdownSectionLevel::H2 => {
                format!("{} subsection_parent_{}", ordinal, parent_ordinal_index)
            }
            MarkdownSectionLevel::H3 => {
                format!("{} subsubsection_parent_{}", ordinal, parent_ordinal_index)
            }
            MarkdownSectionLevel::H4 => {
                format!("{} section_4_parent_{}", ordinal, parent_ordinal_index)
            }
            _ => unimplemented!(),
        }
    }

    fn get_section_header(
        level: MarkdownSectionLevel,
        ordinal_index: usize,
        parent_ordinal_index: usize,
        section_hash: String,
    ) -> MarkdownSectionHeader {
        let title = Self::parse_section_title(ordinal_index, parent_ordinal_index, level);
        let header = level.get_header(&title);
        let section_header = MarkdownSectionHeader::new_from_header_and_hash(
            header.clone(),
            section_hash.clone(),
            0,
        );
        section_header
    }

    pub fn parse_content(&mut self) {
        if !self.sections.is_empty() {
            let parsed_content = self.sections.iter().enumerate().fold(
                "".to_string(),
                |total, (section_index, section)| {
                    if section_index == 0 {
                        format!("{}\n", section.content)
                    } else {
                        format!("{}\n{}", total, section.content)
                    }
                },
            );
            self.content = format!("{}\n\n{}", self.content, parsed_content);
        }
    }

    fn get_ordinal_string(ordinal_index: usize) -> &'static str {
        let ordinal = match ordinal_index {
            0 => "First",
            1 => "Second",
            2 => "Third",
            3 => "Fourth",
            _ => unimplemented!(),
        };
        ordinal
    }

    // pub fn to_markdown_section(&self, md_content: &str) -> MarkdownSection {
    //     MarkdownSection::new_from_md_content_and_header(self.header.clone(), md_content)
    // }
}

#[derive(Debug)]
struct MarkdownTester {
    pub markdown_file: MarkdownFile,
    pub test_sections: Vec<TestMarkdownSection>,
}

impl MarkdownTester {
    pub fn new(path: &str, sections_generator: Vec<(usize, usize)>) -> Self {
        let mut test_sections: Vec<TestMarkdownSection> = vec![];
        let mut md_content = String::new();
        for (section_index, section_generator) in sections_generator.iter().enumerate() {
            let section_hash = get_section_hash();
            let section_header = TestMarkdownSection::get_section_header(
                MarkdownSectionLevel::H1,
                section_index,
                0,
                section_hash,
            );
            let mut test_section = TestMarkdownSection::new(section_header);
            test_section.generate_subsections(section_index, *section_generator);
            test_sections.push(test_section.clone());
            if section_index == 0 {
                md_content = format!("{}\n", test_section.content.clone())
            } else {
                md_content = format!("{}\n{}", md_content, test_section.content.clone())
            }
        }
        let mut markdown_file = MarkdownFile::new_from_path_and_content(path, md_content);
        markdown_file.get_sections();
        MarkdownTester {
            markdown_file,
            test_sections,
        }
    }

    fn content_parser(sections: Vec<TestMarkdownSection>) -> String {
        let content =
            sections
                .iter()
                .enumerate()
                .fold("".to_string(), |total, (section_index, section)| {
                    if section_index == 0 {
                        format!("{}\n", section.content)
                    } else {
                        format!("{}\n{}\n", total, section.content)
                    }
                });
        content
    }
}

#[test]
fn test_top_level_identifier() {
    let s: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    println!("{}", s);
}

#[test]
fn test_title_generation() {
    let generator = vec![(2, 2), (0, 0), (0, 0)];
    let MarkdownTester { test_sections, .. } = MarkdownTester::new(".", generator.clone());
    assert_eq!(
        TestMarkdownSection::parse_section_title(0, 0, MarkdownSectionLevel::H1),
        "First section",
        "Incorrect title"
    );
    assert_eq!(
        TestMarkdownSection::parse_section_title(0, 0, MarkdownSectionLevel::H2),
        "First subsection_parent_0",
        "Incorrect title"
    );
    for (test_section_index, test_section) in test_sections.iter().enumerate() {
        let expected_title = TestMarkdownSection::parse_section_title(
            test_section_index,
            0,
            MarkdownSectionLevel::H1,
        );
        assert_eq!(
            test_section.header.title, expected_title,
            "Incorrect section level 1 title"
        );
        for (test_subsection_index, test_subsection) in test_section.sections.iter().enumerate() {
            let expected_title = TestMarkdownSection::parse_section_title(
                test_subsection_index,
                test_section_index,
                MarkdownSectionLevel::H2,
            );
            assert_eq!(
                test_subsection.header.title, expected_title,
                "Incorrect section level 2 title"
            );
            for (test_subsubsection_index, test_subsubsection) in
                test_subsection.sections.iter().enumerate()
            {
                let expected_title = TestMarkdownSection::parse_section_title(
                    test_subsubsection_index,
                    test_section_index,
                    MarkdownSectionLevel::H3,
                );
                assert_eq!(
                    test_subsubsection.header.title, expected_title,
                    "Incorrect section level 3 title"
                );
            }
        }
    }
}

#[test]
fn test_sections_len() {
    let generator = vec![(2, 3), (0, 0), (0, 0)];
    let markdown_tester = MarkdownTester::new(".", generator.clone());
    let MarkdownTester { test_sections, .. } = markdown_tester;
    assert_eq!(
        test_sections.len(),
        generator.len(),
        "incorrect test_sections len"
    );
    for (test_section_index, test_section) in test_sections.iter().enumerate() {
        let section_level_2_correct_len =
            test_section.sections.len() == generator[test_section_index].0;
        assert!(
            section_level_2_correct_len,
            "incorrect section level 2 subsections len"
        );

        let section_level_3_correct_len = test_section
            .sections
            .iter()
            .all(|subsection| subsection.sections.len() == generator[test_section_index].1);
        assert!(
            section_level_3_correct_len,
            "incorrect section level 3 subsections len"
        );
    }
}
//
#[test]
fn test_get_markdown_section_subsections() {
    let generator = vec![(1, 2), (2, 3)];
    let markdown_tester = MarkdownTester::new(".", generator.clone());
    let MarkdownTester {
        test_sections,
        markdown_file,
    } = markdown_tester;

    let first_section = markdown_file.sections[0].clone();
    let first_subsection = markdown_file.sections[1].clone();
    let first_section_subsections = markdown_file.get_section_subsections(first_section);

    let first_subsection_subsections = markdown_file.get_section_subsections(first_subsection);
    println!("{:#?}", first_section_subsections);
    println!("{:#?}", first_subsection_subsections);
}

#[test]
fn test_replace_section() {
    let generator = vec![(2, 2), (1, 1)];
    let markdown_tester = MarkdownTester::new(".", generator.clone());
    let MarkdownTester {
        mut markdown_file,
        test_sections,
    } = markdown_tester;
    let first_section = markdown_file.get_section("First section").unwrap().clone();
    let mut first_section_subsections =
        markdown_file.get_section_subsections(first_section.clone());

    let mut replace_subsection = first_section_subsections[0].clone();
    replace_subsection.section_header = MarkdownSectionHeader::new_from_header_and_hash(
        "## First new subsection_parent_0".to_string(),
        replace_subsection.section_header.section_hash,
        0,
    );
    first_section_subsections[0] = replace_subsection.clone();
    markdown_file
        .replace_section(
            first_section.clone(),
            first_section.clone(),
            first_section_subsections,
        )
        .unwrap();
    let new_first_section = markdown_file.get_section("First section").unwrap().clone();
    let new_first_section_subsections = markdown_file
        .get_section_subsections(new_first_section)
        .clone();
    assert_eq!(
        replace_subsection.section_header.title,
        new_first_section_subsections[0].section_header.title,
        "error replacing"
    );
    assert_eq!(
        replace_subsection.section_header.section_level,
        new_first_section_subsections[0]
            .section_header
            .section_level,
        "error replacing"
    );
    assert_eq!(
        replace_subsection.section_header.prefix,
        new_first_section_subsections[0].section_header.prefix,
        "error replacing"
    );
    assert_eq!(
        replace_subsection.section_header.section_header,
        new_first_section_subsections[0]
            .section_header
            .section_header,
        "error replacing"
    );
    assert!(
        markdown_file
            .content
            .contains(&replace_subsection.section_header.section_header),
        "new header not found"
    );
}

#[test]
fn test_replace_co_file() {
    let new_signers_content = "
- key: The key authorized for this instruction
- funder: The funder - pays for account rent
";
    let new_function_params_content = "- input: RecipeIngredients";
    let new_ca_content = "
- ```rust
  pub struct AddConsumableInputToRecipe<'info> {
      /// The key authorized for this instruction
      pub key: Signer<'info>,
  
      /// The crafting permissions [`Profile`](player_profile::state::Profile)
      pub profile: AccountLoader<'info, Profile>,
  
      /// The funder - pays for account rent
      #[account(mut)]
      pub funder: Signer<'info>,
  
      /// The [Recipe] account
      #[account(mut)]
      pub recipe: AccountLoader<'info, Recipe>,
  
      /// The [Domain] account
      pub domain: AccountLoader<'info, Domain>,
  
      /// The Mint Account
      pub mint: Account<'info, Mint>,
  
      /// The System program
      pub system_program: Program<'info, System>,
  }
  ```";

    let new_validations_content = "
- ```rust
    #[account(
        mut,
        has_one = domain @Errors::IncorrectDomain
    )]
    pub recipe: AccountLoader<'info, Recipe>,
  ```

- ```rust
    #[account(
        has_one = profile @Errors::IncorrectProfileAddress,
    )]
    pub domain: AccountLoader<'info, Domain>,
  ```


- ```rust
    validate_permissions(
        &ctx.accounts.profile,
        &ctx.accounts.key,
        input.key_index,
        CraftingPermissions::MANAGE_RECIPE,
        None,
    )?;
  ```


- ```rust
    require_keys_eq!(
        input.mint,
        ctx.accounts.mint.key(),
        Errors::IncorrectMintAddress
    );
  ```
";
    let path = "./co_example.md";

    let mut started_markdown_file = CodeOverhaulFile::template_to_markdown_file(path);
    let started_markdown_file_backup = CodeOverhaulFile::template_to_markdown_file(path);
    let signers_title = &CodeOverhaulSection::Signers.to_title();
    let fun_pam_title = &CodeOverhaulSection::FunctionParameters.to_title();
    let ca_title = &CodeOverhaulSection::ContextAccounts.to_title();
    let val_title = &CodeOverhaulSection::Validations.to_title();
    let signers_section = started_markdown_file.get_section(signers_title).unwrap();
    let mut new_signers_section = signers_section.clone();
    new_signers_section.content = new_signers_content.to_string();
    started_markdown_file
        .replace_section(new_signers_section, signers_section.clone(), vec![])
        .unwrap();
    assert_eq!(
        signers_section.content.clone(),
        started_markdown_file_backup
            .get_section(signers_title)
            .unwrap()
            .content,
        "signers section dont match"
    );

    let function_parameters_section = started_markdown_file.get_section(fun_pam_title).unwrap();
    let mut new_fun_param_section = function_parameters_section.clone();
    new_fun_param_section.content = new_function_params_content.to_string();
    started_markdown_file
        .replace_section(
            new_fun_param_section,
            function_parameters_section.clone(),
            vec![],
        )
        .unwrap();

    let context_accounts_section = started_markdown_file.get_section(ca_title).unwrap();
    let mut new_context_accounts_section = context_accounts_section.clone();
    new_context_accounts_section.content = new_ca_content.to_string();
    started_markdown_file
        .replace_section(
            new_context_accounts_section,
            context_accounts_section.clone(),
            vec![],
        )
        .unwrap();
    let validations_section = started_markdown_file.get_section(val_title).unwrap();
    let mut new_validations_section = validations_section.clone();
    started_markdown_file
        .replace_section(new_validations_section, validations_section.clone(), vec![])
        .unwrap();

    assert_eq!(
        signers_section.content.clone(),
        started_markdown_file_backup
            .get_section(signers_title)
            .unwrap()
            .content,
        "signers section dont match"
    );
}
