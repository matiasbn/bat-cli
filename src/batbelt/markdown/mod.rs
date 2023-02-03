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
        let section: String = content_lines.collect::<Vec<&str>>()[start_index..end_index]
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

#[derive(Clone)]
struct TestMarkdownSection {
    pub title: String,
    pub level: MarkdownSectionLevel,
    pub content: String,
}

impl TestMarkdownSection {
    pub fn new(ordinal_index: i64, level: MarkdownSectionLevel) -> Self {
        let title = Self::content_parser(ordinal_index, level);
        let content = format!("{}\n\n{} content", level.get_header(&title), title);
        TestMarkdownSection {
            title,
            level,
            content,
        }
    }

    fn content_parser(ordinal_index: i64, level: MarkdownSectionLevel) -> String {
        let ordinal = match ordinal_index {
            0 => "First",
            1 => "Second",
            2 => "First",
            3 => "Fourth",
            _ => unimplemented!(),
        };

        match level {
            MarkdownSectionLevel::H1 => format!("{} section", ordinal),
            MarkdownSectionLevel::H2 => format!("{} subsection", ordinal),
            MarkdownSectionLevel::H3 => format!("{} subsubsection", ordinal),
            MarkdownSectionLevel::H4 => format!("{} subsubsubsection", ordinal),
            _ => unimplemented!(),
        }
    }

    pub fn to_markdown_section(&self, content: String) -> MarkdownSection {
        MarkdownSection::new(self.title.clone(), self.level, content.to_string())
    }
}

#[derive(Debug)]
struct MarkdownTester {
    pub first_section: MarkdownSection,
    pub second_section: MarkdownSection,
    pub third_section: MarkdownSection,
}

impl MarkdownTester {
    pub fn new() -> Self {
        // First section
        let first_section_tester = TestMarkdownSection::new(0, MarkdownSectionLevel::H1);
        let first_subsection_tester = TestMarkdownSection::new(0, MarkdownSectionLevel::H2);
        let first_subsubsection_tester = TestMarkdownSection::new(0, MarkdownSectionLevel::H3);
        let first_section_content = Self::content_parser(vec![
            first_section_tester.clone(),
            first_subsection_tester,
            first_subsubsection_tester,
        ]);
        let first_section = first_section_tester.to_markdown_section(first_section_content);
        // Second section
        let second_section_tester = TestMarkdownSection::new(1, MarkdownSectionLevel::H1);
        let second_subsection_tester = TestMarkdownSection::new(1, MarkdownSectionLevel::H2);
        let second_subsubsection_tester = TestMarkdownSection::new(1, MarkdownSectionLevel::H3);
        let second_section_content = Self::content_parser(vec![
            second_section_tester.clone(),
            second_subsection_tester,
            second_subsubsection_tester,
        ]);
        let second_section = second_section_tester.to_markdown_section(second_section_content);

        // Third section
        let third_section_tester = TestMarkdownSection::new(2, MarkdownSectionLevel::H1);
        let third_subsection_tester = TestMarkdownSection::new(2, MarkdownSectionLevel::H2);
        let third_subsubsection_tester = TestMarkdownSection::new(2, MarkdownSectionLevel::H3);
        let third_section_content = Self::content_parser(vec![
            third_section_tester.clone(),
            third_subsection_tester,
            third_subsubsection_tester,
        ]);
        let third_section = third_section_tester.to_markdown_section(third_section_content);
        MarkdownTester {
            first_section,
            second_section,
            third_section,
        }
    }

    fn content_parser(sections: Vec<TestMarkdownSection>) -> String {
        let content = sections.iter().fold("".to_string(), |total, section| {
            format!("{}\n\n{}", total, section.content)
        });
        content
    }
}

#[test]
fn test_markdown_tester() {
    let markdown_tester = MarkdownTester::new();
    println!("tester {:#?}", markdown_tester);
    // let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}",);
    // let path = "./test_md.md";
    // fs::write(path, &md_file_content).unwrap();

    // let markdown = MarkdownFile::new("./test_md.md");

    // fs::remove_file(path).unwrap();

    // let sections = markdown.sections.borrow();
    // let first_section = &sections[0];
    // let second_section = &sections[1];
    // assert_eq!(first_section.borrow().title, "First section");
    // assert_eq!(second_section.borrow().title, "Second section");

    // assert_eq!(first_section.borrow().content, TEST_FIRST_SECTION_CONTENT);
    // assert_eq!(second_section.borrow().content, TEST_SECOND_SECTION_CONTENT);
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
