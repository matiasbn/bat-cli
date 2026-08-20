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

/// silicon's `ImageFormatter::line_pad` default (see `ImageFormatterBuilder`).
const LINE_PAD: u32 = 2;

/// silicon's `ImageFormatter::code_pad`, the gap between the image border and
/// the first line of code.
const CODE_PAD: u32 = 25;

/// silicon's `ImageFormatter::line_number_pad`, applied on both sides of the
/// line number gutter.
const LINE_NUMBER_PAD: u32 = 6;

/// Tab width passed to the `ImageFormatterBuilder` in [`create_figure`].
const TAB_WIDTH: usize = 4;

/// Vertical geometry of a rendered screenshot, in PNG pixels.
///
/// silicon draws line `i` (0-based) at `y = i * line_height + code_pad + code_pad_top`
/// (`ImageFormatter::get_line_y`), and the `ShadowAdder` then offsets the whole
/// image by `pad_vert`. We build with `window_controls(false)` and no window
/// title, so `code_pad_top` is 0 and the origin of the first line is simply
/// `PAD + CODE_PAD`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineGeometry {
    /// Distance from the top of the PNG to the top of line 0.
    pub first_line_y: u32,
    /// Distance between the tops of two consecutive lines.
    pub line_height: u32,
}

impl LineGeometry {
    /// Y coordinate, in PNG pixels, of the vertical center of line `line_index`
    /// (0-based, counting every line of the rendered content).
    pub fn line_center_y(&self, line_index: usize) -> u32 {
        self.first_line_y + line_index as u32 * self.line_height + self.line_height / 2
    }

    /// Vertical center of `line_index` as a fraction (0.0 = top, 1.0 = bottom)
    /// of a PNG that is `image_height_px` tall. Clamped to the image bounds so a
    /// call site outside the captured range still yields a usable anchor.
    pub fn line_center_fraction(&self, line_index: usize, image_height_px: u32) -> f64 {
        if image_height_px == 0 {
            return 0.5;
        }
        let y = self.line_center_y(line_index) as f64 / image_height_px as f64;
        y.clamp(0.0, 1.0)
    }
}

/// X coordinate, in PNG pixels, just past the last character of `line_text`.
///
/// Mirrors silicon's `create_drawables`: text starts at `get_left_pad()` (which
/// is `code_pad` plus the line number gutter when line numbers are on) and
/// advances by `FontCollection::get_text_len`. Tabs are expanded first, exactly
/// as silicon does. The `ShadowAdder` offset (`PAD`) is added on top.
///
/// `total_lines` and `line_offset` are needed because the width of the line
/// number gutter depends on how many digits the largest line number has —
/// silicon computes it as `floor(log10(total_lines + line_offset)) + 1`.
pub fn line_end_x(
    font_size: Option<usize>,
    show_line_number: bool,
    total_lines: usize,
    line_offset: usize,
    line_text: &str,
) -> u32 {
    let size = font_size.map(|s| s as f32).unwrap_or(DEFAULT_FONT_SIZE);
    let font = silicon::font::FontCollection::new(&[("Hack", size)])
        .expect("Hack font not available for silicon");

    let left_pad = CODE_PAD
        + if show_line_number {
            let line_number_chars =
                (((total_lines + line_offset) as f32).log10() + 1.0).floor() as usize;
            let widest = format!("{:>width$}", 0, width = line_number_chars);
            2 * LINE_NUMBER_PAD + font.get_text_len(&widest)
        } else {
            0
        };

    let expanded = line_text
        .trim_end_matches('\n')
        .replace('\t', &" ".repeat(TAB_WIDTH));

    PAD + left_pad + font.get_text_len(&expanded)
}

/// Vertical geometry of a screenshot rendered by [`create_figure`] at `font_size`.
///
/// Depends only on the font metrics, so it can be computed before (or without)
/// rendering anything.
pub fn line_geometry(font_size: Option<usize>) -> LineGeometry {
    let size = font_size.map(|s| s as f32).unwrap_or(DEFAULT_FONT_SIZE);
    let font = silicon::font::FontCollection::new(&[("Hack", size)])
        .expect("Hack font not available for silicon");
    LineGeometry {
        first_line_y: PAD + CODE_PAD,
        line_height: font.get_font_height() + LINE_PAD,
    }
}

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
    // Detect language from file_name extension, default to Rust.
    let ext = file_name.rsplit('.').next().unwrap_or("rs");
    let syntax = match ext {
        // Solidity: use JavaScript syntax (best color match with Dracula)
        "sol" => ps
            .find_syntax_by_extension("js")
            .or_else(|| ps.find_syntax_by_extension("rs"))
            .expect("Syntax not found in syntect"),
        // For any other extension, try it directly first, fall back to Rust
        other => ps
            .find_syntax_by_extension(other)
            .or_else(|| ps.find_syntax_by_extension("rs"))
            .expect("Syntax not found in syntect"),
    };
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

#[cfg(test)]
mod line_geometry_test {
    use super::*;

    /// Renders two figures with a known difference in line count and checks that
    /// the measured PNG geometry matches [`line_geometry`].
    ///
    /// silicon's height is `n_lines * line_height + 2 * CODE_PAD + 2 * PAD`
    /// (`get_image_size` uses `get_line_y(max_lineno + 1) + code_pad`, and
    /// `max_lineno` is `n_lines - 1`), so the height delta between an `n` and an
    /// `n + k` line render is exactly `k * line_height`.
    #[test]
    fn test_line_geometry_matches_rendered_png() {
        let dir = std::env::temp_dir().join("bat_cli_line_geometry_test");
        std::fs::create_dir_all(&dir).unwrap();
        let dir_str = dir.to_str().unwrap();

        for font_size in [16usize, 20, 28] {
            let geometry = line_geometry(Some(font_size));

            let render = |n: usize, name: &str| -> (u32, u32) {
                let content = (0..n)
                    .map(|i| format!("let line_{i} = {i};"))
                    .collect::<Vec<_>>()
                    .join("\n");
                let path = create_figure(&content, dir_str, name, 1, Some(font_size), true);
                let dims = image::image_dimensions(&path).unwrap();
                std::fs::remove_file(&path).unwrap();
                dims
            };

            let (_, height_10) = render(10, &format!("probe_10_{font_size}.rs"));
            let (_, height_30) = render(30, &format!("probe_30_{font_size}.rs"));

            // 20 extra lines must add exactly 20 line heights.
            assert_eq!(
                height_30 - height_10,
                20 * geometry.line_height,
                "line_height mismatch at font size {font_size}"
            );

            // And the absolute height must match the closed form.
            let expected_10 = 10 * geometry.line_height + 2 * CODE_PAD + 2 * PAD;
            assert_eq!(
                height_10, expected_10,
                "absolute height mismatch at font size {font_size}"
            );

            // The last line's center must land inside the image, above the bottom pad.
            let last_center = geometry.line_center_y(9);
            assert!(last_center < height_10 - PAD, "last line center out of bounds");
            let fraction = geometry.line_center_fraction(9, height_10);
            assert!(
                fraction > 0.0 && fraction < 1.0,
                "fraction out of range: {fraction}"
            );
        }
    }

    /// Checks [`line_end_x`] against the actual pixels: renders a figure, scans
    /// the rows belonging to a known line, and finds the rightmost pixel that is
    /// not the Dracula background.
    #[test]
    fn test_line_end_x_matches_rendered_png() {
        let dir = std::env::temp_dir().join("bat_cli_line_end_x_test");
        std::fs::create_dir_all(&dir).unwrap();
        let dir_str = dir.to_str().unwrap();

        let font_size = 20usize;
        let offset = 1usize;
        let geometry = line_geometry(Some(font_size));

        // A long line first so the image is wider than the line we measure.
        let lines = vec![
            "let very_long_line_to_widen_the_whole_image = compute(a, b, c, d, e);",
            "let short = 1;",
            "self.rewarder.accrue(account, shares);",
            "",
        ];
        let content = lines.join("\n");
        let path = create_figure(&content, dir_str, "line_end_x.rs", offset, Some(font_size), true);
        let img = image::open(&path).unwrap().to_rgba8();
        let (width, _height) = img.dimensions();

        for (line_index, line_text) in lines.iter().enumerate() {
            if line_text.is_empty() {
                continue;
            }
            let expected = line_end_x(Some(font_size), true, lines.len(), offset, line_text);

            // Scan every pixel row of this line and keep the rightmost non-background one.
            let top = geometry.first_line_y + line_index as u32 * geometry.line_height;
            let mut measured = 0u32;
            for y in top..(top + geometry.line_height) {
                for x in (0..width).rev() {
                    if img.get_pixel(x, y) != &BG {
                        measured = measured.max(x);
                        break;
                    }
                }
            }

            // `line_end_x` returns the pen advance after the last character, so it
            // always sits at or slightly past the last inked pixel — the gap is the
            // glyph's right side bearing, strictly less than one character width.
            let char_width = line_end_x(Some(font_size), true, lines.len(), offset, "a")
                - line_end_x(Some(font_size), true, lines.len(), offset, "");
            let delta = expected as i64 - measured as i64;
            assert!(
                delta >= 0 && delta <= char_width as i64,
                "line {line_index} ({line_text:?}): predicted end x {expected}, \
                 measured {measured}, char width {char_width}"
            );
        }

        std::fs::remove_file(&path).unwrap();
    }
}
