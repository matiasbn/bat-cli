use silicon::assets::HighlightingAssets;
use silicon::formatter::ImageFormatterBuilder;
use silicon::utils::{Background, ShadowAdder};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

use std::fs;

/// Dracula background color.
const BG: image::Rgba<u8> = image::Rgba([0x28, 0x2a, 0x36, 0xff]);

/// Default font size when none is specified.
const DEFAULT_FONT_SIZE: f32 = 20.0;

/// Horizontal and vertical padding around the code image.
const PAD: u32 = 10;

pub fn create_figure(
    content: &str,
    dest_folder_path: &str,
    file_name: &str,
    offset: usize,
    font_size: Option<usize>,
    show_line_number: bool,
) -> String {
    let dest_png_path = format!("{dest_folder_path}/{file_name}.png");

    let size = font_size.map(|s| s as f32).unwrap_or(DEFAULT_FONT_SIZE);

    // Load syntax definitions and themes bundled with silicon/syntect.
    let ha = HighlightingAssets::new();
    let (ps, ts) = (ha.syntax_set, ha.theme_set);

    let theme = &ts.themes["Dracula"];

    // Syntax-highlight every line.
    let syntax = ps
        .find_syntax_by_extension("rs")
        .expect("Rust syntax not found in syntect");
    let mut highlighter = HighlightLines::new(syntax, theme);
    let highlight: Vec<Vec<(syntect::highlighting::Style, &str)>> = LinesWithEndings::from(content)
        .map(|line| highlighter.highlight_line(line, &ps).unwrap())
        .collect();

    // Configure background + padding (no shadow).
    let shadow = ShadowAdder::default()
        .background(Background::Solid(BG))
        .shadow_color(image::Rgba([0, 0, 0, 0]))
        .blur_radius(0.0)
        .pad_horiz(PAD)
        .pad_vert(PAD)
        .offset_x(0)
        .offset_y(0);

    // Build the image formatter.
    let mut formatter = ImageFormatterBuilder::new()
        .font(vec![("Hack".to_string(), size)])
        .line_number(show_line_number)
        .line_offset(offset as u32)
        .tab_width(4)
        .window_controls(false)
        .round_corner(false)
        .shadow_adder(shadow)
        .build()
        .expect("Failed to build silicon ImageFormatter");

    let image = formatter.format(&highlight, theme);

    image
        .save(&dest_png_path)
        .expect("Failed to save screenshot PNG");

    dest_png_path
}

pub fn delete_png_file(path: String) {
    fs::remove_file(path).unwrap();
}

/// No longer needed — silicon is now a library dependency.
/// Kept for backwards compatibility; always returns true.
pub fn check_silicon_installed() -> bool {
    true
}
