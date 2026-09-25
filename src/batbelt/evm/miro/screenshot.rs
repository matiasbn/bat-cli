//! `bat-cli screenshot`: put one declaration's source onto a frame that is already
//! on the board.
//!
//! A function's screenshot does not always explain itself. It compares against
//! `deviationThresholdWad`, or takes a `LevCurve` — the names are on screen, the
//! declarations are not, so reading the diagram means leaving it for the source.
//!
//! Earlier attempts tried to decide automatically WHICH symbols deserve to be drawn.
//! Every such rule either buries the board in things the auditor already knows, or
//! argues about categories instead of doing the job. So there is no rule here: the
//! auditor (or the assistant they are talking to) names a symbol, and this draws it.
//! The judgement of what is worth seeing stays with the person reading the diagram.
//!
//! Two ways to say what to draw, because the index will never cover everything:
//!
//! - by name — `deviationThresholdWad`, `PriceFeed.FeedType` — resolved from the scan;
//! - by range — `--file src/X.sol --lines 12-30` — for anything the index misses, so
//!   the feature never blocks on a gap.
//!
//! The image is rendered exactly the way `deploy` renders a function, so it sits on the
//! board indistinguishable from the rest of the diagram, and it is placed in free space
//! below the existing content: never on top of anything, and the auditor drags it where
//! they want it.

use colored::Colorize;
use error_stack::{IntoReport, Report, ResultExt};

use crate::batbelt::evm::metadata::bat_metadata::{
    AutoDeployedFrame, EvmBatMetadata, ExtraScreenshot,
};
use crate::batbelt::evm::miro::auto_deploy::{
    natspec_start, read_slice, save_frame_record, BOARD_UNITS_PER_PIXEL, PATH_HEADER_LINES,
    REFERENCE_FONT,
};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::evm::types::EvmContractType;
use crate::batbelt::miro::client::MiroClient;
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;

type Result<T> = error_stack::Result<T, EvmMiroError>;

/// Gap left between the existing content and a screenshot dropped below it, and
/// between two screenshots side by side. Wide enough that they read as separate.
const GAP: f64 = 200.0;

#[derive(Debug, Clone, Default)]
pub struct ScreenshotOptions {
    /// Symbol to draw: `Name` or `Contract.Name`.
    pub name: Option<String>,
    /// Explicit source range instead of a name.
    pub file: Option<String>,
    /// `start-end`, 1-based and inclusive.
    pub lines: Option<String>,
    /// The deployment to draw into: the entry point it was deployed for.
    pub deployment: Option<String>,
    /// Which frame of that deployment: a dependency's name. Omitted means the
    /// deployment's own root frame.
    pub dependency: Option<String>,
    /// Include the declaration's NatSpec, as `deploy --with-documentation` does.
    pub with_documentation: bool,
    /// Grow the frame when the screenshot does not fit in it. Off by default: the frame's
    /// size and position are the auditor's arrangement, not ours.
    pub grow: bool,
}

/// Where a named symbol lives.
#[derive(Debug)]
struct Located {
    label: String,
    kind: String,
    file_path: String,
    start: usize,
    end: usize,
}

pub async fn run(options: ScreenshotOptions) -> Result<()> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;

    // No frame named: say which ones exist rather than guessing. This is also how an
    // assistant discovers the frame names without being told them.
    let Some(deployment) = options.deployment.clone() else {
        return list_frames(&metadata);
    };
    let record = resolve_frame(&metadata, &deployment, options.dependency.as_deref())?;
    let frame_name = record.entry_point.clone();

    let located = locate(&metadata, &options)?;

    // Already there: drawing it again would stack an identical copy on the frame.
    if record
        .screenshots
        .iter()
        .any(|shot| shot.label == located.label)
    {
        println!(
            "  {} {} is already on {} — nothing to do",
            "note:".yellow(),
            located.label.clone().bold(),
            record.entry_point
        );
        println!("  {}", record.frame_url.blue());
        return Ok(());
    }

    // The NatSpec above a declaration is part of what explains it, so the same flag that
    // deploy uses applies here.
    let start = if options.with_documentation {
        natspec_start(&located.file_path, located.start)
    } else {
        located.start
    };

    let (png_path, png_width, png_height) = render(&located, start)?;

    let client = MiroClient::new_refreshed()
        .await
        .change_context(EvmMiroError)?;
    // A frame that was cut and pasted has a new id; the record is re-anchored to the copy
    // on the board rather than failing (see `ensure_frame_record`).
    let record = crate::batbelt::evm::miro::auto_deploy::ensure_frame_record(
        &record.entry_point,
        &client,
    )
    .await
    .change_context(EvmMiroError)?
    .ok_or_else(|| {
        Report::new(EvmMiroError)
            .attach_printable(format!(
                "the frame for `{frame_name}` is no longer on the board"
            ))
            .attach(crate::Suggestion(
                "deploy it again with `bat-cli deploy --entry-point <name>`, or point the \
                 record at the right frame with `bat-cli relink <name> --frame-url <url>`"
                    .to_string(),
            ))
    })?;
    let live = client.item_geometry(&record.frame_id).await;

    // Work from where the frame IS, not where the deploy put it: the auditor rearranges the
    // board, and drawing from the recorded position both misplaced the screenshot and, when
    // the frame had to grow, dragged the frame back to its birthplace.
    let mut record = record;
    if let Some((x, y, width, height)) = live {
        record.x = x;
        record.y = y;
        record.width = width;
        record.height = height;
    }

    // What the frame REALLY holds. The registry stores each screenshot's PNG size, but a
    // deep node is drawn scaled down and the auditor moves things, so placing from the
    // registry overestimated the content and pushed every new drawing far below it — which
    // is what made each addition stretch the frame by a screenful.
    let occupied = client
        .frame_children(
            &record.frame_id,
            (record.x, record.y, record.width, record.height),
        )
        .await
        .map(|children| {
            children
                .iter()
                .map(|child| (child.x, child.y, child.width, child.height))
                .collect()
        })
        .unwrap_or_else(|_| recorded_rects(&record));

    let width = png_width as f64 * BOARD_UNITS_PER_PIXEL;
    let height = png_height as f64 * BOARD_UNITS_PER_PIXEL;
    let (x, y) = free_spot(&occupied, record.width, record.height, width, height);
    let needed = y + height / 2.0 + GAP;

    // The frame is left exactly as it is unless `--grow` asks otherwise. Its size and
    // position are the auditor's arrangement — a frame is dragged where it belongs in the
    // reading — and resizing it (or worse, resizing it from the position the deploy
    // recorded) silently undoes that. A screenshot with nowhere to go lands in the corner
    // instead, overlapping; that is visible and one drag away.
    if needed > record.height && options.grow {
        // Growing keeps the TOP-LEFT fixed: frame-local coordinates are measured from it,
        // so extending downwards without moving the centre would shift every child up.
        let delta = needed - record.height;
        record.height += delta;
        record.y += delta / 2.0;
        client
            .update_frame(
                &record.frame_id,
                &format!("auto: {}", record.entry_point),
                record.x,
                record.y,
                record.width,
                record.height,
            )
            .await
            .change_context(EvmMiroError)?;
    }

    let item_id = client
        .create_image_in_frame(&png_path, &record.frame_id, &located.label, x, y, width)
        .await
        .change_context(EvmMiroError)?;
    silicon::delete_png_file(png_path);

    record.screenshots.push(ExtraScreenshot {
        label: located.label.clone(),
        item_id,
        x,
        y,
        width,
        height,
    });
    save_frame_record(&record)?;

    println!(
        "{} {} ({}) on {}",
        "✓".green(),
        located.label.bold(),
        located.kind,
        record.entry_point
    );
    println!(
        "  {}:{}-{}",
        located.file_path, located.start, located.end
    );
    println!("  {}", record.frame_url.blue());
    Ok(())
}

/// Which frame to draw into: a deployment, and a dependency inside it.
///
/// A frame's title is not an address any more. Every deploy is fresh, so a helper two
/// entry points reach is drawn once per deployment and several frames on the board
/// share a title — by design. What IS unique is a name inside one deployment: a
/// deployment draws one frame per function, so `--deployment FLAMM.previewMint
/// --dependency FLAMMFlowLib.requireFlat` names exactly one frame, and no `--dependency`
/// means the deployment's own root frame.
fn resolve_frame(
    metadata: &EvmBatMetadata,
    deployment: &str,
    dependency: Option<&str>,
) -> Result<AutoDeployedFrame> {
    let frames = &metadata.miro.auto.frames;
    let in_deployment: Vec<&AutoDeployedFrame> = frames
        .iter()
        .filter(|record| record.cluster_root == deployment)
        .collect();
    if in_deployment.is_empty() {
        let mut deployments: Vec<&str> = frames.iter().map(|r| r.cluster_root.as_str()).collect();
        deployments.sort_unstable();
        deployments.dedup();
        return Err(Report::new(EvmMiroError)
            .attach_printable(format!(
                "nothing is deployed for `{deployment}`. Deployed: {}",
                if deployments.is_empty() { "nothing yet".to_string() } else { deployments.join(", ") }
            ))
            .attach(crate::Suggestion(
                "deploy it first: `bat-cli deploy --entry-point <name>`".to_string(),
            )));
    }

    // No dependency named: the deployment's own frame, the one it was deployed for.
    let wanted = dependency.unwrap_or(deployment);
    if let Some(exact) = in_deployment.iter().find(|record| record.entry_point == wanted) {
        return Ok((*exact).clone());
    }
    // An overloaded function carries its signature — `MMRouterLib.read(uint256,address)` —
    // so that two frames for one name can be told apart. Nobody should have to type that
    // when there is only one: the bare name is matched against the part before the `(`,
    // and only a name that really was drawn twice asks for the signature.
    let by_name: Vec<&&AutoDeployedFrame> = in_deployment
        .iter()
        .filter(|record| record.entry_point.split('(').next() == Some(wanted))
        .collect();
    match by_name.len() {
        1 => return Ok((**by_name[0]).clone()),
        n if n > 1 => {
            let candidates = by_name
                .iter()
                .map(|record| format!("    {}", record.entry_point))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(Report::new(EvmMiroError)
                .attach_printable(format!(
                    "`{wanted}` is overloaded in this deployment:\n{candidates}"
                ))
                .attach(crate::Suggestion(
                    "name it with its parameter types, as printed above".to_string(),
                )));
        }
        _ => {}
    }
    in_deployment
        .iter()
        .find(|record| record.entry_point == wanted)
        .map(|record| (*record).clone())
        .ok_or_else(|| {
            let mut names: Vec<&str> = in_deployment.iter().map(|r| r.entry_point.as_str()).collect();
            names.sort_unstable();
            Report::new(EvmMiroError)
                .attach_printable(format!(
                    "the deployment of `{deployment}` has no frame for `{wanted}`. It drew:\n    {}",
                    names.join("\n    ")
                ))
                .attach(crate::Suggestion(
                    "a dependency drawn INSIDE the root frame has no frame of its own; only a \
                     branch cut out to its own frame does"
                        .to_string(),
                ))
        })
}

fn list_frames(metadata: &EvmBatMetadata) -> Result<()> {
    if metadata.miro.auto.frames.is_empty() {
        return Err(Report::new(EvmMiroError)
            .attach_printable("nothing is deployed yet")
            .attach(crate::Suggestion(
                "deploy an entry point first: `bat-cli deploy --entry-point <name>`".to_string(),
            )));
    }
    // Grouped by deployment, because that is what a frame belongs to: one entry point
    // deployed, every frame its cluster drew underneath it.
    let mut by_deployment: std::collections::BTreeMap<&str, Vec<&str>> =
        std::collections::BTreeMap::new();
    for frame in &metadata.miro.auto.frames {
        by_deployment
            .entry(frame.cluster_root.as_str())
            .or_default()
            .push(frame.entry_point.as_str());
    }
    println!("{}", "deployed frames, by deployment".bold());
    for (deployment, mut frames) in by_deployment {
        frames.sort_unstable();
        println!("\n  {}", deployment.bold());
        for frame in frames {
            println!("    {frame}");
        }
    }
    println!(
        "\n  bat-cli screenshot <symbol> --deployment {} [--dependency {}]",
        "<deployment>".yellow(),
        "<frame>".yellow()
    );
    Ok(())
}

/// Resolve what to draw: an explicit range wins, otherwise a name is looked up.
fn locate(metadata: &EvmBatMetadata, options: &ScreenshotOptions) -> Result<Located> {
    if let (Some(file), Some(lines)) = (&options.file, &options.lines) {
        let (start, end) = parse_range(lines)?;
        return Ok(Located {
            label: format!("{file}:{lines}"),
            kind: "range".to_string(),
            file_path: file.clone(),
            start,
            end,
        });
    }
    if options.file.is_some() || options.lines.is_some() {
        return Err(Report::new(EvmMiroError)
            .attach_printable("--file and --lines go together")
            .attach(crate::Suggestion(
                "bat-cli screenshot --file src/Vault.sol --lines 40-58 --frame <entry point>"
                    .to_string(),
            )));
    }

    let Some(name) = &options.name else {
        return Err(Report::new(EvmMiroError)
            .attach_printable("nothing to draw")
            .attach(crate::Suggestion(
                "name a symbol, or give --file and --lines".to_string(),
            )));
    };

    let mut matches = find_declaration(metadata, name);
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(Report::new(EvmMiroError)
            .attach_printable(format!("no declaration named `{name}`"))
            .attach(crate::Suggestion(format!(
                "draw it by range: --file <path> --lines <start>-<end>{}",
                if crate::guide::scanned_by_this_binary() {
                    ""
                } else {
                    " (this project was scanned by an older bat-cli; `bat-cli sonar` may index it)"
                }
            )))),
        // Several contracts declare the same name — `Data` and `Node` are common. Say
        // which, rather than silently drawing whichever came first.
        _ => {
            let candidates = matches
                .iter()
                .map(|found| format!("{} ({})", found.label, found.kind))
                .collect::<Vec<_>>()
                .join("\n    ");
            Err(Report::new(EvmMiroError)
                .attach_printable(format!("`{name}` is declared in several places:\n    {candidates}"))
                .attach(crate::Suggestion(
                    "name it as `Contract.Symbol`".to_string(),
                )))
        }
    }
}

/// Every declaration matching `name`, bare or qualified as `Owner.Name`.
fn find_declaration(metadata: &EvmBatMetadata, name: &str) -> Vec<Located> {
    let (wanted_owner, wanted) = match name.split_once('.') {
        Some((owner, symbol)) => (Some(owner), symbol),
        None => (None, name),
    };
    let mut found = Vec::new();

    // Structs, enums and the other indexed declarations. Since 0.24 this includes the
    // ones declared inside a contract, which is where Solidity puts most of them.
    for item in &metadata.file_items {
        if item.name != wanted {
            continue;
        }
        if let Some(owner) = wanted_owner {
            if item.owner != owner {
                continue;
            }
        }
        found.push(Located {
            label: qualified(&item.owner, &item.name),
            kind: item.kind.to_string(),
            file_path: item.file_path.clone(),
            start: item.line,
            end: item.end_line,
        });
    }

    // State variables, including `constant` and `immutable` — they are flags on the same
    // record, so they need no separate lookup.
    for contract in &metadata.contracts {
        if let Some(owner) = wanted_owner {
            if contract.name != owner {
                continue;
            }
        }
        for variable in &contract.state_variables {
            if variable.name != wanted {
                continue;
            }
            let (start, end) = declaration_span(&contract.file_path, variable.line);
            found.push(Located {
                label: qualified(&contract.name, &variable.name),
                kind: if variable.is_constant {
                    "constant".to_string()
                } else if variable.is_immutable {
                    "immutable".to_string()
                } else {
                    "state".to_string()
                },
                file_path: contract.file_path.clone(),
                start,
                end,
            });
        }

        // Functions, modifiers and events. A function is normally read as a node in a
        // deployed graph, but a bodiless one never is: an interface's `market(...)`
        // declaration is exactly what an auditor wants beside the call that crosses that
        // boundary, and `deploy` draws the boundary without ever showing the signature.
        for function in &contract.functions {
            if function.name != wanted {
                continue;
            }
            found.push(Located {
                label: qualified(&contract.name, &function.name),
                kind: if contract.contract_type == EvmContractType::Interface {
                    "interface fn".to_string()
                } else {
                    "fn".to_string()
                },
                file_path: contract.file_path.clone(),
                start: function.line,
                end: if function.end_line >= function.line {
                    function.end_line
                } else {
                    function.line
                },
            });
        }
        for modifier in &contract.modifiers {
            if modifier.name != wanted {
                continue;
            }
            found.push(Located {
                label: qualified(&contract.name, &modifier.name),
                kind: "modifier".to_string(),
                file_path: contract.file_path.clone(),
                start: modifier.line,
                end: if modifier.end_line >= modifier.line {
                    modifier.end_line
                } else {
                    modifier.line
                },
            });
        }
        for event in &contract.events {
            if event.name != wanted {
                continue;
            }
            let (start, end) = declaration_span(&contract.file_path, event.line);
            found.push(Located {
                label: qualified(&contract.name, &event.name),
                kind: "event".to_string(),
                file_path: contract.file_path.clone(),
                start,
                end,
            });
        }
    }
    found
}

fn qualified(owner: &str, name: &str) -> String {
    if owner.is_empty() {
        name.to_string()
    } else {
        format!("{owner}.{name}")
    }
}

/// The full span of a state variable declaration.
///
/// The scan records the line of the IDENTIFIER, so a declaration wrapped across lines
/// (a long `mapping(...)` type on its own line, say) points into the middle of itself.
/// Walk back to the start of the statement and forward to its `;` — the semicolon
/// analogue of the brace scan `function_end` does for functions.
fn declaration_span(file_path: &str, identifier_line: usize) -> (usize, usize) {
    let content = std::fs::read_to_string(file_path).unwrap_or_default();
    let lines: Vec<&str> = content.lines().collect();
    if identifier_line == 0 || identifier_line > lines.len() {
        return (identifier_line, identifier_line);
    }

    // Back to the first line of the statement: keep walking while the line above is a
    // continuation, i.e. it neither closes a statement nor opens/closes a block.
    let mut start = identifier_line;
    while start > 1 {
        let above = lines[start - 2].trim();
        if above.is_empty()
            || above.ends_with(';')
            || above.ends_with('{')
            || above.ends_with('}')
            || above.starts_with("//")
            || above.starts_with('*')
            || above.starts_with("/*")
        {
            break;
        }
        start -= 1;
    }

    // Forward to the terminating semicolon.
    let mut end = identifier_line;
    while end < lines.len() && !lines[end - 1].contains(';') {
        end += 1;
    }
    (start, end)
}

fn parse_range(range: &str) -> Result<(usize, usize)> {
    let bad = || {
        Report::new(EvmMiroError)
            .attach_printable(format!("cannot read `{range}` as a line range"))
            .attach(crate::Suggestion("use --lines 40-58".to_string()))
    };
    let (start, end) = range.split_once('-').ok_or_else(bad)?;
    let start: usize = start.trim().parse().map_err(|_| bad())?;
    let end: usize = end.trim().parse().map_err(|_| bad())?;
    if start == 0 || end < start {
        return Err(bad());
    }
    Ok((start, end))
}

/// Render the slice the way `deploy` renders a function, so the two are indistinguishable
/// on the board: a `// path` header, the true line numbers in the gutter, one reference
/// font size.
fn render(located: &Located, start: usize) -> Result<(String, u32, u32)> {
    let code = read_slice(&located.file_path, start, located.end);
    if code.is_empty() {
        return Err(Report::new(EvmMiroError).attach_printable(format!(
            "{}:{}-{} is empty — is the metadata stale?",
            located.file_path, start, located.end
        )));
    }
    let pretty = crate::batbelt::path::prettify_source_code_path(&located.file_path)
        .unwrap_or_else(|_| located.file_path.clone());
    let mut lines = vec![format!("// {pretty}"), String::new()];
    lines.extend(code);

    let destination = BatFolder::Figures
        .get_path(false)
        .change_context(EvmMiroError)?;
    std::fs::create_dir_all(&destination)
        .into_report()
        .change_context(EvmMiroError)?;
    let file_name = format!(
        "shot_{}_{}_{}.js",
        located.file_path.replace([':', '.', '/'], "_"),
        start,
        located.end
    );
    let png_path = silicon::create_figure(
        &lines.join("\n"),
        &destination,
        &file_name,
        start.saturating_sub(PATH_HEADER_LINES),
        Some(REFERENCE_FONT),
        true,
    );
    let (width, height) = image::image_dimensions(&png_path)
        .into_report()
        .change_context(EvmMiroError)?;
    Ok((png_path, width, height))
}

/// A spot inside the frame that sits on top of nothing.
///
/// The occupied rectangles come from the record rather than the board: there is no API to
/// list a frame's children, and `node_positions` + `image_dims` are enough. The stored
/// dimensions are raw pixels while a deep node is drawn scaled DOWN, so the rectangles are
/// over-estimates — which is exactly the safe direction for "don't land on anything".
///
/// Screenshots go in a band below the content, left to right, wrapping. That is out of the
/// way of the call flow (which runs left to right across the layers) and predictable; the
/// auditor drags it wherever they actually want it.
fn free_spot(
    occupied: &[(f64, f64, f64, f64)],
    frame_width: f64,
    frame_height: f64,
    width: f64,
    height: f64,
) -> (f64, f64) {
    let content_bottom = occupied
        .iter()
        .map(|(_, y, _, h)| y + h / 2.0)
        .fold(0.0_f64, f64::max);
    let left = occupied
        .iter()
        .map(|(x, _, w, _)| x - w / 2.0)
        .fold(f64::MAX, f64::min);
    let left = if left == f64::MAX { GAP } else { left.max(GAP) };

    // Walk left to right along the band, dropping down a row when the frame runs out.
    let mut x = left + width / 2.0;
    let mut y = content_bottom + GAP + height / 2.0;
    while y + height / 2.0 + GAP <= frame_height {
        let candidate = (x, y, width, height);
        if !occupied.iter().any(|rect| overlaps(*rect, candidate)) {
            return (x, y);
        }
        x += width + GAP;
        if x + width / 2.0 > frame_width {
            x = left + width / 2.0;
            y += height + GAP;
        }
    }

    // Nothing free left. Drop it in the bottom-left corner, on top of whatever is there:
    // overlapping something is easy to see and one drag away, whereas resizing the frame
    // silently undoes the arrangement the auditor built, and refusing to draw leaves them
    // with nothing.
    (
        width / 2.0 + GAP,
        (frame_height - height / 2.0 - GAP).max(height / 2.0),
    )
}

/// The frame's contents as the registry remembers them — the fallback when the board cannot
/// be asked. PNG pixels overstate a scaled-down node, which is the safe direction here.
fn recorded_rects(record: &AutoDeployedFrame) -> Vec<(f64, f64, f64, f64)> {
    let dims: std::collections::HashMap<&str, (u32, u32)> = record
        .image_dims
        .iter()
        .map(|(id, w, h)| (id.as_str(), (*w, *h)))
        .collect();
    let mut rects: Vec<(f64, f64, f64, f64)> = Vec::new();
    for (id, x, y) in &record.node_positions {
        if let Some((w, h)) = dims.get(id.as_str()) {
            rects.push((*x, *y, *w as f64, *h as f64));
        }
    }
    for shot in &record.screenshots {
        rects.push((shot.x, shot.y, shot.width, shot.height));
    }
    rects
}

fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    (ax - bx).abs() * 2.0 < aw + bw && (ay - by).abs() * 2.0 < ah + bh
}

#[cfg(test)]
mod screenshot_test {
    use super::*;
    use crate::batbelt::evm::types::{EvmFileItem, EvmFileItemKind, EvmVisibility, StorageVariable};

    fn sol_fixture(name: &str, body: &str) -> String {
        let dir = std::env::temp_dir().join(format!("bat-cli-shot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.sol"));
        std::fs::write(&path, body).unwrap();
        path.to_string_lossy().to_string()
    }

    fn item(name: &str, owner: &str) -> EvmFileItem {
        EvmFileItem {
            name: name.to_string(),
            kind: EvmFileItemKind::Struct,
            file_path: "./src/A.sol".to_string(),
            line: 10,
            end_line: 14,
            external: false,
            owner: owner.to_string(),
        }
    }

    fn variable(name: &str, is_immutable: bool) -> StorageVariable {
        StorageVariable {
            name: name.to_string(),
            type_name: "uint256".to_string(),
            visibility: EvmVisibility::Public,
            is_constant: false,
            is_immutable,
            line: 7,
        }
    }

    fn metadata_with(items: Vec<EvmFileItem>, contracts: Vec<(&str, Vec<StorageVariable>)>) -> EvmBatMetadata {
        let mut metadata = EvmBatMetadata::default();
        metadata.file_items = items;
        metadata.contracts = contracts
            .into_iter()
            .map(|(name, vars)| crate::batbelt::evm::metadata::bat_metadata::ContractMetadata {
                metadata_id: String::new(),
                name: name.to_string(),
                file_path: "./src/A.sol".to_string(),
                contract_type: crate::batbelt::evm::types::EvmContractType::Contract,
                base_contracts: vec![],
                functions: vec![],
                state_variables: vars,
                events: vec![],
                modifiers: vec![],
                line: 1,
                external: false,
            })
            .collect();
        metadata
    }

    #[test]
    fn finds_a_struct_declared_inside_a_contract() {
        // The case the index used to miss entirely: Solidity puts most structs here.
        let metadata = metadata_with(vec![item("FeedType", "PriceFeed")], vec![]);
        let found = find_declaration(&metadata, "FeedType");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "PriceFeed.FeedType");

        // And the qualified form resolves to the same one.
        assert_eq!(find_declaration(&metadata, "PriceFeed.FeedType").len(), 1);
        // A wrong owner matches nothing rather than falling back to the bare name.
        assert!(find_declaration(&metadata, "Other.FeedType").is_empty());
    }

    #[test]
    fn finds_a_state_variable_and_names_its_flavour() {
        let metadata = metadata_with(
            vec![],
            vec![("PriceFeed", vec![variable("deviationThresholdWad", true)])],
        );
        let found = find_declaration(&metadata, "deviationThresholdWad");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].label, "PriceFeed.deviationThresholdWad");
        assert_eq!(found[0].kind, "immutable");
    }

    #[test]
    fn a_name_declared_twice_is_reported_not_guessed() {
        // `Data` and `Node` really are declared in several contracts; taking the first
        // silently is the bug --entry-point still has.
        let metadata = metadata_with(
            vec![item("Data", "SortedPositions"), item("Data", "PriceFeed")],
            vec![],
        );
        assert_eq!(find_declaration(&metadata, "Data").len(), 2);

        let options = ScreenshotOptions {
            name: Some("Data".to_string()),
            ..Default::default()
        };
        let error = format!("{:?}", locate(&metadata, &options).unwrap_err());
        assert!(error.contains("SortedPositions.Data") && error.contains("PriceFeed.Data"));
    }

    #[test]
    fn spans_a_declaration_wrapped_across_lines() {
        // The scan stores the line of the IDENTIFIER, which for a wrapped declaration is
        // not the line the declaration starts on.
        let path = sol_fixture(
            "wrapped",
            "contract C {\n    uint256 public simple;\n\n    mapping(address => uint256)\n        public wrapped;\n}\n",
        );
        assert_eq!(declaration_span(&path, 2), (2, 2)); // single line, unchanged
        assert_eq!(declaration_span(&path, 5), (4, 5)); // identifier on 5, starts on 4
    }

    #[test]
    fn reads_a_line_range_or_says_why_not() {
        assert_eq!(parse_range("40-58").unwrap(), (40, 58));
        assert_eq!(parse_range(" 3 - 9 ").unwrap(), (3, 9));
        for bad in ["", "40", "0-5", "58-40", "a-b"] {
            assert!(parse_range(bad).is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn a_range_needs_both_halves() {
        let metadata = EvmBatMetadata::default();
        let options = ScreenshotOptions {
            file: Some("src/A.sol".to_string()),
            ..Default::default()
        };
        assert!(locate(&metadata, &options).is_err());
    }

    #[test]
    fn free_spot_lands_below_the_content_and_never_on_it() {
        let mut record = AutoDeployedFrame {
            entry_point: "C.f".to_string(),
            frame_id: String::new(),
            frame_url: String::new(),
            x: 0.0,
            y: 0.0,
            width: 4000.0,
            height: 2000.0,
            images: vec![],
            image_dims: vec![("n".to_string(), 1000, 400)],
            node_positions: vec![("n".to_string(), 700.0, 500.0)],
            callee_connectors: vec![],
            link_cards: vec![],
            connector_ids: vec![],
            marker_ids: vec![],
            border_ids: vec![],
            screenshots: vec![],
            cluster_root: String::new(),
        };

        let occupied = recorded_rects(&record);
        let (x, y) = free_spot(&occupied, record.width, record.height, 800.0, 300.0);
        assert!(!overlaps((700.0, 500.0, 1000.0, 400.0), (x, y, 800.0, 300.0)));
        assert!(y > 700.0, "must sit below the content, not beside it");

        // A second one avoids the first as well.
        record.screenshots.push(ExtraScreenshot {
            label: "a".to_string(),
            item_id: String::new(),
            x,
            y,
            width: 800.0,
            height: 300.0,
        });
        let occupied = recorded_rects(&record);
        let (x2, y2) = free_spot(&occupied, record.width, record.height, 800.0, 300.0);
        assert!(!overlaps((x, y, 800.0, 300.0), (x2, y2, 800.0, 300.0)));

        // A frame with no room left drops the drawing in the bottom-left corner rather than
        // resizing the auditor's frame or refusing to draw.
        let (cx, cy) = free_spot(&occupied, record.width, 900.0, 800.0, 300.0);
        assert!(cx < record.width / 2.0 && cy > 900.0 / 2.0);
    }
}
