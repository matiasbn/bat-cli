use rand::distributions::Alphanumeric;
use rand::Rng;
use std::{
    borrow::{Borrow, BorrowMut},
    cell::{Ref, RefCell},
    fs, io,
    ops::Deref,
    rc::{Rc, Weak},
};
use strum_macros;

fn get_section_hash() -> String {
    let s: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    s
}

#[derive(Debug, Clone)]
pub struct SectionHeader {
    pub header: String,
    pub title: String,
    pub prefix: String,
    pub level: MarkdownSectionLevel,
    pub section_hash: String,
}

impl SectionHeader {
    pub fn new(
        header: String,
        title: String,
        prefix: String,
        level: MarkdownSectionLevel,
        section_hash: String,
    ) -> Self {
        SectionHeader {
            header,
            title,
            prefix,
            level,
            section_hash,
        }
    }

    pub fn new_from_header_and_hash(header: String, section_hash: String) -> Self {
        let mut header_split = header.trim().split(" ");
        let prefix = header_split.next().unwrap().to_string();
        let title = header_split.collect::<Vec<&str>>().join(" ");
        let level = MarkdownSectionLevel::from_prefix(&prefix);
        SectionHeader::new(header, title, prefix, level, section_hash)
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

    pub fn get_section(&mut self, title: &str, level: MarkdownSectionLevel) -> MarkdownSection {
        let section = self
            .sections
            .clone()
            .into_iter()
            .find(|section| section.header.title == title && section.header.level == level)
            .unwrap();
        section
    }

    pub fn get_section_subsections(&mut self, section: MarkdownSection) -> Vec<MarkdownSection> {
        let section: Vec<MarkdownSection> = self
            .sections
            .clone()
            .into_iter()
            .filter(|document_section| {
                document_section.header.section_hash == section.header.section_hash
                    && document_section.header.level > section.header.level
            })
            .collect::<Vec<_>>();
        section
    }

    fn get_sections(&mut self) {
        let section_headers: Vec<SectionHeader> = self.get_headers();
        let sections: Vec<MarkdownSection> = section_headers
            .iter()
            .map(|section_header| {
                let new_section = MarkdownSection::new_from_md_content_and_header(
                    section_header.clone(),
                    &self.content,
                );
                new_section
            })
            .collect();
        self.sections = sections;
    }

    fn get_headers(&self) -> Vec<SectionHeader> {
        let headers_string: Vec<String> = self
            .content
            .lines()
            .filter(|line| line.trim().split(" ").next().unwrap().contains("#"))
            .map(|header| header.to_string())
            .collect();
        let mut section_hash = String::new();
        let headers = headers_string
            .iter()
            .map(|header_string| {
                let header_id = header_string.trim().split(" ").next().unwrap();
                if header_id == "#" {
                    section_hash = get_section_hash();
                }
                let header = SectionHeader::new_from_header_and_hash(
                    header_string.clone(),
                    section_hash.clone(),
                );
                header
            })
            .collect();
        headers
    }

    pub fn get_section_content(&self, section: MarkdownSection) -> String {
        let content_lines = self.content.split("\n");
        let section: String = content_lines.collect::<Vec<&str>>()
            [section.start_line_index..=section.end_line_index]
            .to_vec()
            .join("\n");
        section
    }

    // pub get_section_content
}

#[derive(Debug, Clone)]
pub struct MarkdownSection {
    pub header: SectionHeader,
    pub content: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl MarkdownSection {
    pub fn new(
        header: SectionHeader,
        content: String,
        start_line_index: usize,
        end_line_index: usize,
    ) -> MarkdownSection {
        MarkdownSection {
            header,
            content,
            start_line_index,
            end_line_index,
        }
    }

    pub fn new_from_md_content_and_header(
        header: SectionHeader,
        md_file_content: &str,
    ) -> MarkdownSection {
        let content_lines = md_file_content.lines();
        let last_line_index = content_lines.clone().count();
        let start_line_index = content_lines
            .clone()
            .position(|line| line == header.header)
            .unwrap();
        let mut end_line_index = content_lines
            .clone()
            .enumerate()
            .position(|line| {
                (line.1.trim().split(" ").next().unwrap().contains("#")
                    && line.0 > start_line_index)
                    // || line.1.clone().chars().count() == 0
                || line.0 == last_line_index - 1
            })
            .unwrap();
        if end_line_index == last_line_index - 1 {
            end_line_index += 1
        }
        let section_content = content_lines
            .clone()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()[start_line_index + 1..end_line_index]
            .to_vec()
            .join("\n")
            .trim_start()
            .to_string();
        MarkdownSection::new(header, section_content, start_line_index, end_line_index)
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
    pub header: SectionHeader,
    pub content: String,
    pub sections: Vec<TestMarkdownSection>,
}

impl TestMarkdownSection {
    pub fn new(section_header: SectionHeader) -> Self {
        let content = format!(
            "{}\n{} content {}",
            section_header.header, section_header.title, section_header.section_hash
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
    ) -> SectionHeader {
        let title = Self::parse_section_title(ordinal_index, parent_ordinal_index, level);
        let header = level.get_header(&title);
        let section_header =
            SectionHeader::new_from_header_and_hash(header.clone(), section_hash.clone());
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
fn test_markdown_section_new_from_header_and_content() {
    let generator = vec![(1, 0)];
    let markdown_tester = MarkdownTester::new(".", generator.clone());
    let MarkdownTester {
        test_sections,
        markdown_file,
    } = markdown_tester;

    let section_header_content = "# Test section";
    let section_hash = get_section_hash();
    let section_header =
        SectionHeader::new_from_header_and_hash(section_header_content.to_string(), section_hash);

    // let replace_section =
    // assert_eq!(
    //     found_section.title, target_section.title,
    //     "found_section don't match"
    // );
    // assert_eq!(
    //     found_section.content_path, target_section.content_path,
    //     "found_section don't match"
    // );
}
