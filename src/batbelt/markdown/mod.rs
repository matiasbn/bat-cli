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
    pub sections: RefCell<Vec<RefCell<MarkdownSection>>>,
}

impl MarkdownFile {
    pub fn new(path: &str) -> Self {
        let md_file_content = fs::read_to_string(path).unwrap();
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: md_file_content.clone(),
            sections: RefCell::new(vec![]),
        };
        let sections = md_file.get_sections();
        md_file.sections = RefCell::new(sections);
        md_file
    }

    pub fn new_from_path_and_content(path: &str, content: String) -> Self {
        let mut md_file = MarkdownFile {
            path: path.to_string(),
            content: content.to_string(),
            sections: RefCell::new(vec![]),
        };

        let sections = md_file.get_sections();
        md_file.sections = RefCell::new(sections);
        md_file
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.update_markdown().unwrap();
        fs::write(&self.path, &self.content).unwrap();
        Ok(())
    }

    fn get_sections(&self) -> Vec<RefCell<MarkdownSection>> {
        let level = MarkdownSectionLevel::H1;
        let headers: Vec<String> = get_headers_from_section_level(&self.content, level);
        let sections_content: Vec<(String, String)> = headers
            .iter()
            .map(|header| {
                let title = header
                    .clone()
                    .strip_prefix(&format!("{} ", level.get_prefix()))
                    .unwrap()
                    .to_string();
                let content = MarkdownSection::get_section_content(&header, level, &self.content);
                (title, content)
            })
            .collect();
        let md_sections: Vec<RefCell<MarkdownSection>> = sections_content
            .into_iter()
            .map(|(section_title, section_content)| {
                let new_section =
                    MarkdownSection::new(section_title, MarkdownSectionLevel::H1, section_content);
                RefCell::new(new_section)
            })
            .collect();
        md_sections
    }

    pub fn update_markdown(&mut self) -> Result<(), String> {
        let sections = self.get_sections();
        self.sections = RefCell::new(sections);
        Ok(())
    }

    pub fn get_section_by_title(&self, title: &str) -> RefCell<MarkdownSection> {
        let section = self
            .sections
            .take()
            .into_iter()
            .find(|section| section.borrow().title == title)
            .unwrap();
        section
    }
}

// impl Deref for MarkdownSection {
//     type Target = MarkdownSection;

//     fn deref(&self) -> &Self::Target {
//         self
//     }
// }

#[derive(Debug, Clone)]
pub struct MarkdownSection {
    pub title: String,
    pub content: String,
    pub level: MarkdownSectionLevel,
    pub subsections: RefCell<Vec<RefCell<MarkdownSection>>>,
}

impl MarkdownSection {
    pub fn new(
        title: String,
        level: MarkdownSectionLevel,
        section_content: String,
    ) -> MarkdownSection {
        let mut new_section = MarkdownSection {
            title,
            content: section_content.to_string(),
            level,
            subsections: RefCell::new(vec![]),
        };
        new_section.get_subsections();
        new_section
    }

    fn get_subsections(&mut self) {
        let subsection_level = self.level.get_subsection_level();
        let headers: Vec<String> = get_headers_from_section_level(&self.content, subsection_level);
        let subsections: Vec<RefCell<MarkdownSection>> = headers
            .iter()
            .map(|header| {
                let title = header
                    .clone()
                    .strip_prefix(&format!("{} ", subsection_level.get_prefix()))
                    .unwrap()
                    .to_string();
                let content =
                    MarkdownSection::get_section_content(&header, subsection_level, &self.content);
                let new_section = MarkdownSection::new(title, subsection_level, content.clone());
                RefCell::new(new_section)
            })
            .collect();
        self.subsections = RefCell::new(subsections);
    }

    pub fn update_section_content(&mut self, new_section_content: &str) {
        let insert_content = new_section_content.replace(&self.get_header(), "");
        let formatted_content = format!("{}\n\n{}", self.get_header(), insert_content.trim_start());
        self.content = formatted_content;
    }

    fn get_header(&self) -> String {
        self.level.get_header(&self.title)
    }

    pub fn get_subsection_by_title(&self, title: &str) -> RefCell<MarkdownSection> {
        let subsection = self
            .subsections
            .take()
            .into_iter()
            .find(|section| section.borrow().title == title)
            .unwrap();
        subsection
    }

    fn get_section_content(header: &str, level: MarkdownSectionLevel, content: &str) -> String {
        let prefix = level.get_prefix();
        let super_section_prefix = level.get_supersection_prefix();
        let prefixes = [&prefix, &super_section_prefix];
        let content_lines = content.split("\n");
        let last_line_index = content_lines.clone().count() - 1;
        let start_index = content_lines
            .clone()
            .position(|line| line == header)
            .unwrap();
        let end_index = content_lines
            .clone()
            .enumerate()
            .position(|line| {
                (prefixes
                    .iter()
                    .any(|pref| line.1.split(" ").next().unwrap() == pref.to_string())
                    && line.0 > start_index)
                    || line.0 == last_line_index
            })
            .unwrap();
        let section: String = content_lines.collect::<Vec<&str>>()[start_index..=end_index]
            .to_vec()
            .join("\n");
        section
    }
}

fn get_headers_from_section_level(content: &str, level: MarkdownSectionLevel) -> Vec<String> {
    let level_prefix = level.get_prefix();
    let headers: Vec<String> = content
        .lines()
        .filter(|line| line.split(" ").next().unwrap() == level_prefix)
        .map(|header| header.to_string())
        .collect();
    headers
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
}

impl TestMarkdownSection {
    pub fn new(
        ordinal_index: usize,
        parent_ordinal_index: usize,
        level: MarkdownSectionLevel,
    ) -> Self {
        let title = Self::get_section_title(ordinal_index, parent_ordinal_index, level);
        let content = format!("{}\n{} content", level.get_header(&title), title);
        TestMarkdownSection {
            title,
            level,
            content,
            sections: vec![],
        }
    }
    // sections: [(2,3), (1,2)] -> first subsection got 2 subsections, and each got 3 subsubsections
    pub fn generate_subsections(
        &mut self,
        parent_ordinal_index: usize,
        sections: Vec<(usize, usize)>,
    ) {
        for (number_of_subsections, number_of_subsubsections) in sections {
            for subsection_ordinal in 0..number_of_subsections {
                let mut subsection = TestMarkdownSection::new(
                    subsection_ordinal,
                    parent_ordinal_index,
                    MarkdownSectionLevel::H2,
                );
                for subsubsection_ordinal in 0..number_of_subsubsections {
                    let mut subsubsection = TestMarkdownSection::new(
                        subsubsection_ordinal,
                        parent_ordinal_index,
                        MarkdownSectionLevel::H3,
                    );
                    subsubsection.parse_content();
                    subsection.sections.push(subsubsection);
                }
                subsection.parse_content();
                self.sections.push(subsection);
            }
            self.parse_content();
        }
    }

    pub fn get_section_title(
        ordinal_index: usize,
        parent_ordinal_index: usize,
        level: MarkdownSectionLevel,
    ) -> String {
        let ordinal = match ordinal_index {
            0 => "First",
            1 => "Second",
            2 => "Third",
            3 => "Fourth",
            _ => unimplemented!(),
        };

        match level {
            MarkdownSectionLevel::H1 => format!("{} section", ordinal),
            MarkdownSectionLevel::H2 => {
                format!("{} subsection / parent_{}", ordinal, parent_ordinal_index)
            }
            MarkdownSectionLevel::H3 => {
                format!(
                    "{} subsubsection / parent_{}",
                    ordinal, parent_ordinal_index
                )
            }
            MarkdownSectionLevel::H4 => {
                format!(
                    "{} subsubsubsection / parent_{}",
                    ordinal, parent_ordinal_index
                )
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

    pub fn to_markdown_section(&self) -> MarkdownSection {
        MarkdownSection::new(self.title.clone(), self.level, self.content.clone())
    }
}

#[derive(Debug)]
struct MarkdownTester {
    pub markdown_file: MarkdownFile,
    pub first_test_section: TestMarkdownSection,
    pub second_test_section: TestMarkdownSection,
    pub third_test_section: TestMarkdownSection,
}

impl MarkdownTester {
    pub fn new(path: &str, subsections_generator: [Vec<(usize, usize)>; 3]) -> Self {
        let mut first_test_section = TestMarkdownSection::new(0, 0, MarkdownSectionLevel::H1);
        first_test_section.generate_subsections(0, subsections_generator[0].clone());
        let mut second_test_section = TestMarkdownSection::new(1, 0, MarkdownSectionLevel::H1);
        second_test_section.generate_subsections(0, subsections_generator[1].clone());
        let mut third_test_section = TestMarkdownSection::new(2, 0, MarkdownSectionLevel::H1);
        third_test_section.generate_subsections(0, subsections_generator[2].clone());
        let first_section = RefCell::new(first_test_section.to_markdown_section());
        let second_section = RefCell::new(second_test_section.to_markdown_section());
        let third_section = RefCell::new(third_test_section.to_markdown_section());
        let markdown_content = format!(
            "{}\n\n{}\n\n{}",
            first_test_section.content, second_test_section.content, third_test_section.content
        );
        let sections = RefCell::new(vec![first_section, second_section, third_section]);
        let markdown_file = MarkdownFile {
            path: path.to_string(),
            content: markdown_content,
            sections: sections,
        };
        MarkdownTester {
            markdown_file,
            first_test_section: first_test_section,
            second_test_section: second_test_section,
            third_test_section: third_test_section,
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
    let generator = [vec![(2, 2)], vec![(0, 0)], vec![(0, 0)]];
    let markdown_tester = MarkdownTester::new(".", generator.clone());
    assert_eq!(
        TestMarkdownSection::get_section_title(0, 0, MarkdownSectionLevel::H1),
        "First section",
        "Incorrect title"
    );
    assert_eq!(
        TestMarkdownSection::get_section_title(0, 0, MarkdownSectionLevel::H2),
        "First subsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.title, "First section",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[0].title,
        "First subsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[0].sections[0].title,
        "First subsubsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[0].sections[1].title,
        "Second subsubsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[1].title,
        "Second subsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[1].sections[0].title,
        "First subsubsection / parent_0",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[1].sections[1].title,
        "Second subsubsection / parent_0",
        "Incorrect title"
    );
    // Sections length
    assert_eq!(
        markdown_tester.first_test_section.clone().sections.len(),
        generator[0].clone()[0].0,
        "Incorrect length"
    );
    // Subsections length
    assert_eq!(
        markdown_tester.first_test_section.clone().sections[0]
            .sections
            .len(),
        generator[0].clone()[0].1,
        "Incorrect length"
    );

    assert_eq!(
        markdown_tester.second_test_section.title, "Second section",
        "Incorrect title"
    );
    assert_eq!(
        markdown_tester.third_test_section.title, "Third section",
        "Incorrect title"
    );
}

fn test_markdown_tester() {
    let test_path = "./test_md.md";
    let first_generator = vec![(1, 1), (2, 1)];
    let second_generator = vec![(1, 1)];
    let third_generator = vec![(1, 1)];
    let markdown_tester = MarkdownTester::new(
        test_path,
        [
            first_generator.clone(),
            second_generator.clone(),
            third_generator.clone(),
        ],
    );
    let MarkdownTester {
        markdown_file,
        first_test_section,
        second_test_section,
        third_test_section,
    } = markdown_tester;
    println!(
        "first section content\n\n{}",
        first_test_section.content.clone()
    );
    assert_eq!(
        first_test_section.sections.len(),
        first_generator.len(),
        "incorrect first generator"
    );
    for (number_of_subsection, number_of_subsubsection) in first_generator {
        assert_eq!(
            first_test_section.sections.clone()[number_of_subsection]
                .sections
                .len(),
            number_of_subsubsection,
            "incorrect number of subsubsections for first"
        )
    }
    assert_eq!(
        second_test_section.sections.len(),
        second_generator.len(),
        "incorrect second generator"
    );
    assert_eq!(
        third_test_section.sections.len(),
        third_generator.len(),
        "incorrect third generator"
    );
    // println!(
    //     "first section content\n\n{}",
    //     markdown_tester.first_test_section.content
    // );
}

// #[test]
// fn test_update_section_content() {
//     let path = "./test_md.md";
//     fs::write(path, TEST_FIRST_SECTION_CONTENT).unwrap();

//     let mut markdown = MarkdownFile::new(path);

//     fs::remove_file(path).unwrap();

//     println!("markdown \n{:#?}", markdown);
//     println!("markdown content\n{:#?}", markdown.content);
// }

// // #[test]
// // fn test_update_section_content() {
// //     let path = "./test_md.md";
// //     fs::write(path, TEST_FIRST_SECTION_CONTENT).unwrap();

// //     let mut markdown = MarkdownFile::new(path);

// //     fs::remove_file(path).unwrap();

// //     println!("markdown \n{:#?}", markdown);
// //     println!("markdown content\n{:#?}", markdown.content);
// // }

// #[test]
// fn test_get_subsections() {
//     let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}");
//     let path = "./test_md.md";
//     fs::write(path, &md_file_content).unwrap();

//     let mut markdown = MarkdownFile::new("./test_md.md");

//     println!("markdown \n {:#?}", markdown);
//     println!("markdown content \n {}", markdown.content);

//     let first_section = markdown.borrow().get_section_by_title("First section");

//     // println!("first_section \n {:#?}", first_section);

//     // let first_section_parent = &first_section.parent.borrow().upgrade();

//     // println!("first_section_parent \n {:#?}", first_section_parent);

//     let first_subsection = first_section
//         .borrow()
//         .get_subsection_by_title("First subsection");
//     println!("first_subsection \n {:#?}", first_subsection);

//     let replace_subsection_content = "## Replace subsection \n\n Replace subsection content";

//     first_subsection
//         .borrow_mut()
//         .update_section_content(replace_subsection_content);

//     markdown.borrow_mut().update_markdown().unwrap();
//     println!("markdown replaced \n {:#?}", markdown);
//     println!("markdown replaced content \n {}", markdown.content);

//     let first_section_replaced = markdown.get_section_by_title("First section");

//     println!("first_section_replaced \n {:#?}", first_section_replaced);

//     let replace_subsection = first_section_replaced
//         .borrow()
//         .get_subsection_by_title("First subsection");

//     println!("replace_subsection {:#?}", replace_subsection);
//     // let first_subsection_parent = &first_subsection.parent.borrow().upgrade();

//     // println!("first_subsection_parent {:#?}", first_subsection_parent);

//     // let subsection_context
//     fs::remove_file(path).unwrap();
// }
