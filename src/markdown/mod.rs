use std::{fs, io};

#[derive(Debug, Clone)]
pub struct MardkownFile {
    path: String,
    content: String,
    sections: Vec<MarkdownSection>,
}

impl MarkdownParser for MardkownFile {}

impl MardkownFile {
    pub fn new(path: &str) -> Self {
        let md_file_content = fs::read_to_string(path).unwrap();
        let sections = Self::get_sections(md_file_content.clone(), MarkdownSectionLevel::H1);
        MardkownFile {
            path: path.to_string(),
            content: md_file_content,
            sections,
        }
    }

    pub fn replace_section(
        &mut self,
        old_section: MarkdownSection,
        new_section: MarkdownSection,
    ) -> Result<(), io::Error> {
        if old_section.level != new_section.level {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Can't replace section of different levels",
            ));
        }
        let old_section_index = self
            .sections
            .clone()
            .into_iter()
            .position(|section| section == old_section)
            .unwrap();
        self.sections[old_section_index] = new_section;
        let sections_content: Vec<String> = self
            .sections
            .iter()
            .map(|section| section.content.clone())
            .collect();
        self.content = sections_content.join("\n");
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct MarkdownSection {
    title: String,
    content: String,
    level: MarkdownSectionLevel,
    subsections: Vec<MarkdownSection>,
}

impl MarkdownParser for MarkdownSection {}

impl MarkdownSection {
    fn new(title: String, level: MarkdownSectionLevel, section_content: &str) -> MarkdownSection {
        let subsections =
            Self::get_sections(section_content.to_string(), level.get_subsection_level());
        MarkdownSection {
            title,
            content: section_content.to_string(),
            level,
            subsections,
        }
    }

    fn new_from_content(section_content: &str) -> MarkdownSection {
        let header = section_content.trim().lines().next().unwrap();
        let section_prefix = header.split(" ").next().unwrap();
        let title = header
            .strip_prefix(&format!("{} ", section_prefix))
            .unwrap()
            .trim()
            .to_string();
        let level = MarkdownSectionLevel::from_prefix(section_prefix);
        MarkdownSection::new(title, level, section_content)
    }

    fn get_header(&self) -> String {
        self.level.get_header(&self.title)
    }

    pub fn replace_subsection(
        &mut self,
        old_subsection: MarkdownSection,
        new_subsection: MarkdownSection,
    ) -> Result<(), io::Error> {
        if old_subsection.level != new_subsection.level {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Can't replace subsection of different levels",
            ));
        }
        let old_subsection_index = self
            .subsections
            .clone()
            .into_iter()
            .position(|subsection| subsection == old_subsection)
            .unwrap();
        self.subsections[old_subsection_index] = new_subsection;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum MarkdownSectionLevel {
    H1,
    H2,
    H3,
    H4,
}

impl MarkdownSectionLevel {
    pub fn get_subsection_level(&self) -> MarkdownSectionLevel {
        match self {
            MarkdownSectionLevel::H1 => MarkdownSectionLevel::H2,
            MarkdownSectionLevel::H2 => MarkdownSectionLevel::H3,
            MarkdownSectionLevel::H3 => MarkdownSectionLevel::H4,
            MarkdownSectionLevel::H4 => unimplemented!(),
        }
    }

    pub fn get_prefix(&self) -> String {
        match self {
            MarkdownSectionLevel::H1 => "#".to_string(),
            MarkdownSectionLevel::H2 => "##".to_string(),
            MarkdownSectionLevel::H3 => "###".to_string(),
            MarkdownSectionLevel::H4 => "####".to_string(),
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
        }
    }

    fn get_header(&self, title: &str) -> String {
        format!("{} {}", self.get_prefix(), title)
    }

    fn get_subsection_header(&self, title: &str) -> String {
        format!("{} {}", self.get_subsection_prefix(), title)
    }
}

pub trait MarkdownParser {
    fn get_section_content(title: &str, level: MarkdownSectionLevel, content: &str) -> String {
        let header = level.get_header(title);
        let prefix = level.get_prefix();
        let last_line_index = content.lines().count() - 1;
        let start_index = content
            .trim()
            .lines()
            .position(|line| line.trim() == header)
            .unwrap();
        let mut end_index = content
            .trim()
            .lines()
            .enumerate()
            .position(|line| {
                (line.1.trim().split(" ").next().unwrap() == prefix && line.0 > start_index)
                    || line.0 == last_line_index
            })
            .unwrap();
        if end_index == last_line_index {
            end_index += 1
        }
        let section: String = content.lines().collect::<Vec<&str>>()[start_index..end_index]
            .to_vec()
            .join("\n");
        section
    }

    fn get_headers_from_section_level(content: &str, level: MarkdownSectionLevel) -> Vec<String> {
        let level_prefix = level.get_prefix();
        let headers: Vec<String> = content
            .lines()
            .filter(|line| line.trim().split(" ").next().unwrap() == level_prefix)
            .map(|header| header.to_string())
            .collect();
        headers
    }

    fn get_sections(content: String, level: MarkdownSectionLevel) -> Vec<MarkdownSection> {
        if !content.contains(&level.get_prefix()) {
            return vec![];
        };
        let headers: Vec<String> = Self::get_headers_from_section_level(&content, level);
        headers
            .into_iter()
            .map(|header| {
                let title = header
                    .clone()
                    .trim()
                    .strip_prefix(&format!("{} ", level.get_prefix()))
                    .unwrap()
                    .trim()
                    .to_string();
                let content = MarkdownSection::get_section_content(&title, level.clone(), &content);
                MarkdownSection::new(title, level.clone(), &content)
            })
            .collect()
    }
}

const TEST_FIRST_SECTION_CONTENT: &str = "# First section

First section content

## First subsection

First subsection content";

const TEST_SECOND_SECTION_CONTENT: &str = "# Second section

Second section content

## Second subsection

Second subsection content

### Second subsubsection

Second subsubsection content

more content";

const TEST_THIRD_SECTION_CONTENT: &str = "# Third section

Third section content

## Third subsection

Third subsection content";

const TEST_SUBSECTION_CONTENT: &str = "## Third subsection

Third subsection content

### Third subsubsection

Third subsubsection content";

#[test]
fn test_new_markdown_file() {
    let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}",);
    let path = "./test_md.md";
    fs::write(path, &md_file_content).unwrap();

    let markdown = MardkownFile::new("./test_md.md");

    fs::remove_file(path).unwrap();

    let sections = markdown.sections;
    let first_section = &sections[0];
    let second_section = &sections[1];
    assert_eq!(first_section.title, "First section");
    assert_eq!(second_section.title, "Second section");

    assert_eq!(first_section.content, TEST_FIRST_SECTION_CONTENT);
    assert_eq!(second_section.content, TEST_SECOND_SECTION_CONTENT);
}

#[test]
fn test_replace_section() {
    let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}");
    let path = "./test_md.md";
    fs::write(path, &md_file_content).unwrap();

    let mut markdown = MardkownFile::new("./test_md.md");

    fs::remove_file(path).unwrap();
    let new_section = MarkdownSection::new_from_content(TEST_THIRD_SECTION_CONTENT);
    let sections = markdown.sections.clone();

    markdown
        .replace_section(sections[0].clone(), new_section)
        .unwrap();
    let first_section = markdown.sections[0].clone();
    let second_section = markdown.sections[1].clone();
    assert_eq!(first_section.title, "Third section");
    assert_eq!(second_section.title, "Second section");

    assert_eq!(first_section.content, TEST_THIRD_SECTION_CONTENT);
    assert_eq!(second_section.content, TEST_SECOND_SECTION_CONTENT);

    let expected_content = format!("{TEST_THIRD_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}");
    assert_eq!(markdown.content, expected_content);
}
