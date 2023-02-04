use std::{
    borrow::{Borrow, BorrowMut},
    cell::{Ref, RefCell},
    fs, io,
    ops::Deref,
    rc::{Rc, Weak},
};

#[derive(Debug, Clone)]
pub struct MarkdownFile {
    pub path: String,
    pub content: String,
    pub sections: Vec<MarkdownSection>,
}

const PATH_SEPARATOR: &'static str = "/___/";

impl MarkdownFile {
    pub fn new(path: &str) -> Self {
        let md_file_content = fs::read_to_string(path).unwrap();
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: md_file_content.clone(),
            sections: vec![],
        };
        let sections = md_file.get_sections();
        md_file.sections = sections;
        md_file
    }

    pub fn new_from_path_and_content(path: &str, content: String) -> Self {
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: content.to_string(),
            sections: vec![],
        };

        let sections = md_file.get_sections();
        md_file.sections = sections;
        md_file
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.update_markdown().unwrap();
        fs::write(&self.path, &self.content).unwrap();
        Ok(())
    }

    fn get_sections(&self) -> Vec<MarkdownSection> {
        let headers: Vec<String> = self.get_headers();
        let sections: Vec<MarkdownSection> = headers
            .iter()
            .map(|header| {
                let new_section =
                    MarkdownSection::new_from_md_content_and_header(header, &self.content);
                new_section
            })
            .collect();
        sections
    }

    pub fn update_markdown(&mut self) -> Result<(), String> {
        let sections = self.get_sections();
        self.sections = sections;
        Ok(())
    }

    pub fn get_section_by_title(&mut self, title: &str) -> MarkdownSection {
        let section = self
            .sections
            .clone()
            .into_iter()
            .find(|section| section.borrow().title == title)
            .unwrap();
        section
    }

    fn get_headers(&self) -> Vec<String> {
        let headers: Vec<String> = self
            .content
            .lines()
            .filter(|line| line.trim().split(" ").next().unwrap().contains("#"))
            .map(|header| header.to_string())
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
}

#[derive(Debug, Clone)]
pub struct MarkdownSection {
    pub title: String,
    pub level: MarkdownSectionLevel,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl MarkdownSection {
    pub fn new(
        title: String,
        level: MarkdownSectionLevel,
        start_line_index: usize,
        end_line_index: usize,
    ) -> MarkdownSection {
        MarkdownSection {
            title,
            level,
            start_line_index,
            end_line_index,
        }
    }

    pub fn new_from_md_content_and_header(header: &str, md_content: &str) -> MarkdownSection {
        let mut header_split = header.clone().trim().split(" ");
        let prefix = header_split.next().unwrap().to_string();
        let title = header_split.collect::<Vec<&str>>().join(" ");
        let section_level = MarkdownSectionLevel::from_prefix(&prefix);
        let super_section_prefix = section_level.get_supersection_prefix();
        let prefixes = [&prefix, &super_section_prefix];
        let content_lines = md_content.split("\n");
        let last_line_index = content_lines.clone().count() - 1;
        let start_line_index = content_lines
            .clone()
            .position(|line| line == header)
            .unwrap();
        let end_line_index = content_lines
            .clone()
            .enumerate()
            .position(|line| {
                (prefixes
                    .iter()
                    .any(|pref| line.1.split(" ").next().unwrap() == pref.to_string())
                    && line.0 > start_line_index)
                    || line.0 == last_line_index
            })
            .unwrap();
        let level = MarkdownSectionLevel::from_prefix(&prefix);
        // let (start_line_index, end_line_index) = Self::get_section_indexes(content, header);
        MarkdownSection::new(title, level, start_line_index, end_line_index)
    }

    // pub fn get_section_indexes(content: &str, header: &str) -> (usize, usize) {
    //     let mut header_split = header.clone().trim().split(" ");
    //     let prefix = header_split.next().unwrap().to_string();
    //     let title = header_split.collect::<Vec<&str>>().join(" ");
    //     let section_level = MarkdownSectionLevel::from_prefix(&prefix);
    //     let super_section_prefix = section_level.get_supersection_prefix();
    //     let prefixes = [&prefix, &super_section_prefix];
    //     let content_lines = content.split("\n");
    //     let last_line_index = content_lines.clone().count() - 1;
    //     let start_line_index = content_lines
    //         .clone()
    //         .position(|line| line == header)
    //         .unwrap();
    //     let end_line_index = content_lines
    //         .clone()
    //         .enumerate()
    //         .position(|line| {
    //             (prefixes
    //                 .iter()
    //                 .any(|pref| line.1.split(" ").next().unwrap() == pref.to_string())
    //                 && line.0 > start_line_index)
    //                 || line.0 == last_line_index
    //         })
    //         .unwrap();
    //     (start_line_index, end_line_index)
    // }

    fn get_header(&self) -> String {
        self.level.get_header(&self.title)
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
    pub title: String,
    pub level: MarkdownSectionLevel,
    pub content: String,
    pub sections: Vec<TestMarkdownSection>,
    pub content_path: String,
}

impl TestMarkdownSection {
    pub fn new(
        ordinal_index: usize,
        parent_ordinal_index: usize,
        level: MarkdownSectionLevel,
        parent_path: String,
    ) -> Self {
        let title = Self::parse_section_title(ordinal_index, parent_ordinal_index, level);
        let content = format!("{}\n{} content", level.get_header(&title), title);
        TestMarkdownSection {
            title: title.clone(),
            level,
            content,
            sections: vec![],
            content_path: format!("{}{}{}", parent_path, PATH_SEPARATOR, title),
        }
    }
    // sections: [(2,3), (1,2)] -> first subsection got 2 subsubsections, and each got 3 sections_4
    pub fn generate_subsections(&mut self, parent_ordinal_index: usize, sections: (usize, usize)) {
        let (number_of_sections_level_2, number_of_sections_level_3) = sections;
        for index_of_section_level_2 in 0..number_of_sections_level_2 {
            let mut section_sub_2 = TestMarkdownSection::new(
                index_of_section_level_2,
                parent_ordinal_index,
                MarkdownSectionLevel::H2,
                self.content_path.clone(),
            );
            for index_of_section_level_3 in 0..number_of_sections_level_3 {
                let mut section_sub_3 = TestMarkdownSection::new(
                    index_of_section_level_3,
                    parent_ordinal_index,
                    MarkdownSectionLevel::H3,
                    section_sub_2.content_path.clone(),
                );
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

    pub fn parse_content(&mut self) {
        if !self.sections.is_empty() {
            let parsed_content = self.sections.iter().enumerate().fold(
                "".to_string(),
                |total, (section_index, section)| {
                    if section_index == 0 {
                        format!("{}", section.content)
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

    pub fn to_markdown_section(&self) -> MarkdownSection {
        let header = self.level.get_header(&self.title);
        MarkdownSection::new_from_md_content_and_header(&header, &self.content)
    }
}

#[derive(Debug)]
struct MarkdownTester {
    pub markdown_file: MarkdownFile,
    pub test_sections: Vec<TestMarkdownSection>,
}

impl MarkdownTester {
    pub fn new(path: &str, sections_generator: Vec<(usize, usize)>) -> Self {
        let mut test_sections: Vec<TestMarkdownSection> = vec![];
        for (section_index, section_generator) in sections_generator.iter().enumerate() {
            let mut test_section = TestMarkdownSection::new(
                section_index,
                0,
                MarkdownSectionLevel::H1,
                "".to_string(),
            );
            test_section.generate_subsections(section_index, *section_generator);
            test_sections.push(test_section);
        }
        let mut markdown_sections: Vec<MarkdownSection> = vec![];
        let mut markdown_content = String::new();
        for test_section in test_sections.clone() {
            let markdown_section = test_section.to_markdown_section();
            if markdown_content.is_empty() {
                markdown_content = test_section.content;
            } else {
                markdown_content = format!("{markdown_content}\n\n{}", test_section.content);
            }
            markdown_sections.push(markdown_section);
        }
        let sections = markdown_sections;
        let mut markdown_file = MarkdownFile {
            path: path.to_string(),
            content: markdown_content,
            sections,
        };
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
            test_section.title, expected_title,
            "Incorrect section level 1 title"
        );
        for (test_subsection_index, test_subsection) in test_section.sections.iter().enumerate() {
            let expected_title = TestMarkdownSection::parse_section_title(
                test_subsection_index,
                test_section_index,
                MarkdownSectionLevel::H2,
            );
            assert_eq!(
                test_subsection.title, expected_title,
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
                    test_subsubsection.title, expected_title,
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

// #[test]
// fn test_replace_section_by_path() {
//     let generator = vec![(2, 3), (1, 2), (1, 1)];
//     let path = "./test_md.md";
//     let MarkdownTester {
//         mut markdown_file,
//         test_sections,
//     } = MarkdownTester::new(path, generator);
//     let MarkdownFile { sections, .. } = markdown_file.clone();
//     let target_section = sections[0].subsections[1].clone();
//     // let replace_section =
//     let found_section = markdown_file.get_section_by_content_path(&target_section.content_path);
//     assert_eq!(
//         found_section.title, target_section.title,
//         "found_section don't match"
//     );
//     assert_eq!(
//         found_section.content_path, target_section.content_path,
//         "found_section don't match"
//     );
// }
