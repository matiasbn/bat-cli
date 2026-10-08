//! One frame per contract: where its storage actually lives, and who moves it.
//!
//! A contract's source says what it declares. It does not say what it *occupies* — the slots
//! a base contract took before it, which two variables share a slot, or that the dozen
//! `immutable`s reading like state are in the bytecode and occupy nothing. `YieldRouter`
//! declares four mutable variables and its slots 0, 1 and 2 belong to `TwoStepOwned` and to
//! the reentrancy guard.
//!
//! The slot numbers are **not computed here**. Packing, dynamic types, inherited layout and
//! ERC-7201 base-slot hashing are a week of work and wrong at the edges, and every Foundry
//! repo already ships the answer: `forge inspect <C> storage-layout --json`. This consumes it
//! and draws it next to the code it belongs to.
//!
//! What this tool adds on top of `forge` is the two things it cannot know: which source line
//! each slot is declared on, and which functions write it.

use colored::Colorize;
use error_stack::{Report, Result, ResultExt};

use crate::batbelt::evm::metadata::bat_metadata::{ContractMetadata, EvmBatMetadata};
use crate::batbelt::evm::miro::auto_deploy::{beside_cluster, below_everything, REFERENCE_FONT};
use crate::batbelt::evm::miro::EvmMiroError;
use crate::batbelt::miro::client::{ArrowEnd, ConnectorStroke, ConnectorStyle, MiroClient};
use crate::batbelt::path::BatFolder;
use crate::batbelt::silicon;

/// Gap between two columns, and the margin around them, in board units.
const COLUMN_GAP: f64 = 600.0;
const MARGIN: f64 = 400.0;

pub struct StorageOptions {
    /// The contract whose storage to draw.
    pub contract: String,
    /// Print the table and stop, touching no board.
    pub dry_run: bool,
}

/// One entry of `forge`'s layout: a variable that occupies storage.
#[derive(Debug, Clone)]
struct Slot {
    slot: String,
    offset: u64,
    label: String,
    type_label: String,
    bytes: u64,
    /// The contract that DECLARED it. NOT forge's `contract` field, which names the
    /// compilation unit — it says `YieldRouter` for a slot `TwoStepOwned` declares, which is
    /// the one thing this column exists to tell you. Resolved from the metadata instead.
    declared_by: String,
    /// Where that declaration is, when it was found: its file and line.
    declared_at: Option<(String, usize)>,
}

/// A variable that reads like state and occupies no slot.
#[derive(Debug, Clone)]
struct Unstored {
    name: String,
    type_name: String,
    kind: &'static str,
    line: usize,
}

/// A function that writes one of the slots, and where.
#[derive(Debug, Clone)]
struct Writer {
    slot_label: String,
    function: String,
    path: String,
    line: usize,
    /// The lvalue as written, which is the field when the write goes through a pointer.
    lvalue: String,
}

pub async fn run(options: StorageOptions) -> Result<(), EvmMiroError> {
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let contract = pick_contract(&metadata, &options.contract)?;

    let mut slots = forge_layout(&contract.name)?;
    attribute(&metadata, contract, &mut slots);
    if slots.is_empty() {
        println!(
            "  {} {} occupies no storage slot at all",
            "note:".yellow(),
            contract.name
        );
    }
    let unstored = unstored_variables(&metadata, contract);
    let writers = writers_of(&metadata, contract, &slots);

    let (table, slot_rows) = slot_table(&contract.name, &slots, &writers);
    let sidebar = sidebar(&contract.name, &unstored);
    for line in table.iter().chain(sidebar.iter()) {
        println!("{line}");
    }

    if options.dry_run {
        return Ok(());
    }

    deploy(contract, &slots, &slot_rows, &table, &unstored).await
}

/// The contract this name means, refusing to guess.
///
/// `lib/` vendors several copies of the same library, so a name is not an identity (see
/// CLAUDE.md). With no caller to resolve against, the honest move is to prefer the ones in
/// scope and, when the name still names several, stop and list them.
fn pick_contract<'a>(
    metadata: &'a EvmBatMetadata,
    wanted: &str,
) -> Result<&'a ContractMetadata, EvmMiroError> {
    let matches: Vec<&ContractMetadata> = metadata
        .contracts
        .iter()
        .filter(|contract| contract.name == wanted && !contract.vendored)
        .collect();
    match matches.len() {
        1 => Ok(matches[0]),
        0 => Err(Report::new(EvmMiroError).attach_printable(format!(
            "no in-scope contract called `{wanted}`; run `bat-cli sonar` if the scan is stale"
        ))),
        _ => Err(Report::new(EvmMiroError).attach_printable(format!(
            "`{wanted}` names {} contracts in scope:\n    {}",
            matches.len(),
            matches
                .iter()
                .map(|contract| contract.file_path.clone())
                .collect::<Vec<_>>()
                .join("\n    ")
        ))),
    }
}

/// The layout `forge` computes, as it computes it.
///
/// Shelling out rather than reimplementing: the compiler owns the answer, and a second
/// implementation of slot assignment would only be a way to be wrong differently.
fn forge_layout(contract: &str) -> Result<Vec<Slot>, EvmMiroError> {
    let output = std::process::Command::new("forge")
        .args(["inspect", contract, "storage-layout", "--json"])
        .output()
        .map_err(|error| {
            Report::new(EvmMiroError).attach_printable(format!(
                "could not run `forge inspect {contract} storage-layout --json`: {error}"
            ))
        })?;
    if !output.status.success() {
        return Err(Report::new(EvmMiroError)
            .attach_printable(format!(
                "`forge inspect {contract} storage-layout --json` failed:\n{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))
            .attach(crate::Suggestion(
                "run it by hand from the Foundry root — the layout comes from the compiler, \
                 so the contract has to compile"
                    .to_string(),
            )));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| {
            Report::new(EvmMiroError)
                .attach_printable(format!("forge printed something that is not JSON: {error}"))
        })?;
    let types = &value["types"];
    let mut slots = Vec::new();
    for entry in value["storage"].as_array().into_iter().flatten() {
        let type_key = entry["type"].as_str().unwrap_or_default();
        let described = &types[type_key];
        slots.push(Slot {
            slot: entry["slot"].as_str().unwrap_or_default().to_string(),
            offset: entry["offset"].as_u64().unwrap_or_default(),
            label: entry["label"].as_str().unwrap_or_default().to_string(),
            type_label: described["label"]
                .as_str()
                .unwrap_or(type_key)
                .to_string(),
            bytes: described["numberOfBytes"]
                .as_str()
                .and_then(|n| n.parse().ok())
                .or_else(|| described["numberOfBytes"].as_u64())
                .unwrap_or_default(),
            declared_by: String::new(),
            declared_at: None,
        });
    }
    Ok(slots)
}

/// Say which contract of the chain declares each slot, and where.
///
/// This is the column the source cannot give you: `YieldRouter`'s first three slots belong
/// to `TwoStepOwned` and to the reentrancy guard, and nothing in its own file says so.
fn attribute(metadata: &EvmBatMetadata, contract: &ContractMetadata, slots: &mut [Slot]) {
    let chain = chain_of(metadata, contract);
    for slot in slots.iter_mut() {
        let found = chain.iter().find_map(|source| {
            source
                .state_variables
                .iter()
                .find(|variable| variable.name == slot.label)
                .map(|variable| (source, variable))
        });
        match found {
            Some((source, variable)) => {
                slot.declared_by = source.name.clone();
                slot.declared_at = Some((source.file_path.clone(), variable.line));
            }
            // Left blank rather than guessed: a slot the scan cannot place is a slot whose
            // declaring contract we do not know, and saying "this contract" would be wrong.
            None => slot.declared_by = "?".to_string(),
        }
    }
}

/// The `constant`s and `immutable`s of this contract and its bases.
///
/// They are in the frame on purpose: they read exactly like state in the source and occupy
/// nothing, and that is a thing to know before reasoning about an upgrade or a proxy.
fn unstored_variables(metadata: &EvmBatMetadata, contract: &ContractMetadata) -> Vec<Unstored> {
    let mut out = Vec::new();
    for source in chain_of(metadata, contract) {
        for variable in &source.state_variables {
            let kind = match (variable.is_constant, variable.is_immutable) {
                (true, _) => "constant",
                (_, true) => "immutable",
                _ => continue,
            };
            out.push(Unstored {
                name: variable.name.clone(),
                type_name: variable.type_name.clone(),
                kind,
                line: variable.line,
            });
        }
    }
    out
}

/// This contract and every base it inherits from, as far as the names resolve.
fn chain_of<'a>(
    metadata: &'a EvmBatMetadata,
    contract: &'a ContractMetadata,
) -> Vec<&'a ContractMetadata> {
    let mut out = vec![contract];
    let mut pending: Vec<String> = contract.base_contracts.clone();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    seen.insert(contract.name.clone());
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        // A base is resolved from the SAME file's import graph where one is available, since
        // a bare name is not an identity across `lib/`.
        let Some(base) = metadata
            .contract_in_scope(&name, &contract.file_path)
            .or_else(|| metadata.contracts.iter().find(|c| c.name == name))
        else {
            continue;
        };
        pending.extend(base.base_contracts.clone());
        out.push(base);
    }
    out
}

/// Every function of this contract or its bases that writes one of these slots.
///
/// Matched on the ROOT of the lvalue: `standing[id].lastRise` is a write to `standing`. A
/// write rooted at a storage POINTER local (`r.status`, where `r` was bound by
/// `Request storage r = requests[id]`) does not name a slot and is left out rather than
/// guessed at — that is the one gap, and it is named in the frame instead of hidden.
fn writers_of(
    metadata: &EvmBatMetadata,
    contract: &ContractMetadata,
    slots: &[Slot],
) -> Vec<Writer> {
    let labels: std::collections::HashSet<&str> =
        slots.iter().map(|slot| slot.label.as_str()).collect();
    let mut out = Vec::new();
    for source in chain_of(metadata, contract) {
        for function in &source.functions {
            for site in &function.storage_write_sites {
                let root = site
                    .name
                    .split(['.', '['])
                    .next()
                    .unwrap_or(&site.name)
                    .to_string();
                if !labels.contains(root.as_str()) {
                    continue;
                }
                out.push(Writer {
                    slot_label: root,
                    function: format!("{}.{}", source.name, function.name),
                    path: prettify(&source.file_path),
                    line: site.line,
                    lvalue: site.name.clone(),
                });
            }
        }
    }
    out.sort_by(|a, b| {
        (&a.slot_label, &a.function, a.line).cmp(&(&b.slot_label, &b.function, b.line))
    });
    out.dedup_by(|a, b| a.slot_label == b.slot_label && a.line == b.line && a.path == b.path);
    out
}

/// The slot table: one row per entry, in slot order, with that slot's writers under it.
///
/// The writers live HERE rather than in a column of their own, because "who moves this slot"
/// is a question about the slot — read on its own the list was a second index to cross-check
/// against the first. Two entries on one slot stay two rows with the same number, which is
/// what packing looks like.
///
/// Returns the lines and, for each slot, the ROW its header landed on, so an arrow can be
/// aimed at it: once the writers are interleaved, the n-th slot is no longer the n-th line.
fn slot_table(contract: &str, slots: &[Slot], writers: &[Writer]) -> (Vec<String>, Vec<usize>) {
    let mut lines = vec![
        format!("{contract} — storage layout ({} slot entries)", slots.len()),
        "from `forge inspect`; a repeated slot number is packing".to_string(),
        "beside it: a purple name is a constant, a red one an immutable — neither holds a slot"
            .to_string(),
        String::new(),
    ];
    let mut rows = Vec::new();
    if slots.is_empty() {
        lines.push("  this contract occupies no storage".to_string());
        return (lines, rows);
    }

    for slot in slots {
        let mine: Vec<&Writer> = writers
            .iter()
            .filter(|writer| writer.slot_label == slot.label)
            .collect();
        // The `[]` is kept when the writes go through an index: `paths[quote] = …` and
        // `paths = …` are not the same statement.
        let indexed = mine.iter().any(|writer| writer.lvalue.contains('['));
        // What the row does NOT carry any more: the size in bytes (the type says it), the
        // number of writers (they are listed right underneath) and the file and line of the
        // declaration (the arrow points at it, and the writers carry the path). Every one of
        // those pushed the end of the row — where the arrow leaves from — further away from
        // the name the arrow is about. What is left is the one thing nothing else says: that
        // a slot came from a BASE, and from which.
        let inherited = if slot.declared_by != contract && !slot.declared_by.is_empty() {
            format!(" ({})", slot.declared_by)
        } else {
            String::new()
        };
        // A blank line between groups: a slot and the functions that write it are one
        // block, and without the gap seven blocks read as one list.
        if !rows.is_empty() {
            lines.push(String::new());
        }
        rows.push(lines.len());
        // Read as a declaration — `address owner` — not as a padded table. Columns put the
        // type a tab-stop away from the name it belongs to, and the gap is where the eye
        // loses which variable it is on.
        lines.push(format!(
            "  slot {:>3}+{}  {} {}{}",
            slot.slot,
            slot.offset,
            slot.type_label,
            format!("{}{}", slot.label, if indexed { "[]" } else { "" }),
            inherited,
        ));
        let last = mine.len().saturating_sub(1);
        for (index, writer) in mine.iter().enumerate() {
            // What is left of the lvalue past the root — the field of a struct write. Empty
            // for a plain assignment, which is most of them.
            let field = writer
                .lvalue
                .trim_start_matches(&writer.slot_label)
                .trim_start_matches("[]")
                .to_string();
            // The same tree connectors `effects` draws with, for the same reason: the elbow
            // says which rows hang off this slot and the last one closes the group, so a
            // long table reads as groups instead of as one list with repeated indentation.
            let elbow = if index == last { "└─" } else { "├─" };
            lines.push(format!(
                "      {elbow} {:<44} {:<10} {}:{}",
                writer.function, field, writer.path, writer.line
            ));
        }
    }
    (lines, rows)
}

/// The second column: what reads like state and occupies no slot.
///
/// In the frame on purpose. A `constant` and an `immutable` are declared among the state
/// variables, look like them in every way, and are in the bytecode — which is what decides
/// whether an upgrade can change them.
fn sidebar(contract: &str, unstored: &[Unstored]) -> Vec<String> {
    let mut lines = vec![
        format!("{contract} — in the bytecode, not in storage ({})", unstored.len()),
        "declared among the state variables; occupies no slot".to_string(),
        String::new(),
    ];
    if unstored.is_empty() {
        lines.push("    none".to_string());
    }
    for variable in unstored {
        lines.push(format!(
            "    {:<10} {:<28} {:<24} L{}",
            variable.kind, variable.name, variable.type_name, variable.line
        ));
    }
    lines
}

fn prettify(path: &str) -> String {
    crate::batbelt::path::prettify_source_code_path(path).unwrap_or_else(|_| path.to_string())
}

/// Put the two columns on the board, with an arrow from every slot row to the line that
/// declares it.
///
/// The arrow is the point: a table of slots and a file of code are two things to hold in
/// your head at once, and the board is where they stop being two.
async fn deploy(
    contract: &ContractMetadata,
    slots: &[Slot],
    slot_rows: &[usize],
    table: &[String],
    unstored: &[Unstored],
) -> Result<(), EvmMiroError> {
    BatFolder::Figures.create_folder().change_context(EvmMiroError)?;
    let destination = BatFolder::Figures.get_path(true).change_context(EvmMiroError)?;

    let source = std::fs::read_to_string(&contract.file_path)
        .change_context(EvmMiroError)
        .attach_printable_lazy(|| format!("cannot read {}", contract.file_path))?;
    let all: Vec<String> = source.lines().map(|line| line.to_string()).collect();
    // The DECLARATION BLOCK, not the file: from the contract's first state variable to its
    // last, whole and uncut. Rendering the file instead would be a 900-line image to point
    // seven arrows into, and the lines between the declarations are kept because they are
    // the comments that say what each one is for.
    let Some((first, last)) = declaration_span(contract, all.len()) else {
        println!(
            "  {} {} declares no state variable of its own",
            "note:".yellow(),
            contract.name
        );
        return Ok(());
    };
    let declarations = &all[first - 1..last];

    let table_png = render(table, &destination, &format!("storage_{}_table", contract.name), 0, &[])?;
    let code_png = render(
        declarations,
        &destination,
        &format!("storage_{}_code", contract.name),
        // The gutter carries the REAL file lines, so a number on the board is a number in
        // the editor. silicon numbers the FIRST rendered line with this value, so it is the
        // span's first line and not the one before it.
        first,
        &unstored_marks(unstored),
    )?;

    // Two columns, not three. What occupies no slot used to be a list beside the code; it is
    // now marked ON the code, which is where the reader already is — a third column saying
    // the same thing is a second place to look.
    //
    // The CODE goes first and the table second. The arrows then run right to left and land
    // just past the end of each declaration, instead of arriving at the far left edge of a
    // wide block and leaving the reader to find which line they meant.
    let columns = [&code_png, &table_png];
    let width: f64 = columns.iter().map(|c| c.width).sum::<f64>()
        + COLUMN_GAP * (columns.len() as f64 - 1.0);
    let height: f64 = columns.iter().fold(0.0_f64, |tallest, c| tallest.max(c.height));
    let frame_width = width + MARGIN * 2.0;
    let frame_height = height + MARGIN * 2.0;

    let client = MiroClient::new_refreshed().await.change_context(EvmMiroError)?;
    let metadata = EvmBatMetadata::read_metadata().change_context(EvmMiroError)?;
    let board_frames = client.list_frames().await.change_context(EvmMiroError)?;
    // Beside this contract's own deployed cluster when it has one: the layout and the
    // diagram of the same contract belong within one glance of each other.
    let (frame_x, frame_y) = beside_cluster(
        &metadata,
        &contract.name,
        &board_frames,
        frame_width,
        frame_height,
    )
    .unwrap_or_else(|| {
        let (left, top) = below_everything(&board_frames);
        (left + frame_width / 2.0, top + frame_height / 2.0)
    });

    // A second run REPLACES the first: this frame is derived entirely from the source and
    // the compiler, so two of them side by side are not two views, they are one view and one
    // stale copy. Deleting the frame takes its contents with it.
    let title = format!("storage: {}", contract.name);
    for previous in board_frames.iter().filter(|frame| frame.title == title) {
        client
            .delete_item(&previous.id)
            .await
            .change_context(EvmMiroError)?;
        println!("  {} replaced the previous {title} frame", "↻".blue());
    }

    let frame_id = client
        .create_frame(
            &title,
            frame_x,
            frame_y,
            frame_width,
            frame_height,
            None,
        )
        .await
        .change_context(EvmMiroError)?;

    let mut cursor = MARGIN;
    let mut placed: Vec<(String, f64)> = Vec::new();
    for column in columns {
        let (x, y) = (cursor + column.width / 2.0, MARGIN + column.height / 2.0);
        let item = client
            .create_image_in_frame(&column.png_path, &frame_id, &contract.name, x, y, column.width)
            .await
            .change_context(EvmMiroError)?;
        placed.push((item, cursor));
        cursor += column.width + COLUMN_GAP;
        let _ = std::fs::remove_file(&column.png_path);
    }
    println!("    {} {} column(s) uploaded", "✓".green(), columns.len());

    let code_left = placed[0].1;
    let table_left = placed[1].1;
    let code_right = code_left + code_png.width;
    let geometry = silicon::line_geometry(Some(REFERENCE_FONT));

    // Which slots can be pointed at: the ones this file declares. A slot a base declares
    // lives in another file, which this frame does not draw, so it gets no arrow rather than
    // one landing on the wrong line.
    let aimed: Vec<(usize, &Slot, usize)> = slots
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| {
            let declared = contract
                .state_variables
                .iter()
                .find(|variable| variable.name == slot.label)?;
            Some((index, slot, declared.line))
        })
        .collect();

    // Each arrow through a LANE of its own, drawn by the deploy's own `draw_lane_arrow`:
    // three straight legs, nothing left for Miro to route. The lanes are ordered the way the
    // deploy orders them — the arrow that ENDS lowest takes the lane furthest out — so
    // arrows that need not cross do not.
    let mut aimed: Vec<(usize, usize)> = aimed
        .iter()
        .filter_map(|(index, _, declared_line)| Some((*slot_rows.get(*index)?, *declared_line)))
        .collect();
    aimed.sort_by(|a, b| a.1.cmp(&b.1));

    let gutter = (table_left - code_right).max(1.0);
    let margin = 100.0_f64.min(gutter / 4.0);
    let usable = (gutter - 2.0 * margin).max(0.0);
    let pitch = if aimed.len() > 1 {
        usable / (aimed.len() - 1) as f64
    } else {
        0.0
    };

    let mut drawn = 0usize;
    for (order, (row, declared_line)) in aimed.iter().enumerate() {
        // The same arithmetic the deploy anchors a call site with: the line's centre as a
        // fraction of the PNG's real height, so the shadow and the padding are accounted for
        // by the renderer rather than guessed at here.
        let row_y = MARGIN
            + table_png.height * geometry.line_center_fraction(*row, table_png.height as u32);
        let code_y = MARGIN
            + code_png.height
                * geometry.line_center_fraction(declared_line - first, code_png.height as u32);

        // The row is LEFT of the lane now, so the arrow leaves from the start of its text,
        // not the end: leaving from the end meant the first leg ran back across the row and
        // struck its own words through. The head still lands past the last character of the
        // declaration, which is the end the reader is looking for.
        let row_start = table_left
            + silicon::line_end_x(Some(REFERENCE_FONT), false, table.len(), 0, "") as f64;
        let declaration_end = code_left
            + silicon::line_end_x(
                Some(REFERENCE_FONT),
                true,
                declarations.len(),
                first,
                &declarations[declared_line - first],
            ) as f64;
        // Running right to left, the arrow whose declaration sits LOWEST takes the lane
        // nearest the table: every arrow below another then stops short of that one's lane
        // instead of crossing it. The deploy's rule, mirrored, because the direction is.
        let lane_x = code_right + margin + order as f64 * pitch;

        crate::batbelt::evm::miro::auto_deploy::draw_lane_arrow(
            &client,
            &frame_id,
            crate::batbelt::evm::miro::auto_deploy::LaneEnd::Point { x: row_start, y: row_y },
            // The head lands on the DECLARATION, because that is what the row is pointing
            // at — the deploy's arrows land on the call line for the same reason.
            crate::batbelt::evm::miro::auto_deploy::LaneEnd::Point {
                x: declaration_end,
                y: code_y,
            },
            lane_x,
            ConnectorStyle {
                stroke_color: silicon::BAT_PALETTE[order % silicon::BAT_PALETTE.len()].to_string(),
                stroke_width: "8".to_string(),
                stroke: ConnectorStroke::Solid,
                caption: None,
                arrow: ArrowEnd::End,
            },
        )
        .await
        .change_context(EvmMiroError)?;
        drawn += 1;
    }
    println!("    {} {drawn} slot(s) pointed at their declaration", "✓".green());
    println!("  {}", client.frame_url(&frame_id).blue());
    Ok(())
}

/// One rendered column.
struct Rendered {
    png_path: String,
    width: f64,
    height: f64,
}

/// The lines this contract's own declarations span, 1-based and inclusive.
fn declaration_span(contract: &ContractMetadata, file_lines: usize) -> Option<(usize, usize)> {
    let first = contract
        .state_variables
        .iter()
        .map(|variable| variable.line)
        .min()?;
    let last = contract
        .state_variables
        .iter()
        .map(|variable| variable.line)
        .max()?;
    (first <= file_lines).then(|| (first.max(1), last.min(file_lines)))
}

/// Mark every `constant` and `immutable` where it is declared: purple for a constant, red
/// for an immutable.
///
/// Both are in the bytecode and occupy no slot, and the difference between them is when they
/// are fixed — a constant at compile time, an immutable by the constructor. Marking them in
/// the code is what makes the slot table complete without a third column: a declaration with
/// no mark is a declaration that holds a slot.
fn unstored_marks(unstored: &[Unstored]) -> Vec<silicon::TracedName> {
    use silicon::{TraceKind, TracedName};
    // Indices into `TRACE_COLORS`: 0 is the salmon red, 2 the purple. A background rather
    // than a rule, because what is being said is a property of the name, not its register.
    const IMMUTABLE: usize = 0;
    const CONSTANT: usize = 2;
    unstored
        .iter()
        .map(|variable| TracedName {
            name: variable.name.clone(),
            kind: TraceKind::Parameter,
            color: if variable.kind == "constant" { CONSTANT } else { IMMUTABLE },
            dotted: false,
        })
        .collect()
}

fn render(
    lines: &[String],
    destination: &str,
    name: &str,
    line_offset: usize,
    traced: &[silicon::TracedName],
) -> Result<Rendered, EvmMiroError> {
    let png_path = silicon::create_figure_tracing(
        &lines.join("\n"),
        destination,
        // `.sol` so the declarations are highlighted as Solidity. The table keeps `.txt`,
        // which silicon falls back to Rust for — which is fine as long as every line PARSES:
        // a type cut short with an ellipsis left an unbalanced `(`, and the highlighter
        // stopped colouring from there on, so two rows of one table looked like two kinds of
        // thing. Nothing in this table is cut any more.
        &format!("{name}.{}", if line_offset > 0 { "sol" } else { "txt" }),
        line_offset,
        Some(REFERENCE_FONT),
        line_offset > 0,
        traced,
    );
    let (width, height) = image::image_dimensions(&png_path).change_context(EvmMiroError)?;
    Ok(Rendered {
        png_path,
        width: width as f64,
        height: height as f64,
    })
}

#[cfg(test)]
mod storage_test {
    use super::*;

    fn slot(label: &str, slot: &str, offset: u64, declared_by: &str) -> Slot {
        Slot {
            slot: slot.to_string(),
            offset,
            label: label.to_string(),
            type_label: "address".to_string(),
            bytes: 20,
            declared_by: declared_by.to_string(),
            declared_at: Some(("src/C.sol".to_string(), 7)),
        }
    }

    /// Two entries on one slot stay two rows: that repetition IS the packing.
    #[test]
    fn packing_is_two_rows_with_one_slot_number() {
        let slots = vec![slot("a", "0", 0, "C"), slot("b", "0", 20, "C")];
        let (table, rows) = slot_table("C", &slots, &[]);
        assert_eq!(rows.len(), 2, "one row per entry, both on slot 0");
        assert!(table.iter().any(|line| line.contains("slot   0+0")));
        assert!(table.iter().any(|line| line.contains("slot   0+20")));
    }

    /// A slot nothing writes has no rows under it.
    #[test]
    fn a_slot_with_no_writer_says_so() {
        let (table, _) = slot_table("C", &[slot("a", "0", 0, "C")], &[]);
        // Nothing writes it, so nothing hangs off its row — the row is the whole group.
        assert_eq!(table.iter().filter(|line| line.contains("├─") || line.contains("└─")).count(), 0);
    }

}
