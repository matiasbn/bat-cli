//! One frame holding every in-scope contract, whole, side by side.
//!
//! The question it answers is not "how does this work" but "how much of this have I read".
//! Twenty-five files in an editor is twenty-five tabs and no sense of proportion; as columns
//! on a board it is one picture, and a column you have not read is visibly a column you have
//! not read.
//!
//! Three measurements decided the shape of this (taken 2026-10-06 against a 49-file repo):
//!
//! - Miro refuses an image over **6 MB**. The longest contract in that repo, 823 lines,
//!   renders to 15 MB at the deploy's own font.
//! - It does NOT refuse a tall one. A 1154×13270 px image uploaded without complaint, so the
//!   8192 px the renderer warns about is not a limit of the board — nothing has to be cut
//!   into pieces, it only has to be rendered smaller.
//! - At font 12 that contract is 4.6 MB. The next size up, 16, is 6.6 MB and over.
//!
//! So the scale is chosen from the LONGEST file and applied to every other one. The columns
//! are left-aligned at the top rather than centred, because the thing being compared is how
//! far down each one goes.

use colored::Colorize;
use error_stack::{Result, ResultExt};
use rayon::prelude::*;

use crate::batbelt::evm::metadata::bat_metadata::EvmBatMetadata;
use crate::batbelt::evm::miro::auto_deploy::{matches_ignore, BOARD_UNITS_PER_PIXEL};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::miro::client::MiroClient;
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;

/// Font sizes to try for the longest file, largest first. The first one whose render fits
/// the upload budget is used for every file, so the board is one scale throughout.
const FONT_LADDER: &[usize] = &[20, 16, 14, 12, 10, 8, 6];

/// Miro rejects an image over 6 MB; stop short of it so a file a little longer than the one
/// measured does not fail the whole run.
const MAX_IMAGE_BYTES: u64 = 5_500_000;

/// Gap between two columns, and the margin around them, in board units.
const COLUMN_GAP: f64 = 240.0;
const MARGIN: f64 = 400.0;

/// Clear space left below the lowest frame already on the board, so this one cannot land on
/// top of anything — Miro refuses overlapping frames outright, with a 500.
const REGION_MARGIN: f64 = 4_000.0;

/// One file, rendered.
struct Column {
    path: String,
    lines: usize,
    png_path: String,
    width: f64,
    height: f64,
}

pub async fn run(include_dependencies: bool) -> Result<(), EvmMiroError> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    // The unit is the FILE, not the contract: a file holds a contract plus the interfaces
    // and libraries that belong with it, and that is what you actually read top to bottom.
    // Rendering per contract would both duplicate a shared file and cut each piece out of
    // the context around it.
    let mut paths: Vec<String> = metadata
        .contracts
        .iter()
        // Vendored code is left out by default and NOT because it is out of scope — that is
        // the ignore list's job — but because `lib/` is hundreds of thousands of lines
        // against this project's eleven thousand, and the picture stops being one. Asking
        // for it is a flag, so the choice is yours and visible.
        .filter(|contract| include_dependencies || !contract.vendored)
        .filter(|contract| {
            !metadata.ignored_contracts.iter().any(|pattern| {
                matches_ignore(pattern, &contract.name, &contract.file_path)
            })
        })
        .map(|contract| contract.file_path.clone())
        .collect();
    paths.sort();
    paths.dedup();

    if paths.is_empty() {
        println!("  {} no in-scope contracts to draw", "note:".yellow());
        return Ok(());
    }

    let sources: Vec<(String, Vec<String>)> = paths
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let mut lines = vec![format!("// {path}"), String::new()];
            lines.extend(blank_imports(&text));
            Some((path, lines))
        })
        .collect();

    let total_lines: usize = sources.iter().map(|(_, lines)| lines.len() - 2).sum();
    let longest = sources
        .iter()
        .max_by_key(|(_, lines)| lines.len())
        .expect("at least one file");
    println!(
        "{} {} file(s), {} lines; longest is {} at {}",
        "▦".blue(),
        sources.len().to_string().green(),
        total_lines.to_string().green(),
        longest.0,
        (longest.1.len() - 2).to_string().green()
    );

    BatFolder::Figures.create_folder().change_context(EvmMiroError)?;
    let destination = BatFolder::Figures
        .get_path(true)
        .change_context(EvmMiroError)?;
    let font = choose_font(&longest.1, &destination)?;
    println!("  rendering every file at font {}", font.to_string().green());

    let columns = render_columns(&sources, font, &destination)?;

    let width: f64 =
        columns.iter().map(|c| c.width).sum::<f64>() + COLUMN_GAP * (columns.len() as f64 - 1.0);
    let height: f64 = columns.iter().fold(0.0_f64, |tallest, c| tallest.max(c.height));
    let frame_width = width + MARGIN * 2.0;
    let frame_height = height + MARGIN * 2.0;

    let client = MiroClient::new_refreshed().await.change_context(EvmMiroError)?;
    let (frame_x, frame_y) = free_spot(&client, frame_width, frame_height).await?;
    let title = format!("overview: {} files, {} lines", columns.len(), total_lines);
    let frame_id = client
        .create_frame(&title, frame_x, frame_y, frame_width, frame_height, None)
        .await
        .change_context(EvmMiroError)?;

    // Positions inside a frame are the item's CENTRE measured from the frame's top-left.
    // The columns are top-aligned: what is being compared is how far down each one goes.
    let mut cursor = MARGIN;
    let mut uploads = tokio::task::JoinSet::new();
    for column in &columns {
        let client = client.clone();
        let frame_id = frame_id.clone();
        let png = column.png_path.clone();
        let label = column.path.clone();
        let (x, y) = (cursor + column.width / 2.0, MARGIN + column.height / 2.0);
        let width = column.width;
        cursor += column.width + COLUMN_GAP;
        uploads.spawn(async move {
            client
                .create_image_in_frame(&png, &frame_id, &label, x, y, width)
                .await
                .map(|_| label)
        });
    }

    let mut failed = Vec::new();
    while let Some(joined) = uploads.join_next().await {
        // One file failing must not cost the other forty-eight.
        match joined {
            Ok(Ok(_)) => {}
            Ok(Err(report)) => failed.push(report.to_string()),
            Err(join_error) => failed.push(join_error.to_string()),
        }
    }
    for failure in &failed {
        println!("  {} {failure}", "skipped:".yellow());
    }

    println!("  {}", client.frame_url(&frame_id).blue());
    let _ = std::fs::remove_dir_all(&destination);
    Ok(())
}

/// The file's lines with every `import` statement replaced by an empty one.
///
/// An import path is often the widest line in a file — `import {A, B} from
/// "../../lib/openzeppelin-contracts/contracts/token/ERC20/utils/SafeERC20.sol";` — and the
/// column is as wide as its widest line, so a handful of them widen the whole picture for
/// something nobody reads in an overview.
///
/// Blanked, not deleted: dropping the lines would shift every line number after them, and a
/// screenshot whose numbers disagree with the editor is worse than a wide one. The gap left
/// at the top of a file is itself legible — that is where the imports were.
///
/// Handles the multi-line form, where the statement runs until its semicolon.
fn blank_imports(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !inside && trimmed.starts_with("import") {
            inside = !line.contains(';');
            out.push(String::new());
            continue;
        }
        if inside {
            inside = !line.contains(';');
            out.push(String::new());
            continue;
        }
        out.push(line.to_string());
    }
    out
}

/// The largest font whose render of the longest file still fits the upload budget.
///
/// Measured on the one file that decides it, rather than estimated: the byte size of a PNG
/// depends on what is in it, not only on how many pixels it has.
fn choose_font(longest: &[String], destination: &str) -> Result<usize, EvmMiroError> {
    let source = longest.join("\n");
    for font in FONT_LADDER {
        let path = silicon::create_figure(
            &source,
            destination,
            &format!("overview_probe_{font}.js"),
            0,
            Some(*font),
            true,
        );
        let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(u64::MAX);
        let _ = std::fs::remove_file(&path);
        if bytes <= MAX_IMAGE_BYTES {
            return Ok(*font);
        }
    }
    Ok(*FONT_LADDER.last().expect("the ladder is not empty"))
}

fn render_columns(
    sources: &[(String, Vec<String>)],
    font: usize,
    destination: &str,
) -> Result<Vec<Column>, EvmMiroError> {
    let bar = crate::batbelt::evm::miro::auto_deploy::phase_bar("rendering files", sources.len());
    let rendered: std::result::Result<Vec<Column>, String> = sources
        .par_iter()
        .map(|(path, lines)| {
            let name = format!("overview_{}.js", path.replace([':', '.', '/'], "_"));
            let png_path = silicon::create_figure(
                &lines.join("\n"),
                destination,
                &name,
                0,
                Some(font),
                true,
            );
            let (png_width, png_height) = image::image_dimensions(&png_path)
                .map_err(|e| format!("cannot measure {png_path}: {e}"))?;
            bar.inc(1);
            Ok(Column {
                path: path.clone(),
                lines: lines.len() - 2,
                png_path,
                width: png_width as f64 * BOARD_UNITS_PER_PIXEL,
                height: png_height as f64 * BOARD_UNITS_PER_PIXEL,
            })
        })
        .collect();
    bar.finish_and_clear();
    rendered.map_err(|message| {
        error_stack::Report::new(EvmMiroError).attach_printable(message)
    })
}

/// A centre for a frame of this size that overlaps nothing already on the board.
///
/// Miro refuses overlapping frames with a 500, so this is not a nicety. The board is asked
/// for its frames and the new one goes below all of them — the same rule the deploy region
/// uses, and it re-reads the board every time so two runs in a row cannot collide either.
pub(crate) async fn free_spot(
    client: &MiroClient,
    width: f64,
    height: f64,
) -> Result<(f64, f64), EvmMiroError> {
    let frames = client.list_frames().await.change_context(EvmMiroError)?;
    // Under the frame that reaches lowest, and aligned with IT — not with the leftmost
    // frame on the board, which lets one frame dragged far left move the origin for
    // everything drawn afterwards.
    let (left, top) = crate::batbelt::evm::miro::auto_deploy::below_everything(&frames);
    println!(
        "  below {} existing frame(s), at ({}, {})",
        frames.len(),
        left.round(),
        top.round()
    );
    Ok((left + width / 2.0, top + height / 2.0))
}

#[cfg(test)]
mod import_test {
    use super::*;

    /// Blanked and not deleted, so a line number still means what it means in the editor.
    #[test]
    fn an_import_becomes_an_empty_line_in_place() {
        let source = "pragma solidity ^0.8.0;\nimport {A} from \"./A.sol\";\ncontract C {}";
        let out = blank_imports(source);
        assert_eq!(out, vec!["pragma solidity ^0.8.0;", "", "contract C {}"]);
    }

    /// A multi-line import runs to its semicolon, and all of it goes.
    #[test]
    fn a_multi_line_import_is_blanked_to_its_semicolon() {
        let source = "import {\n    A,\n    B\n} from \"./A.sol\";\ncontract C {}";
        let out = blank_imports(source);
        assert_eq!(out, vec!["", "", "", "", "contract C {}"]);
    }

    /// A line that merely mentions the word is code, not a statement.
    #[test]
    fn a_word_inside_code_is_not_an_import() {
        let source = "    uint256 imported = 1;\n    // import this\n";
        let out = blank_imports(source);
        assert_eq!(out, vec!["    uint256 imported = 1;", "    // import this"]);
    }
}
