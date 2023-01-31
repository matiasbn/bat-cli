use std::fs;

use crate::{
    command_line::vs_code_open_file_in_current_window,
    markdown::MarkdownFile,
    silicon,
    utils::{
        self,
        path::{FilePathType, FolderPathType},
    },
};

use super::MetadataContent;

pub struct SourceCodeScreenshotOptions {
    pub include_path: bool,
    pub offset_to_start_line: bool,
    pub filter_comments: bool,
    pub font_size: Option<usize>,
    pub filters: Option<Vec<String>>,
    pub show_line_number: bool,
}

#[derive(Debug, Clone)]
pub struct SourceCodeMetadata {
    pub name: String,
    pub path: String,
    pub start_line_index: usize,
    pub end_line_index: usize,
}

impl SourceCodeMetadata {
    pub fn new(name: String, path: String, start_line_index: usize, end_line_index: usize) -> Self {
        SourceCodeMetadata {
            name,
            path,
            start_line_index,
            end_line_index,
        }
    }

    pub fn new_from_metadata_data(name: &str, section: &str, subsection: &str) -> Self {
        let metadata_path = utils::path::get_file_path(FilePathType::Metadata, true);
        let metadata_markdown = MarkdownFile::new(&metadata_path);
        let section = metadata_markdown.get_section_by_title(section);
        let subsection = section.get_subsection_by_title(subsection);
        let source_code_metadata = subsection.get_subsection_by_title(name);
        let path = Self::parse_metadata_info_section(
            &source_code_metadata.content,
            MetadataContent::Path.get_prefix(),
        );
        let start_line_index: usize = Self::parse_metadata_info_section(
            &source_code_metadata.content,
            MetadataContent::StartLineIndex.get_prefix(),
        )
        .parse()
        .unwrap();
        let end_line_index: usize = Self::parse_metadata_info_section(
            &source_code_metadata.content,
            MetadataContent::EndLineIndex.get_prefix(),
        )
        .parse()
        .unwrap();
        SourceCodeMetadata::new(name.to_string(), path, start_line_index, end_line_index)
    }

    pub fn create_screenshot(&self, options: SourceCodeScreenshotOptions) -> String {
        let dest_path = utils::path::get_folder_path(FolderPathType::AuditorFigures, true);
        let mut offset = if options.offset_to_start_line {
            self.start_line_index
        } else {
            0
        };
        let content = fs::read_to_string(&self.path).unwrap();
        let content_lines = content.lines().collect::<Vec<_>>()
            [self.start_line_index - 1..=self.end_line_index - 1]
            .to_vec();
        let content_vec = content_lines
            .iter()
            .map(|line| line.to_string())
            .filter(|line| {
                if options.filter_comments {
                    !line.contains("//")
                } else {
                    true
                }
            })
            .filter(|line| {
                if let Some(filters) = options.filters.clone() {
                    !filters.into_iter().any(|filter| line.contains(&filter))
                } else {
                    true
                }
            })
            .collect::<Vec<_>>()
            .to_vec();
        let mut content = content_vec.join("\n");
        if options.include_path {
            let program_path = utils::path::get_folder_path(FolderPathType::ProgramPath, false);
            let path_to_include = if self.path.contains(&program_path) {
                let path = self.path.replace(&program_path, "");
                path.strip_prefix("/").unwrap().to_string()
            } else {
                self.path.to_string()
            };
            content = format!("// {}\n\n{}", path_to_include, content);
            offset = if options.offset_to_start_line {
                offset + 2
            } else {
                0
            };
        }

        let png_screenshot_path = silicon::create_figure(
            &content,
            &dest_path,
            &self.name,
            offset,
            options.font_size,
            options.show_line_number,
        );
        png_screenshot_path
    }

    fn parse_metadata_info_section(metadata_info_content: &str, section: &str) -> String {
        let path = metadata_info_content
            .lines()
            .find(|line| line.contains(section))
            .unwrap()
            .replace(section, "")
            .trim()
            .to_string();
        path
    }
}
