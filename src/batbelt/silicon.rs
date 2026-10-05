use once_cell::sync::Lazy;
use silicon::assets::HighlightingAssets;
use silicon::formatter::ImageFormatterBuilder;
use silicon::utils::{Background, ShadowAdder};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;

use std::fs;

/// Syntax definitions and themes, loaded once.
///
/// `HighlightingAssets::new()` reads and decodes the bundled syntax and theme
/// dumps every time it is called. Doing that per screenshot dominated the
/// render phase of a deployment, which produces dozens of them.
static ASSETS: Lazy<HighlightingAssets> = Lazy::new(HighlightingAssets::new);

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

/// bat-cli's palette for everything drawn ON the board: the arrows between screenshots,
/// the cards, the tints. Its job is to separate shapes from each other against white.
pub const BAT_PALETTE: &[&str] = &[
    "#2d9bf0", "#f24726", "#8fd14f", "#fac710", "#a259ff", "#12cdd4", "#ff8c00", "#e6007a",
];

/// Colours for tracing a name INSIDE a screenshot. A reader cannot click an identifier on
/// a PNG the way they can in an editor, so each traced name is painted in its own colour
/// wherever it appears and the signature becomes the legend: read `address assetIn` in
/// salmon, then sweep the body for salmon.
///
/// Deliberately NOT `BAT_PALETTE`, and the difference is not cosmetic: the two palettes
/// answer different questions. On the board a colour has to separate one arrow from the
/// next against white. Inside a screenshot it has to stand out from a syntax theme that
/// already uses green for calls, orange for types and yellow for fields — `BAT_PALETTE`
/// was tried there and three of its eight colours were lost in the highlighting.
///
/// These are Dracula's own BRIGHT variants: built for `#282a36`, and distinct from each
/// other at the low alpha a mark is drawn with.
///
/// Green is in the list, which it could not be while the TEXT was being recoloured — green
/// is what the theme gives function names, and a Solidity body is mostly calls. Marking the
/// background instead of the glyphs took that constraint away, and the extra slots are what
/// let local variables be followed at all: a function with five parameters would otherwise
/// spend the whole palette before reaching them.
pub const TRACE_COLORS: &[&str] = &[
    "#ff6e6e", "#69ff94", "#d6acff", "#ffffa5", "#a4ffff", "#ff92df", "#ffb86c", "#8be9fd",
];

/// How many times `name` appears in `text` as a WHOLE word. Ranking by `str::matches`
/// instead counts `f` inside `if` and `feeWad`, which put one-letter names at the top of
/// every function.
pub fn count_word(text: &str, name: &str) -> usize {
    let mut count = 0;
    let mut from = 0usize;
    while let Some(at) = find_word(&text[from..], name) {
        count += 1;
        from += at + name.len();
    }
    count
}

/// `name` as a WHOLE word: `p` must not match the `p` inside `supply`, and `from` must not
/// match `p.from`'s field when the traced name is the variable `from` — a word boundary is
/// anything that cannot be part of a Solidity identifier.
fn find_word(haystack: &str, name: &str) -> Option<usize> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let mut from = 0usize;
    while let Some(at) = haystack[from..].find(name) {
        let at = from + at;
        let before_ok = at == 0 || !haystack[..at].chars().next_back().is_some_and(is_ident);
        let after = at + name.len();
        let after_ok = after >= haystack.len() || !haystack[after..].chars().next().is_some_and(is_ident);
        if before_ok && after_ok {
            return Some(at);
        }
        from = at + name.len().max(1);
    }
    None
}

/// Where each traced name sits in the rendered PNG: one rectangle per occurrence, in
/// pixels, with the index of the colour it is traced in.
///
/// Mirrors silicon's own layout — text starts at `get_left_pad()` and advances by
/// `FontCollection::get_text_len`, tabs expanded first — which is the same arithmetic
/// `line_end_x` uses to anchor a connector on a token. A monospaced font would let us
/// multiply by a character width; measuring the prefix instead is what keeps this correct
/// if the font ever changes.
fn traced_rects(
    content: &str,
    traced: &[String],
    font_size: Option<usize>,
    show_line_number: bool,
    line_offset: usize,
) -> Vec<TracedRect> {
    if traced.is_empty() {
        return Vec::new();
    }
    let size = font_size.map(|s| s as f32).unwrap_or(DEFAULT_FONT_SIZE);
    let font = silicon::font::FontCollection::new(&[("Hack", size)])
        .expect("Hack font not available for silicon");
    let geometry = line_geometry(font_size);
    let lines: Vec<&str> = content.lines().collect();

    let left_pad = CODE_PAD
        + if show_line_number {
            let line_number_chars =
                (((lines.len() + line_offset) as f32).log10() + 1.0).floor() as usize;
            let widest = format!("{:>width$}", 0, width = line_number_chars);
            2 * LINE_NUMBER_PAD + font.get_text_len(&widest)
        } else {
            0
        };

    let mut rects = Vec::new();
    for (row, line) in lines.iter().enumerate() {
        let expanded = line.replace('\t', &" ".repeat(TAB_WIDTH));
        for (index, name) in traced.iter().enumerate() {
            let mut from = 0usize;
            while let Some(at) = find_word(&expanded[from..], name) {
                let at = from + at;
                from = at + name.len();
                if is_field_key(&expanded, at, name.len()) {
                    continue;
                }
                let x = PAD + left_pad + font.get_text_len(&expanded[..at]);
                let width = font.get_text_len(&expanded[at..at + name.len()]);
                // `first_line_y` already carries the ShadowAdder's padding.
                let y = geometry.first_line_y + row as u32 * geometry.line_height;
                rects.push(TracedRect { x, y, width, height: geometry.line_height, index });
            }
        }
    }
    rects
}

/// One mark: where it goes, and which traced name it belongs to.
struct TracedRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    index: usize,
}

/// Whether this occurrence is a struct literal's FIELD NAME rather than a use of the
/// variable — `amountIn: amountIn` names the field on the left and passes the variable on
/// the right, and marking both says the value flows into itself.
///
/// A field key is the first thing on its line and is followed by a colon. A ternary's
/// colon also follows a value, which is why the rule is anchored at the start of the line:
/// `a ? b : c` never has `b` there.
fn is_field_key(line: &str, at: usize, len: usize) -> bool {
    let starts_the_line = line[..at].trim().is_empty();
    let followed_by_colon = line[at + len..]
        .trim_start()
        .starts_with(|c: char| c == ':');
    starts_the_line && followed_by_colon
}

/// Paint each traced occurrence's own colour BEHIND it, like a marker pen.
///
/// The foreground is left to the syntax theme on purpose. The theme already spends every
/// hue it has — green on calls, orange on types, yellow on fields, pink on keywords — so
/// recolouring an identifier makes it compete with that; the background is the one register
/// nothing else uses. It is also what an editor does when you click a name.
///
/// Drawn OVER the finished image at low alpha rather than under the text, because silicon
/// composes the text itself and ignores a span's background (`formatter.rs`, which reads
/// only `style.foreground`).
fn paint_traces(image: &mut image::DynamicImage, rects: &[TracedRect], underlined: usize) {
    use image::GenericImageView;
    const ALPHA: f32 = 0.30;
    /// Thickness of a parameter's underline, in pixels.
    const RULE: u32 = 3;
    let mut buffer = image.to_rgba8();
    let (width, height) = image.dimensions();
    for rect in rects {
        let tint = parse_hex(TRACE_COLORS[rect.index % TRACE_COLORS.len()]);
        let blend = |under: u8, over: u8, alpha: f32| {
            (under as f32 * (1.0 - alpha) + over as f32 * alpha).round() as u8
        };
        for py in rect.y..(rect.y + rect.height).min(height) {
            // A PARAMETER also gets a solid rule under it. The two kinds of name answer
            // different questions — a parameter is what the caller chose, a local is what
            // this function made of it — and telling them apart at a glance is the whole
            // reason to mark them. Locals carry the tint alone.
            let on_rule =
                rect.index < underlined && py + RULE >= rect.y + rect.height;
            let alpha = if on_rule { 1.0 } else { ALPHA };
            for px in rect.x..(rect.x + rect.width).min(width) {
                let pixel = buffer.get_pixel_mut(px, py);
                pixel[0] = blend(pixel[0], tint.r, alpha);
                pixel[1] = blend(pixel[1], tint.g, alpha);
                pixel[2] = blend(pixel[2], tint.b, alpha);
            }
        }
    }
    *image = image::DynamicImage::ImageRgba8(buffer);
}

fn parse_hex(hex: &str) -> syntect::highlighting::Color {
    let value = hex.trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&value[i..i + 2], 16).unwrap_or(0xff);
    syntect::highlighting::Color { r: byte(0), g: byte(2), b: byte(4), a: 0xff }
}

pub fn create_figure(
    content: &str,
    dest_folder_path: &str,
    file_name: &str,
    offset: usize,
    font_size: Option<usize>,
    show_line_number: bool,
) -> String {
    create_figure_tracing(
        content,
        dest_folder_path,
        file_name,
        offset,
        font_size,
        show_line_number,
        &[],
        0,
    )
}

/// `create_figure`, plus the names to mark through the code, each in its own colour.
#[allow(clippy::too_many_arguments)]
pub fn create_figure_tracing(
    content: &str,
    dest_folder_path: &str,
    file_name: &str,
    offset: usize,
    font_size: Option<usize>,
    show_line_number: bool,
    traced: &[String],
    // How many of `traced` are the function's PARAMETERS. They come first in the list and
    // are the ones that also get a rule under them.
    parameters: usize,
) -> String {
    let dest_png_path = format!("{dest_folder_path}/{file_name}.png");

    let size = font_size.map(|s| s as f32).unwrap_or(DEFAULT_FONT_SIZE);

    let ps = &ASSETS.syntax_set;
    let theme = &ASSETS.theme_set.themes["Dracula"];

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
    let mut highlight: Vec<Vec<(syntect::highlighting::Style, &str)>> =
        LinesWithEndings::from(content)
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

    let mut image = formatter.format(&highlight, theme);
    paint_traces(
        &mut image,
        &traced_rects(content, traced, font_size, show_line_number, offset),
        parameters,
    );

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

#[cfg(test)]
mod trace_test {
    use super::*;

    /// The rectangles must land on the token, and a name used twice on one line must get
    /// two of them — the geometry is the same arithmetic a connector anchor uses.
    #[test]
    fn a_rect_is_produced_per_occurrence_and_lines_up_with_the_text() {
        let content = "// path.sol\n\nuint a = b + amountIn;\nx = amountIn * amountIn;";
        let rects = traced_rects(content, &["amountIn".to_string()], Some(20), true, 0);
        assert_eq!(rects.len(), 3, "one on line 3, two on line 4");

        let geometry = line_geometry(Some(20));
        assert_eq!(rects[0].y, geometry.first_line_y + 2 * geometry.line_height);
        assert!(rects[0].width > 0 && rects[0].height == geometry.line_height);
        // The second occurrence on a line sits to the right of the first.
        assert!(rects[2].x > rects[1].x);
        assert_eq!(rects[1].y, rects[2].y, "same line, same row");
    }

    /// A struct literal's field name is not a use of the variable: `amountIn: amountIn`
    /// names the field on the left and passes the value on the right.
    #[test]
    fn a_struct_field_key_is_not_marked() {
        let content = "// path.sol\n\n    amountIn: amountIn,";
        let rects = traced_rects(content, &["amountIn".to_string()], Some(20), true, 0);
        assert_eq!(rects.len(), 1, "only the value on the right is a use");
    }

    /// A ternary's colon follows a value mid-line, which must stay marked.
    #[test]
    fn a_ternary_is_not_mistaken_for_a_field_key() {
        let content = "// path.sol\n\n    uint a = x > y ? amountIn : other;";
        let rects = traced_rects(content, &["amountIn".to_string()], Some(20), true, 0);
        assert_eq!(rects.len(), 1);
    }

    #[test]
    fn nothing_is_painted_when_nothing_is_traced() {
        assert!(traced_rects("uint a = b;", &[], Some(20), true, 0).is_empty());
    }

    #[test]
    fn a_traced_name_matches_whole_words_only() {
        // `p` must not light up the `p` inside `supply`, and `from` must not light up
        // `p.from`'s field — a screenshot full of false positives is worse than none.
        assert_eq!(find_word("uint256 supply", "p"), None);
        assert_eq!(find_word("p.from = x", "p"), Some(0));
        assert_eq!(find_word("$.loans[p.from]", "$"), Some(0));
        assert_eq!(find_word("cin.token", "token"), Some(4));
        assert_eq!(find_word("maxSwapNotional", "Swap"), None);
    }


}



