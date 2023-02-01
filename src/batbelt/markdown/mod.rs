use std::{fs, io};

#[derive(Debug, Clone)]
pub struct MarkdownFile {
    pub path: String,
    pub content: String,
    pub sections: Vec<MarkdownSection>,
}

impl MarkdownParser for MarkdownFile {}

impl MarkdownFile {
    pub fn new(path: &str) -> Self {
        let md_file_content = fs::read_to_string(path).unwrap();
        let sections = Self::get_sections(md_file_content.clone(), MarkdownSectionLevel::H1);
        MarkdownFile {
            path: path.to_string(),
            content: md_file_content,
            sections,
        }
    }

    pub fn new_from_path_and_content(path: &str, content: &str) -> Self {
        let sections = Self::get_sections(content.to_string(), MarkdownSectionLevel::H1);
        MarkdownFile {
            path: path.to_string(),
            content: content.to_string(),
            sections,
        }
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.update_content().unwrap();
        fs::write(&self.path, &self.content).unwrap();
        Ok(())
    }

    pub fn update_content(&mut self) -> Result<(), String> {
        let sections_content: Vec<String> = self
            .sections
            .iter()
            .map(|section| section.content.clone())
            .collect();
        self.content = sections_content.join("\n");
        Ok(())
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
        self.update_content().unwrap();
        Ok(())
    }

    pub fn get_section_by_title(&self, title: &str) -> MarkdownSection {
        self.sections
            .clone()
            .into_iter()
            .find(|section| section.title == title)
            .unwrap()
    }

    pub fn get_subsections(&self) -> Vec<MarkdownSection> {
        let mut subsections: Vec<MarkdownSection> = vec![];
        let _thing = self
            .sections
            .iter()
            .map(|section| subsections.append(&mut section.get_children_subsections().unwrap()));
        subsections
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct MarkdownSection {
    pub title: String,
    pub content: String,
    pub level: MarkdownSectionLevel,
    pub subsections: Vec<MarkdownSection>,
}

impl MarkdownParser for MarkdownSection {}

impl MarkdownSection {
    pub fn new(
        title: String,
        level: MarkdownSectionLevel,
        section_content: &str,
    ) -> MarkdownSection {
        let subsections =
            Self::get_sections(section_content.to_string(), level.get_subsection_level());
        MarkdownSection {
            title,
            content: section_content.to_string(),
            level,
            subsections,
        }
    }
    pub fn update_content(&mut self) -> Result<(), String> {
        let subsections_content: Vec<String> = self
            .subsections
            .iter()
            .map(|section| section.content.clone())
            .collect();
        let content = subsections_content.join("\n");
        self.content = if content.is_empty() {
            self.get_header()
        } else {
            format!("{}\n\n{}", self.get_header(), content)
        };
        Ok(())
    }

    pub fn new_from_content(section_content: &str) -> MarkdownSection {
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

    pub fn new_from_subsections(
        title: &str,
        level: MarkdownSectionLevel,
        subsections: Vec<MarkdownSection>,
    ) -> MarkdownSection {
        let header = level.get_header(title);
        let section_content: String = subsections
            .clone()
            .into_iter()
            .map(|subsection| subsection.content)
            .collect::<Vec<_>>()
            .join("\n");
        let section_content = if subsections.clone().is_empty() {
            format!("{}\n", header)
        } else {
            format!("{}\n\n{}", header, section_content)
        };
        MarkdownSection::new(title.to_string(), level, &section_content)
    }

    pub fn update_subsection_subsections_by_title(
        &mut self,
        title: &str,
        subsections: Vec<MarkdownSection>,
    ) -> Result<(), String> {
        let new_subsection = MarkdownSection::new_from_subsections(
            title,
            self.level.get_subsection_level(),
            subsections.clone(),
        );
        self.update_subsection_by_title(title, new_subsection)
            .unwrap();
        Ok(())
    }

    fn get_header(&self) -> String {
        self.level.get_header(&self.title)
    }

    pub fn get_children_subsections(&self) -> Result<Vec<MarkdownSection>, io::Error> {
        let mut children_subsections_vec: Vec<MarkdownSection> = vec![];
        let _children_subsections = self.subsections.clone().into_iter().map(|subsection| {
            children_subsections_vec
                .append(&mut subsection.clone().get_children_subsections().unwrap())
        });
        Ok(children_subsections_vec.clone())
    }

    pub fn update_subsection_by_title(
        &mut self,
        old_subsection_title: &str,
        new_subsection: MarkdownSection,
    ) -> Result<(), io::Error> {
        let old_subsection = self.get_subsection_by_title(old_subsection_title);
        self.replace_subsection(old_subsection.clone(), new_subsection)
            .unwrap();
        Ok(())
    }

    fn replace_subsection(
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
        self.update_content().unwrap();
        Ok(())
    }
    pub fn get_subsection_by_title(&self, title: &str) -> &MarkdownSection {
        self.subsections
            .iter()
            .find(|section| section.title == title)
            .unwrap()
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
        format!("{} {}", self.get_prefix(), title)
    }

    pub fn get_subsection_header(&self, title: &str) -> String {
        format!("{} {}", self.get_subsection_prefix(), title)
    }
}

pub trait MarkdownParser {
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
        let mut end_index = content_lines
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

    fn get_headers_from_section_level(content: &str, level: MarkdownSectionLevel) -> Vec<String> {
        let level_prefix = level.get_prefix();
        let headers: Vec<String> = content
            .lines()
            .filter(|line| line.trim().split(" ").next().unwrap() == level_prefix)
            .map(|header| header.to_string())
            .collect();
        println!("headers {:#?}", headers);
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
                println!("title {}", title);
                let content =
                    MarkdownSection::get_section_content(&header, level.clone(), &content);
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

const TEST_SUBSUBSECTION_CONTENT: &str = "### Replace subsubsection

Replace subsubsection content";

#[test]
fn test_new_markdown_file() {
    let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}",);
    let path = "./test_md.md";
    fs::write(path, &md_file_content).unwrap();

    let markdown = MarkdownFile::new("./test_md.md");

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

    let mut markdown = MarkdownFile::new("./test_md.md");

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

#[test]
fn test_get_subsections() {
    let md_file_content = format!("{TEST_FIRST_SECTION_CONTENT}\n{TEST_SECOND_SECTION_CONTENT}");
    let path = "./test_md.md";
    fs::write(path, &md_file_content).unwrap();

    let markdown = MarkdownFile::new("./test_md.md");

    fs::remove_file(path).unwrap();
    let subsections = markdown.get_subsections();
    println!("sub {:#?}", subsections);
}
