//! Put a blue band on chosen lines of a frame already on the board.
//!
//! The assistant reads the code and the auditor reads the board; this is how one points at
//! something for the other. "Look at `MMRouterLib.fund` lines 288 to 291" means hunting for
//! a box among thirty frames — a green band means opening the link and seeing it.
//!
//! Blue on purpose: red and amber on a frame are claims the deploy makes about the code —
//! this writes storage, this leaves the repository — and a mark that merely says "look here"
//! must not be confused with them. Green was tried and is too close to the amber at the
//! alpha a band is drawn with.
//!
//! It works from the registry, never by re-deriving the drawing: the band lands where the
//! line actually is because `LineMap` records what each screenshot shows and at what size.
//! Nothing is re-laid-out and nothing else on the frame is touched, which is the same
//! promise `screenshot` makes — the arrangement on the board is the auditor's.

use colored::Colorize;
use error_stack::{Report, Result, ResultExt};

use crate::batbelt::evm::metadata::bat_metadata::{AutoDeployedFrame, EvmBatMetadata, LineMap};
use crate::batbelt::evm::miro::auto_deploy::PATH_HEADER_LINES;
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::miro::client::MiroClient;
use crate::batbelt::silicon;

/// Miro's blue. The colours a band can take are nearly all spoken for: red is a state
/// change, amber an external boundary, and green at 30% over dark code is close enough to
/// the amber to be mistaken for it. Blue is the one register nothing on a line uses.
const HIGHLIGHT_COLOR: &str = "#2d9bf0";

pub struct HighlightOptions {
    /// The deployment: the entry point it was deployed for.
    pub deployment: Option<String>,
    /// Which frame of that deployment — a function's name. Omitted means the deployment's
    /// own root frame.
    pub function: Option<String>,
    /// The frame's Miro link, when the names are ambiguous.
    pub frame_url: Option<String>,
    /// File lines to mark: `288`, `288-291`, or a comma-separated mix of both. Empty when
    /// clearing.
    pub lines: String,
    /// Remove every band this command put on the frame, and draw nothing.
    pub clear: bool,
    /// List what can be marked instead of marking anything.
    pub list: bool,
}

pub async fn run(options: HighlightOptions) -> Result<(), EvmMiroError> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    if options.list {
        return list(&metadata, options.deployment.as_deref());
    }
    let frame = pick_frame(&metadata, &options)?;
    if options.clear {
        return clear(frame).await;
    }
    let wanted = parse_lines(&options.lines)?;

    if frame.line_maps.is_empty() {
        return Err(Report::new(EvmMiroError).attach_printable(format!(
            "`{}` was drawn before bat-cli recorded what each screenshot shows, so a line \
             cannot be located on it — deploy it again",
            frame.entry_point
        )));
    }

    // A line can appear on SEVERAL screenshots of one frame: a helper copied next to two
    // callers is drawn twice on purpose (see docs/diagram-deploy-design.md §2). Marking
    // every copy is right — the reader may be looking at either.
    let positions: Vec<(&LineMap, f64, f64)> = frame
        .line_maps
        .iter()
        .filter_map(|map| {
            frame
                .node_positions
                .iter()
                .find(|(id, _, _)| *id == map.node_id)
                .map(|(_, x, y)| (map, *x, *y))
        })
        .collect();

    let mut bands: Vec<(f64, f64, f64, f64, String, usize)> = Vec::new();
    let mut unplaced: Vec<usize> = Vec::new();
    for line in &wanted {
        let mut found = false;
        for (map, x, y) in &positions {
            if *line < map.start_line || *line > map.end_line || map.png_height == 0 {
                continue;
            }
            let geometry = silicon::line_geometry(Some(map.font_size));
            let height = (map.height * geometry.line_height as f64 / map.png_height as f64).max(1.0);
            let rendered_index = PATH_HEADER_LINES + (line - map.start_line);
            let fraction = geometry.line_center_fraction(rendered_index, map.png_height);
            let center_y = y - map.height / 2.0 + map.height * fraction;
            bands.push((*x, center_y, map.width, height, map.node_id.clone(), *line));
            found = true;
        }
        if !found {
            unplaced.push(*line);
        }
    }

    // Said before anything is drawn: a line that is not on this frame is the auditor
    // pointing at the wrong one, and finding out from a band that never appeared is worse
    // than being told.
    if !unplaced.is_empty() {
        let shown: Vec<String> = unplaced.iter().map(|l| l.to_string()).collect();
        println!(
            "  {} line(s) {} are not drawn on `{}`",
            "note:".yellow(),
            shown.join(", "),
            frame.entry_point
        );
    }
    if bands.is_empty() {
        return Err(Report::new(EvmMiroError)
            .attach_printable("none of those lines appear on that frame"));
    }

    let client = MiroClient::new_refreshed().await.change_context(EvmMiroError)?;
    let mut drawn = Vec::new();
    let mut first: Option<String> = None;

    // The frame itself first, so it is recognisable from the zoom where a whole cluster
    // fits on screen — a band is one line tall and a screenshot's outline is not much more.
    // Only once per frame, however many lines are marked.
    if frame.highlights.is_empty() {
        let id = client
            .create_frame_outline(&frame.frame_id, frame.width, frame.height, HIGHLIGHT_COLOR)
            .await
            .change_context(EvmMiroError)?;
        drawn.push(id);
    }

    // One border per screenshot that received a band, drawn BEFORE the bands so a band is
    // never hidden under it. It is what makes a marked line findable from across a frame of
    // twenty boxes — the band itself is one line tall.
    let mut bordered: Vec<&str> = Vec::new();
    for (_, _, _, _, node_id, _) in &bands {
        if bordered.contains(&node_id.as_str()) {
            continue;
        }
        bordered.push(node_id);
        let Some((map, x, y)) = positions.iter().find(|(m, _, _)| m.node_id == *node_id) else {
            continue;
        };
        let id = client
            .create_highlight_border(&frame.frame_id, *x, *y, map.width, map.height, HIGHLIGHT_COLOR)
            .await
            .change_context(EvmMiroError)?;
        drawn.push(id);
    }

    for (x, y, width, height, node_id, line) in bands {
        let id = client
            .create_colored_line_band(&frame.frame_id, x, y, width, height, HIGHLIGHT_COLOR)
            .await
            .change_context(EvmMiroError)?;
        println!("  {} {}:{}", "●".blue(), node_id.replacen("::", ".", 1), line);
        first.get_or_insert_with(|| id.clone());
        drawn.push(id);
    }

    // Recorded so the next deploy of this entry point can report them, and so they are not
    // orphan shapes nobody can account for.
    let entry_point = frame.entry_point.clone();
    let cluster_root = frame.cluster_root.clone();
    EvmBatMetadata::update_metadata(move |m| {
        if let Some(record) = m
            .miro
            .auto
            .frames
            .iter_mut()
            .find(|f| f.entry_point == entry_point && f.cluster_root == cluster_root)
        {
            record.highlights.extend(drawn.iter().cloned());
        }
    })
    .change_context(EvmMiroError)?;

    // The link lands on the BAND, not the frame. `moveToWidget` takes any item, and the
    // whole point of this is to arrive at the line instead of hunting for a box among
    // twenty screenshots.
    if let Some(id) = first {
        println!("  {}", client.frame_url(&id).blue());
    }
    Ok(())
}

/// What can be marked: every function DRAWN in a deployment, with the lines it shows.
///
/// Guessing and failing is the alternative, and the set is not obvious — a deployment draws
/// what its own entry point reaches, so `TrancheToken.deposit` has `_enter` and not `_exit`,
/// which lives on the redeem path. `screenshot` lists frames, a much smaller set: most
/// functions are screenshots INSIDE one.
fn list(metadata: &EvmBatMetadata, deployment: Option<&str>) -> Result<(), EvmMiroError> {
    let mut deployments: Vec<&str> = metadata
        .miro
        .auto
        .frames
        .iter()
        .map(|f| f.cluster_root.as_str())
        .filter(|root| deployment.is_none_or(|wanted| *root == wanted))
        .collect();
    deployments.sort_unstable();
    deployments.dedup();

    if deployments.is_empty() {
        println!("  {} no deployment{}", "note:".yellow(), match deployment {
            Some(name) => format!(" called `{name}`"),
            None => " on this board".to_string(),
        });
        return Ok(());
    }

    for root in deployments {
        let frames: Vec<&AutoDeployedFrame> = metadata
            .miro
            .auto
            .frames
            .iter()
            .filter(|f| f.cluster_root == root)
            .collect();
        let mut drawable: Vec<(String, usize, usize)> = frames
            .iter()
            .flat_map(|f| f.line_maps.iter())
            .map(|map| {
                // A function copied next to two callers is drawn twice on purpose and
                // carries a `#dup…` id for each copy. To the question "what can I mark" it
                // is one function — and marking it marks every copy anyway.
                let name = map.node_id.split('#').next().unwrap_or(&map.node_id);
                (name.replacen("::", ".", 1), map.start_line, map.end_line)
            })
            .collect();
        drawable.sort();
        drawable.dedup();

        println!("\n{} {}", "▸".blue(), root.bold());
        if drawable.is_empty() {
            println!(
                "  {} drawn before 0.26.37, which is when bat-cli started recording what each",
                "note:".yellow()
            );
            println!("  screenshot shows, so nothing on it can be marked — deploy it again");
            continue;
        }
        for (name, start, end) in drawable {
            println!("  {name:<48} lines {start}-{end}");
        }
    }
    Ok(())
}

/// Take every band off the frame.
///
/// Only the ones this command drew: their ids were recorded, so the auditor's own shapes —
/// and the deploy's red and amber marks, which mean something about the code — are never
/// touched. A band whose id the board no longer knows is forgotten rather than reported, so
/// deleting one by hand in Miro does not leave the registry stuck.
async fn clear(frame: &AutoDeployedFrame) -> Result<(), EvmMiroError> {
    if frame.highlights.is_empty() {
        println!("  {} nothing to clear on `{}`", "note:".yellow(), frame.entry_point);
        return Ok(());
    }
    let client = MiroClient::new_refreshed().await.change_context(EvmMiroError)?;
    let count = frame.highlights.len();
    for id in &frame.highlights {
        client.delete_item(id).await.change_context(EvmMiroError)?;
    }
    let entry_point = frame.entry_point.clone();
    let cluster_root = frame.cluster_root.clone();
    EvmBatMetadata::update_metadata(move |m| {
        if let Some(record) = m
            .miro
            .auto
            .frames
            .iter_mut()
            .find(|f| f.entry_point == entry_point && f.cluster_root == cluster_root)
        {
            record.highlights.clear();
        }
    })
    .change_context(EvmMiroError)?;
    println!("  {} {count} band(s) removed", "✓".green());
    println!("  {}", client.frame_url(&frame.frame_id).blue());
    Ok(())
}

/// `288`, `288-291`, or `288,290-292` — 1-based file lines, inclusive.
fn parse_lines(spec: &str) -> Result<Vec<usize>, EvmMiroError> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let parsed = match part.split_once('-') {
            Some((from, to)) => {
                let from: usize = from.trim().parse().map_err(|_| bad(part))?;
                let to: usize = to.trim().parse().map_err(|_| bad(part))?;
                if to < from {
                    return Err(bad(part));
                }
                (from..=to).collect::<Vec<usize>>()
            }
            None => vec![part.parse().map_err(|_| bad(part))?],
        };
        out.extend(parsed);
    }
    out.sort_unstable();
    out.dedup();
    if out.is_empty() {
        return Err(bad(spec));
    }
    Ok(out)
}

fn bad(spec: &str) -> Report<EvmMiroError> {
    Report::new(EvmMiroError)
        .attach_printable(format!("`{spec}` is not a line or a range; use `288` or `288-291`"))
}

/// The frame to draw on, by the same selectors `bat-cli screenshot` uses.
fn pick_frame<'a>(
    metadata: &'a EvmBatMetadata,
    options: &HighlightOptions,
) -> Result<&'a AutoDeployedFrame, EvmMiroError> {
    let frames = &metadata.miro.auto.frames;
    if let Some(url) = &options.frame_url {
        // A URL is exact, which is why it exists: a title can be shared by frames of several
        // deployments, and a band on the wrong one is worse than no band.
        let id = url.rsplit("moveToWidget=").next().unwrap_or("").trim();
        return frames
            .iter()
            .find(|f| f.frame_id == id)
            .ok_or_else(|| Report::new(EvmMiroError).attach_printable(format!("no frame {id}")));
    }

    let deployment = options.deployment.as_deref();
    let wanted = options.function.as_deref().or(deployment);
    let Some(wanted) = wanted else {
        return Err(Report::new(EvmMiroError)
            .attach_printable("say which frame: --deployment, --function, or --frame-url"));
    };

    let in_deployment =
        |f: &&AutoDeployedFrame| deployment.is_none_or(|root| f.cluster_root == root);
    let named = |f: &&AutoDeployedFrame| {
        f.entry_point == wanted || f.entry_point.ends_with(&format!(".{wanted}"))
    };

    let mut matches: Vec<&AutoDeployedFrame> =
        frames.iter().filter(named).filter(in_deployment).collect();

    // Most functions have no frame of their own: they are screenshots INSIDE one. Naming
    // `TrancheController.depositFor` should find the frame that draws it, not report that
    // no frame is called that — which is true and useless.
    if matches.is_empty() {
        let node_key = wanted.replacen('.', "::", 1);
        matches = frames
            .iter()
            .filter(in_deployment)
            .filter(|f| {
                f.line_maps.iter().any(|map| {
                    map.node_id == node_key || map.node_id.starts_with(&format!("{node_key}("))
                })
            })
            .collect();

        // A deployment drawn before the line maps existed can answer nothing, and saying
        // "that function is not drawn" blames the function for the record being old. The
        // two are different problems and only one of them is fixed by looking elsewhere.
        if matches.is_empty() {
            let candidates: Vec<&AutoDeployedFrame> =
                frames.iter().filter(in_deployment).collect();
            if !candidates.is_empty() && candidates.iter().all(|f| f.line_maps.is_empty()) {
                return Err(Report::new(EvmMiroError).attach_printable(format!(
                    "the deployment of `{}` was drawn before bat-cli recorded what each \
                     screenshot shows, so nothing on it can be marked — deploy it again",
                    candidates[0].cluster_root
                )));
            }
        }
    }

    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(Report::new(EvmMiroError).attach_printable(format!(
            "nothing called `{wanted}` is drawn{} — `bat-cli highlight --list` says what is",
            deployment.map(|d| format!(" in the deployment of {d}")).unwrap_or_default()
        ))),
        _ => {
            let urls: Vec<String> = matches.iter().map(|f| f.frame_url.clone()).collect();
            Err(Report::new(EvmMiroError).attach_printable(format!(
                "`{wanted}` is drawn on {} frames; narrow it with --deployment, or pass \
                 --frame-url with one of:\n    {}",
                matches.len(),
                urls.join("\n    ")
            )))
        }
    }
}

#[cfg(test)]
mod line_spec_test {
    use super::*;

    #[test]
    fn a_single_line_a_range_and_a_mix_all_parse() {
        assert_eq!(parse_lines("288").unwrap(), vec![288]);
        assert_eq!(parse_lines("288-291").unwrap(), vec![288, 289, 290, 291]);
        assert_eq!(parse_lines("291, 288-289").unwrap(), vec![288, 289, 291]);
    }

    /// Asking twice for the same line should not draw two bands on it.
    #[test]
    fn a_line_named_twice_is_marked_once() {
        assert_eq!(parse_lines("288,288-289,289").unwrap(), vec![288, 289]);
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed() {
        assert!(parse_lines("").is_err());
        assert!(parse_lines("abc").is_err());
        assert!(parse_lines("291-288").is_err(), "a backwards range is a typo");
    }
}
