//! Generates the AI guide — bat-cli-owned markdown that teaches an AI assistant how to drive
//! bat-cli without reading `--help` — plus the routers that point an assistant at it.
//!
//! Everything here is **machine-global**, under `<config_dir>/ai_context/` (so
//! `~/.config/bat-cli/ai_context/`, honouring `XDG_CONFIG_HOME` / `BAT_CLI_CONFIG_DIR`).
//! rover keeps its guide per project because it owns the folder it writes into; bat-cli does
//! not — its `Bat.toml` sits at the root of somebody else's repository, and dropping four
//! generated markdown files next to it is litter in a tree the auditor did not ask us to
//! touch. The guide also describes the *binary*, not the project, so one copy per machine is
//! the honest granularity: there is nothing per project to say.
//!
//! Regenerated idempotently on every command, so it always documents the installed binary.
//! These are generated files, never the auditor's own content — overwriting them is safe.
//!
//! The split within it is deliberate:
//!
//! - **The guide is version-stamped.** Written by the running binary, so it cannot drift from
//!   the code, and an assistant can see which version it is reading.
//! - **The routers are version-agnostic and byte-stable.** They carry no instructions: they
//!   only say where the guide lives. Because they never change, upgrading bat-cli never
//!   rewrites them, so a running assistant session never needs another restart.

use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use error_stack::{IntoReport, Result, ResultExt};

#[derive(Debug)]
pub struct GuideError;

impl std::fmt::Display for GuideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Guide error")
    }
}

impl std::error::Error for GuideError {}

pub type GuideResult<T> = Result<T, GuideError>;

/// The guide files, written under `<config_dir>/ai_context/`.
const FILES: &[(&str, &str)] = &[
    ("README.md", README),
    ("workflow.md", WORKFLOW),
    ("metadata.md", METADATA),
    ("changelog.md", CHANGELOG),
];

/// Markers delimiting bat-cli's **managed block** inside a foreign context file. Only the
/// text between them is ever rewritten; everything the user wrote outside is preserved.
const AGENTS_BEGIN: &str = "<!-- bat-cli:agents:begin -->";
const AGENTS_END: &str = "<!-- bat-cli:agents:end -->";

/// Writes `body` to `path` only when it differs from what is on disk, to avoid churn in the
/// audited repository's git tree. Returns whether it changed the file.
fn write_if_changed(path: &Path, body: &str) -> GuideResult<bool> {
    if fs::read_to_string(path)
        .map(|current| current == body)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    fs::write(path, body)
        .into_report()
        .change_context(GuideError)
        .attach_printable_lazy(|| format!("could not write {}", path.display()))?;
    Ok(true)
}

/// The one place the guide lives: `<config_dir>/ai_context/`, next to the machine-wide
/// preferences and credentials rather than inside any audited repository.
pub fn ai_context_dir() -> PathBuf {
    crate::config::global_config_dir().join("ai_context")
}

/// (Re)writes the AI guide, stamping the running version into each file. Only writes a file
/// when its bytes change. Returns whether anything did.
pub fn ensure_ai_guide_at(dir: &Path) -> GuideResult<bool> {
    fs::create_dir_all(dir)
        .into_report()
        .change_context(GuideError)
        .attach_printable_lazy(|| format!("could not create {}", dir.display()))?;
    let mut changed = false;
    for (name, body) in FILES {
        let body = body.replace("{BAT_CLI_VERSION}", env!("CARGO_PKG_VERSION"));
        changed |= write_if_changed(&dir.join(name), &body)?;
    }
    Ok(changed)
}

pub fn ensure_ai_guide() -> GuideResult<bool> {
    ensure_ai_guide_at(&ai_context_dir())
}

/// Writes bat-cli's managed block into `path`: replaces it in place when the markers are
/// there, appends it to an existing foreign file otherwise (keeping everything the user
/// wrote), or creates the file. Returns whether it changed anything.
fn write_managed_block(path: &Path, body: &str) -> GuideResult<bool> {
    let block = format!("{AGENTS_BEGIN}\n{body}{AGENTS_END}");
    let next = match fs::read_to_string(path) {
        // Our block is already there → rewrite only the region between the markers.
        Ok(current) if current.contains(AGENTS_BEGIN) && current.contains(AGENTS_END) => {
            let start = current.find(AGENTS_BEGIN).unwrap();
            let end = current.find(AGENTS_END).unwrap() + AGENTS_END.len();
            format!("{}{}{}", &current[..start], block, &current[end..])
        }
        // A file the user owns → append, keeping everything they wrote.
        Ok(current) if !current.trim().is_empty() => {
            let separator = if current.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{current}{separator}{block}\n")
        }
        // Absent or empty → the file is just our block.
        _ => format!("{block}\n"),
    };
    write_if_changed(path, &next)
}

fn home_dir() -> GuideResult<PathBuf> {
    let home = std::env::var("HOME")
        .into_report()
        .change_context(GuideError)
        .attach_printable("could not locate the home directory ($HOME is unset)")?;
    Ok(PathBuf::from(home))
}

/// Installs the **global**, version-agnostic AI-assistant routers, so every bat-cli project
/// is understood with no per-project setup and — after one first-time restart — no further
/// ones:
///
/// - **Claude Code**: `~/.claude/skills/bat-cli/SKILL.md`, auto-invoked via its `description`.
/// - **Codex**: `~/.agents/skills/bat-cli/SKILL.md`, a standalone skill rather than an
///   always-on `AGENTS.md` block, so bat-cli is selected by its description.
/// - **Gemini CLI**: a managed block in `~/.gemini/GEMINI.md`, appended to whatever is there.
///
/// Best-effort and idempotent. Returns `true` only the first time the global Claude skills
/// directory is created — the one case needing a one-off `claude --continue` to watch it.
pub fn ensure_global_ai_skills() -> GuideResult<bool> {
    ensure_global_ai_skills_at(&home_dir()?)
}

fn ensure_global_ai_skills_at(home: &Path) -> GuideResult<bool> {
    // A brand-new ~/.claude/skills directory needs a one-time restart to be watched; adding
    // into an existing one is picked up live.
    let skills_root = home.join(".claude").join("skills");
    let needs_restart = !skills_root.exists();
    let skill_dir = skills_root.join("bat-cli");
    fs::create_dir_all(&skill_dir)
        .into_report()
        .change_context(GuideError)
        .attach_printable_lazy(|| format!("could not create {}", skill_dir.display()))?;
    write_if_changed(&skill_dir.join("SKILL.md"), GLOBAL_SKILL_MD)?;

    let codex_skill_dir = home.join(".agents").join("skills").join("bat-cli");
    fs::create_dir_all(&codex_skill_dir)
        .into_report()
        .change_context(GuideError)
        .attach_printable_lazy(|| format!("could not create {}", codex_skill_dir.display()))?;
    write_if_changed(&codex_skill_dir.join("SKILL.md"), GLOBAL_SKILL_MD)?;

    let gemini_dir = home.join(".gemini");
    fs::create_dir_all(&gemini_dir)
        .into_report()
        .change_context(GuideError)
        .attach_printable_lazy(|| format!("could not create {}", gemini_dir.display()))?;
    write_managed_block(&gemini_dir.join("GEMINI.md"), GLOBAL_AGENTS_BODY)?;

    Ok(needs_restart)
}

/// Refresh step run by every command: regenerate the global guide, make sure the routers
/// exist, and stamp the running version into `Bat.toml` when there is one here.
///
/// Best-effort throughout — none of this may break a command that otherwise worked, so a
/// failure only logs. Prints the one-time restart hint when the global Claude skills
/// directory had to be created.
pub fn refresh_ai_surface() {
    match ensure_ai_guide() {
        Ok(true) => println!(
            "{} regenerated the AI guide in {}",
            "✓".green(),
            ai_context_dir().display()
        ),
        Ok(false) => {}
        Err(report) => log::debug!("could not regenerate the AI guide: {report:?}"),
    }

    match ensure_global_ai_skills() {
        Ok(true) => {
            println!("✓ Installed the bat-cli AI-assistant skill (`~/.claude/skills/bat-cli`, `~/.agents/skills/bat-cli`, `~/.gemini`).");
            println!("  Run `claude --continue` once to load it — first time only, not needed again.");
        }
        Ok(false) => {}
        Err(report) => log::debug!("could not install the global AI skills: {report:?}"),
    }

    record_bat_cli_version();
}

/// Print where the AI assistant integration lives and the one line a user can say
/// to their assistant to start driving bat-cli. Shown by `bat-cli refresh-ai-guide`
/// so, right after `cargo install`, the setup is discoverable in one command.
pub fn print_ai_setup_hint() {
    let home = home_dir().ok();
    let claude = home
        .as_ref()
        .map(|h| h.join(".claude/skills/bat-cli").display().to_string())
        .unwrap_or_else(|| "~/.claude/skills/bat-cli".to_string());
    println!("{} bat-cli AI assistant is set up.", "✓".green());
    println!("  skill:  {claude}");
    println!("          (Codex: ~/.agents/skills/bat-cli · Gemini: ~/.gemini/GEMINI.md)");
    println!("  guide:  {} (README, workflow, metadata, changelog)", ai_context_dir().display());
    println!(
        "\n  Tell your AI assistant: {} — it will read the guide and drive bat-cli for you.",
        "\"use the bat-cli skill\"".green()
    );
    println!(
        "  (If your assistant just installed the skill, restart it once — e.g. `claude --continue`.)"
    );
}

/// Stamps the running version into `Bat.toml` when one exists here and it differs from what
/// is stored. The guide itself is global, so this is not what tells an assistant the guide
/// moved — it records **which binary last scanned this project**, which is what says whether
/// `BatMetadata.json` came from the parser you are running now. Best-effort: no `Bat.toml`,
/// or a write failure, is silently nothing. Returns whether the stamp moved.
pub fn record_bat_cli_version() -> bool {
    let current = env!("CARGO_PKG_VERSION");
    let mut config = match crate::config::BatConfig::get_config() {
        Ok(config) => config,
        // No Bat.toml here — a global command like `config` or `update`. Nothing to stamp.
        Err(report) => {
            log::debug!("no Bat.toml to stamp: {report:?}");
            return false;
        }
    };
    if config.bat_cli_version == current {
        return false;
    }
    config.bat_cli_version = current.to_string();
    match config.save() {
        Ok(()) => true,
        Err(report) => {
            log::debug!("could not stamp bat_cli_version into Bat.toml: {report:?}");
            false
        }
    }
}

const README: &str = r##"<!-- Generated by bat-cli {BAT_CLI_VERSION} — do not edit; regenerated on every bat-cli command. -->
# bat-cli — guide for AI assistants

You are helping a security auditor on a **bat-cli** project. bat-cli parses the smart-contract
codebase in this repository into `BatMetadata.json`, and draws a function's call graph onto a
Miro board — every function rendered as a syntax-highlighted screenshot, laid out, uploaded
already positioned, with every arrow landing on the exact line that makes the call.

The auditor drives it by talking to you ("rescan the code", "deploy `Vault.deposit` to the
board", "which external functions have no access control?") rather than by reading `--help`.

Two files at the root of the audited repository are the whole project:

| file | holds |
|---|---|
| `Bat.toml` | project type, program/`src` paths, the Miro board URL, `bat_cli_version` |
| `BatMetadata.json` | the parsed codebase, and what has already been deployed |

Screenshots are rendered to the system temp directory and deleted after upload. bat-cli
creates **no branches and no commits**: version control is the auditor's business.

Open the guide that matches the task:

- `workflow.md` — the commands, their flags, what is interactive, and the failure modes.
- `metadata.md` — the `BatMetadata.json` schema and the `jq` recipes to query it.
- `changelog.md` — what is NEW per bat-cli version; read it when the version rises.

## This guide is machine-global, not per project

It lives once per machine, in `ai_context/` next to bat-cli's own config — NOT inside the
audited repository. That is deliberate: `Bat.toml` sits at the root of a repo the auditor does
not own, and these docs describe the **binary**, not the project, so there is nothing per
project for them to say. Every bat-cli command regenerates them, so they always document the
version that is installed right now.

The per-project stamp is a different thing: `Bat.toml`'s `bat_cli_version` records **which
binary last scanned this project**, i.e. whether `BatMetadata.json` came from the parser you
are running today. If it is behind `bat-cli --version`, the scan predates your binary — run
`bat-cli sonar` before trusting the metadata, since a newer parser can extract things the old
scan simply does not contain.

## Read this guide once; re-read it ONLY when bat-cli updates

These docs only change when bat-cli's version changes — so read them once and do not keep
re-reading them:

- Note the version in this file's header comment when you first load the guide.
- Before acting later, `bat-cli --version` is a cheap check. If it is HIGHER than the version
  stamped in these docs — bat-cli was updated and the next command will regenerate them —
  **read `changelog.md` FIRST**. Each version entry lists what is new AND a `Re-read:` line
  naming the docs that actually changed: re-open ONLY those (the union across every version
  above the one you last saw), not everything. If the version is the SAME, the guide is
  current — do NOT re-read it.

## Golden rules

1. **Run from the root of the audited repository**, where `Bat.toml` lives — never from the
   bat-cli source checkout.
2. **A stale scan lies.** `BatMetadata.json` carries line numbers; if the source changed since
   the last scan they point at the wrong lines. Re-run `bat-cli sonar` before trusting them,
   and always before a deploy.
3. **You cannot answer an interactive prompt.** `init`, `login`, `config --edit` and a bare
   `deploy` open `dialoguer` pickers. Hand those to the auditor (`! bat-cli init`) instead of
   launching them and hanging. See `workflow.md` for what is safe to run unattended.
4. **Prefer `--dry-run` while you are checking that a graph resolves.** It computes and prints
   the layout without contacting Miro, needs no login, and puts nothing on the board.
5. **Deploying is not free.** Every deploy uploads dozens of images and connectors to a shared
   board; Miro slows down past about a thousand objects. Deploy what the auditor asked for,
   one function at a time, and never pass `--all` on your own initiative.
6. **Query the metadata before grepping the source.** `metadata.md`'s recipes answer most
   structural questions (entry points, access control, storage, the call graph) in one read.
"##;

const WORKFLOW: &str = r##"<!-- Generated by bat-cli {BAT_CLI_VERSION} — do not edit; regenerated on every bat-cli command. -->
# bat-cli — commands and workflow

## The loop

```
init  ──▶  sonar  ──▶  deploy
(once)     (after every source change)     (per function, on demand)
```

`init` scans once as its last step, so a fresh project is ready to deploy. After that,
`sonar` is what keeps `BatMetadata.json` in step with the code, and `deploy` reads it.

Check the state before acting:

```bash
ls Bat.toml BatMetadata.json 2>/dev/null; bat-cli --version
```

- no `Bat.toml` → not initialized; `init` is the first step (the auditor runs it)
- `Bat.toml` but no `BatMetadata.json` → run `bat-cli sonar`
- source changed since the last scan → `bat-cli sonar` before anything else

## Which stack is this?

`Bat.toml`'s `project_type` is one of `Anchor`, `Pinocchio`, `VanillaSolana`, `Foundry`,
`GenericRust`, and it decides what is possible:

- **`Foundry`** (Solidity/EVM) — the complete path: scan, query, deploy.
- **the SVM types** (Anchor, Pinocchio, vanilla Rust) — `init` and `sonar` work and fill
  `BatMetadata.json`, but **`deploy` has no SVM path** and will fail. Say so plainly rather
  than trying flags; the metadata is still worth querying.

## Commands

| command | what it does | interactive? |
|---|---|---|
| `bat-cli init` | detect the framework, write `Bat.toml`, create/pick the Miro board, then scan | **yes** |
| `bat-cli sonar` | rescan the source, rebuild `BatMetadata.json` | no |
| `bat-cli deploy` | render a function's call graph and upload it to Miro | yes unless `--entry-point` |
| `bat-cli login` / `logout` | machine-wide Miro OAuth (`--setup`, `--status`, `--force`) | **yes** (browser) |
| `bat-cli config` | show the machine preferences (`--edit` re-answers them) | only with `--edit` |
| `bat-cli update` | install the latest crates.io version (`--check`, `--force`) | no |

Machine-wide state lives in `~/.config/bat-cli/` — `config.toml` (auditor name, code editor),
`miro.toml` (the OAuth credentials, `0600`) and `ai_context/` (this guide). Override the
directory with `XDG_CONFIG_HOME` or `BAT_CLI_CONFIG_DIR`. **Authorization is per machine, not
per project**: one `bat-cli login` covers every audit on the box, and only the board URL
belongs to the project.

**When this guide appears.** Not at `cargo install` — cargo runs nothing after it builds. The
first bat-cli command you run publishes it, along with the assistant skills, and every command
after that re-checks them. `bat-cli update` publishes the new version's guide itself, by asking
the binary it just installed to do it (the updating process is the outgoing version, so it
could not). `bat-cli refresh-ai-guide` forces the same thing on demand; it needs no project.

A **project** does not catch up on its own: after an update its `BatMetadata.json` still comes
from the old parser until `bat-cli sonar` runs inside it, and `Bat.toml`'s `bat_cli_version` is
what tells you so.

`-v` / `-vv` raise the `env_logger` level (logs go to stderr); `RUST_LOG` works too.

## Interactive prompts — you cannot answer them

`login`, `config --edit`, a bare `deploy`, and a bare `init` use `dialoguer` prompts (select,
multiselect, fuzzy-select, yes/no). Do not launch those and hope. Ask the auditor to run them
in-session with the `!` prefix:

> Run `! bat-cli login` and press Accept in the browser (it also prints the URL to paste).

**`init --yes` runs with NO prompt — you can run it yourself.** The project name is the folder
name, and the Miro board is resolved without asking: it reuses an existing board with the same
name as the folder, or creates one, or attaches `--board-url <URL>`. (A Foundry project already
derives `src` from `foundry.toml`; only a bare `init` on an SVM project still asks which folders
to scan.) So the AI-drivable setup is: the auditor runs `! bat-cli login` ONCE, then you run
`bat-cli init --yes` → `bat-cli sonar` → `bat-cli deploy --entry-point <X>`.

A bare `deploy` shows a fuzzy list: entry points first and marked `[entry point]`, then every
other function, with `(deployed)` on what is already on the board — pass `--entry-point` to skip
it. If that entry point is ALREADY on the board, `deploy` asks a yes/no before drawing a second
frame — pass `--yes` to answer it and redeploy non-interactively.

Safe to run unattended: `init --yes`, `sonar`, `config` (no flag), `update --check`,
`login --status`, `deploy --entry-point <X>` (add `--yes` to redeploy, or `--dry-run`).

## deploy

```bash
bat-cli deploy                                        # fuzzy-pick (interactive)
bat-cli deploy --entry-point Vault.deposit            # Contract.function, or a bare function name
bat-cli deploy --entry-point Vault.deposit --dry-run  # print the layout, contact nothing
bat-cli deploy --entry-point Vault.deposit --preview /tmp/frame.png
bat-cli deploy --entry-point Vault.deposit --refresh-links  # incremental: only swap newly-framed callees
```

| flag | |
|---|---|
| `--entry-point <name>` | `Contract.function` or bare `function`; omit to pick from a list |
| `--dry-run` | compute and print the layout, never touch Miro (no login needed) |
| `--preview <path>` | compose the frame locally as a PNG |
| `--refresh-links` | incrementally swap callees that gained their own frame for link cards, WITHOUT re-deploying (see below) |
| `--undeploy` | remove this entry point's frame from the board and registry entirely (frame, items, link cards, metadata) — to clean up a helper that should never have been its own frame |
| `--max-depth <n>` / `--max-nodes <n>` | bound the graph by hand; unset draws all of it |
| `--include-external` | include contracts coming from `lib/` |
| `--stroke-width <1-24>` | connector thickness in dp (default 8) |
| `--all` | every entry point at once — **discouraged**; it warns and asks first |
| `--yes` | skip the "already on the board — deploy again?" confirmation (redeploy non-interactively; builds a second frame) |

**Incremental relink — `--refresh-links`.** After you've hand-arranged a deployed frame, giving one
of its callees its own frame (by deploying that callee as an entry point) means the callee should
become a link card in the parent. A plain redeploy would rebuild the whole frame and DESTROY your
manual layout. `deploy --entry-point <fn> --refresh-links` instead updates ONLY what changed: for
each callee that gained a frame since the last deploy it deletes that callee's screenshot + its
connectors and drops a link card + one arrow in its place, leaving every other box, connector and
your manual positioning untouched (no re-render, no re-layout). It's idempotent (re-running reports
"nothing to refresh") and never deletes existing link cards. A frame first deployed by bat-cli <
0.22.9 has no recorded box positions, so refresh will ask you to deploy it once (full) first.

**Cross-contract resolution loop.** `deploy` follows the tree into other contracts, but a call on
an interface-typed receiver (`$.borrowerOps.adjustPosition`) has a runtime-bound target it can't
pin. So it STOPS and lists them (each with `[InterfaceType]` and in-scope candidates) rather than
silently dropping them. To include those downstream functions (and their storage markers): read
the wiring, pick the real contract, `bat-cli resolve <INTERFACE> <CONTRACT>`, and deploy again.
Each round follows what you resolved and surfaces the next layer, until the tree is complete —
resolutions live in the metadata and persist across `sonar`. `bat-cli resolve --list` shows them;
`--allow-unresolved` draws the partial graph without stopping. Standard-token interfaces
(`IERC20.balanceOf`, …) surface as name-collision candidates — those are reads, safe to skip with
`--allow-unresolved` once the real state-changing hops are resolved.

**Any function can be deployed, not just an entry point.** A shared helper needs a frame of
its own for anything else to point at, and is worth reading on its own terms.

**One frame per function, board-wide.** A function already on the board is pointed at rather
than redrawn — that is what lets several diagrams share a helper's frame. Asked for directly,
though, it is the auditor's call: a redeploy prompts yes/no, so `--entry-point` on an
already-deployed function still blocks. Add `--dry-run`, or hand the command over.

What lands on the board: one frame per entry point, one image per function already positioned,
and one connector per call site anchored to the exact line — past the end of the line when it
makes one call, on the called token itself when it makes several, since then the column is the
only thing telling them apart. The entry point sits in the top-left, layers run downward, and
no arrow points backwards.

**Storage-write markers.** Every node whose function mutates contract storage is drawn inside a
hollow red rectangle, so state-changing functions stand out at a glance. This is derived from
each function's `storage_writes` in the metadata (see `metadata.md`); nothing to configure.
Inside the rectangle, a **translucent red band** covers each exact line that writes storage
(from `storage_write_sites`), so you see WHICH state changes, statement by statement. A **modifier**
that writes storage (e.g. `initializer`) is marked the same way — it runs as part of the function
it guards, so its writes count.

**External-boundary markers.** A **dashed amber band** covers a line that calls an external
contract with no in-scope source — an interface-typed receiver nothing in the repo implements
(e.g. an ERC-20 by address), via a non-view method (from `unknown_external_calls`). It means the
flow leaves the audited code and the callee MIGHT mutate its own state — unverified, so it is
deliberately distinct from the solid-red proven write. `view`/`pure` calls are never flagged.
A function that makes such a call but writes no storage of its own also gets a **solid amber
rectangle** around its whole node — the amber counterpart of the red storage border, so
"probably a state change here" reads at a glance alongside the proven-write nodes.

**Frame recycling.** Redeploying a function reuses its existing frame (same id and position),
wiping and redrawing the contents — so a diagram that links to the frame by URL keeps working,
and no duplicate frames pile up. Interface/abstract stubs (bodyless declarations, empty
`virtual {}`) are never drawn on their own: a stub is redirected to its concrete override.
A callee that ALREADY has its own (still-on-the-board) frame is referenced with a link card pointing
at that frame instead of being redrawn with its subtree — so the more of a big tree's functions you
deploy as their own frames, the thinner the parent frame becomes on its next redeploy.

**When a branch is linked out to its own frame.** Framing is a size-balanced partition, not a hard
cap. A call tree under ~20 screenshots is drawn whole. A bigger one is split so each piece lands near
a readable **target of ~15 screenshots** — never below 6 (so no husks) — at most 6 pieces per frame;
a piece that is itself still over ~20 becomes its own frame and is split again the same way, giving a
shallow hierarchy of readable frames instead of one giant canvas. The branch chosen to link out is
the one whose size is nearest the target AND severs the fewest cross-frame arrows (not simply the
biggest), and a densely-shared function can be lifted out whole — each of its callers keeps a link
card to it — which is the only way to partition a graph where a few helpers are reused everywhere.
Depth counts against a frame (a deep, narrow frame runs off-screen). Cutting to a frame that already
exists is always allowed (it reuses, doesn't create). So a redeploy prefers several readable frames,
each linked from the ones above it, over one wall of screenshots or a scatter of tiny fragments.

## Failure modes

| symptom | cause / fix |
|---|---|
| `this is already a bat project` | `Bat.toml` exists — the auditor wanted `sonar`, not `init` |
| `no entry point matched; run bat-cli sonar first` | metadata missing or stale, or the name is wrong — check the names with `jq '.entry_points[].name' BatMetadata.json` |
| deploy fails on an Anchor/Pinocchio project | expected: no SVM deploy path, only `init`/`sonar` |
| `not logged in to Miro` | the auditor runs `! bat-cli login` once per machine; `--dry-run` and `--preview` work without it |
| board creation failed during `init` | Miro's free plan caps team boards at three; `init` keeps going, set the board later |
| `No .sol files found in src/` | `init` ran outside the Foundry repository root |
| the command hangs with no output | it hit an interactive prompt — kill it and hand it to the auditor |
"##;

const METADATA: &str = r##"<!-- Generated by bat-cli {BAT_CLI_VERSION} — do not edit; regenerated on every bat-cli command. -->
# bat-cli — querying `BatMetadata.json`

The scan is the fastest way to answer a structural question about the codebase. Reach for it
before grepping `.sol` files: one `jq` read replaces a search that has to guess at naming.

**It is only as fresh as the last `bat-cli sonar`.** Every record carries line numbers; if the
source moved since the scan they point at the wrong lines. When a `line` does not match what
is in the file, rescan rather than working around it.

## Shape (Foundry / EVM)

Top level: `contracts`, `entry_points`, `function_dependencies`, `interfaces`, `file_items`,
`miro`.

- **`contracts[]`** — `metadata_id`, `name`, `file_path`, `contract_type`
  (`Contract` | `Interface` | `Abstract` | `Library`), `base_contracts`, `line`, `external`,
  and nested `functions`, `state_variables`, `events`, `modifiers`.
  - `functions[]` — `metadata_id`, `name`, `contract_name`, `visibility`, `mutability`,
    `modifiers`, `params`, `returns`, `line`, `end_line`, `is_constructor`, `is_stub` (bodyless
    declaration or empty `virtual {}` — nothing to draw; redirected to its override on deploy),
    `storage_writes`, `storage_write_sites`, `unresolved_calls`, `unknown_external_calls`.
  - `state_variables[]` — `name`, `type_name`, `visibility`, `is_constant`, `is_immutable`, `line`.
- **`entry_points[]`** — the public/external surface: `name` (stored as `Contract.function`),
  `contract_name`, `function_metadata_id`, `access_control`, `storage_reads`, `storage_writes`,
  `external_calls`, `events_emitted`, `modifiers`, `dependencies`.
- **`function_dependencies[]`** — the call graph, as `function_metadata_id` → `callees[]`.
- **`interfaces[]`** — `name`, `implemented_by`, `functions`.

**`storage_writes`** is populated (empty = writes no storage) on BOTH `entry_points[]` and every
`contracts[].functions[]`. It lists the written storage locations as readable paths — a state
var (`totalSupply`), an index/mapping (`balances[]`), a storage-pointer field (`$.reserveStable`),
or an accessor path (`_s().paused`). The Miro deploy rings storage-writing nodes in red from this.
Match the exact string (the recipe below finds writers of one var). **`storage_write_sites`** is
the same writes with the FILE `line` each sits on (`{name, line}`), driving the per-line red band.

**`unknown_external_calls`** (on every `contracts[].functions[]`) lists calls on an interface-typed
receiver with NO in-scope implementer — the callee's source is not in the repo (e.g. an ERC-20 by
address), so its storage effect is unknowable. Each is `{receiver, method, inferred_type}`. The
deploy flags non-view ones with a dashed amber band; unlike `unresolved_calls`, these have no
in-scope target to `resolve`.

**`unresolved_calls`** (on every `contracts[].functions[]`) is the AI-resolution work-list for
FULL cross-contract storage coverage. Static analysis marks a function's own `storage_writes`
exactly, and follows calls to concrete contracts — but a call on an **interface-typed** receiver
(`$.borrowerOps.adjustPosition(…)`) has a target that is only bound at runtime, so it cannot be
pinned statically. Each entry is `{receiver, method, inferred_type, candidates}` — `candidates`
are the in-scope concrete contracts that plausibly implement it. **To answer "what does entry
point X change?" completely:** (1) collect `storage_writes` across X's statically-resolved call
tree, then (2) for each `unresolved_calls` entry on the way, read the WIRING (where `receiver` is
assigned — the constructor, config setter, factory, deploy script) to pick the real contract from
`candidates`, look up that method there, and recurse into ITS `storage_writes` / `unresolved_calls`.
That's the only irreducibly-dynamic step, and it's yours: static analysis narrows it to a short
candidate list; you decide the actual target from the evidence. To surface it visually, deploy the
resolved writer (`bat-cli deploy --entry-point <Contract.method>`) — its node is ringed red.
- **`miro`** — what is already on the board; `miro.auto.frames[]` holds `entry_point` and
  `frame_url` per deployed frame.

Two rules that decide most queries:

- **Cross-references are by `metadata_id`** — a random 30-character string — **never by name.**
  Join on the id; two contracts can define the same function name.
- **`external: true` marks anything from `lib/`.** Exclude it for scope questions: a finding in
  a vendored dependency is usually out of scope.

`access_control` values: `OnlyOwner`, `{"RoleBased": {"role": …}}`,
`{"RequireMsgSender": {"compared_to": …}}`, `{"CustomModifier": {"name": …}}`, `None`.

## Recipes

```bash
# every entry point with its access control
jq -r '.entry_points[] | "\(.name)  \(.access_control | tostring)"' BatMetadata.json

# entry points with NO access control — the first thing to look at
jq -r '.entry_points[] | select((.access_control | length) == 0 or .access_control == ["None"]) | .name' BatMetadata.json

# in-scope contracts only (drop lib/)
jq -r '.contracts[] | select(.external | not) | "\(.name)\t\(.file_path)"' BatMetadata.json

# one contract's functions, with visibility, mutability and line span
jq -r '.contracts[] | select(.name == "Vault") | .functions[]
       | "\(.name) \(.visibility) \(.mutability) L\(.line)-\(.end_line)"' BatMetadata.json

# one contract's storage layout
jq -r '.contracts[] | select(.name == "Vault") | .state_variables[]
       | "\(.type_name) \(.name) \(.visibility)"' BatMetadata.json

# which entry points write a given storage variable
jq -r --arg v "totalSupply" '.entry_points[] | select(.storage_writes | index($v)) | .name' BatMetadata.json

# entry points that make an external call (reentrancy surface)
jq -r '.entry_points[] | select((.external_calls | length) > 0)
       | "\(.name): \(.external_calls | join(", "))"' BatMetadata.json

# resolve a function name to its metadata_id
jq -r '.contracts[] | .functions[] | select(.name == "deposit")
       | "\(.contract_name).\(.name) \(.metadata_id)"' BatMetadata.json

# who a function calls, by name (join callees back through the contracts)
jq -r --arg id "<metadata_id>" '
  . as $m
  | ($m.contracts[] | .functions[] | {(.metadata_id): "\(.contract_name).\(.name)"}) as $names
  | $m.function_dependencies[] | select(.function_metadata_id == $id) | .callees[]' BatMetadata.json

# what is already deployed to the board
jq -r '.miro.auto.frames[] | "\(.entry_point)\t\(.frame_url)"' BatMetadata.json
```

## SVM projects

An Anchor/Pinocchio/vanilla-Rust scan writes a different shape — `source_code`, `entry_points`,
`function_dependencies`, `traits`, `context_accounts`, `miro` — with the same `metadata_id`
discipline. There is no deploy path for it, but the same querying approach applies; inspect the
top-level keys with `jq 'keys' BatMetadata.json` before writing a recipe.
"##;

const CHANGELOG: &str = r##"<!-- Generated by bat-cli {BAT_CLI_VERSION} — do not edit; regenerated on every bat-cli command. -->
# bat-cli changelog — what's new (read this to spot new capabilities)

New bat-cli capabilities **by version, newest first**. You are running bat-cli
**{BAT_CLI_VERSION}** — everything listed at `{BAT_CLI_VERSION}` and below is available to you.

When `Bat.toml`'s `bat_cli_version` rises above the value you last saw, **read THIS file
first**: each entry lists exactly what changed AND which guide docs to re-read (`Re-read:`),
so you re-open only the docs that actually changed — not everything.

## 0.22.10
- **Readable frames for tangled graphs: frame the big shared subtrees, localize the small crossers.**
  A function reused all over a diagram used to draw arrows that cross every screenshot, turning a big
  frame into an unreadable mesh (Miro auto-routes connectors, so the layout can't bend around them).
  Now: (1) big shared subtrees (≥ 6 screenshots) are cut to their own sub-frames; (2) THEN the small
  helpers still crossing (a caller ≥ 2 columns back) get a local copy per far caller — cheap, and no
  longer whack-a-mole because the deep floor is already framed out. increaseLeverage's main frame
  went from a 178-screenshot mesh to a readable ~37.
- **`deploy --redeploy` — fresh cluster in a clean zone, old URLs handed back.** Redeploys the whole
  cluster (entry point + every dependency frame) FRESH into a clean region, reusing nothing already
  on the board (only frames created within the same run). At the end it prints the PREVIOUS cluster's
  frame URLs so you delete them with one click in Miro's web UI (which deletes a frame with its
  contents; the API can't, and one-by-one deletion is slow). No hunting the board by hand.
- **`deploy --inline-all`** draws the whole graph in one frame (no cuts, no links) to see how big a
  function is with screenshots only. **`deploy --preview <path>`** now composes the frame as a LOCAL
  png and never touches the board — the fast way to iterate on a diagram (was a footgun that also
  deployed). **Render dedup:** each distinct function is rendered once per run (shared across every
  frame and every duplicate copy), not once per appearance.
- **Overloaded functions are no longer collapsed.** When a contract defines the same function name
  several times (Solidity overloads — e.g. a public `deleverageQuote(...)` that forwards to an
  internal `deleverageQuote(curve, ...)`), the deploy used to map every call to the FIRST definition,
  so a wrapper calling its sibling overload looked like a self-call and was dropped — the whole
  implementation subtree behind it silently vanished. Calls now carry their argument count and
  resolve to the overload whose parameter count matches, and each overload is its own node, so the
  full graph is drawn. Non-overloaded code is unchanged. Regenerate with `deploy`.
- **`deploy --undeploy <entry-point>` removes a frame outright.** Cleans a frame that should never
  have been its own — its shell, all its screenshots/markers/borders, its link cards + arrows, and
  its registry entry — so the next deploy of a caller inlines that helper instead of linking to it.
  (A frame you delete BY HAND in Miro is also detected as gone on the next deploy and forgotten, so
  it is never silently re-created either.)
- **Duplicated callers no longer lose their call-out.** When a helper is duplicated so each caller
  has a nearby copy, a copy whose real work is a call to a SHARED function (e.g. a copy of
  `_positionCollAndDebt` that just calls the shared `getPositionCollAndDebt`) used to be drawn as a
  dead-end — its call line pointing at nothing. Each copy now keeps its calls to shared functions
  (the shared node duplicates in turn, or the copies converge on it), so the graph below a
  duplicated node is always complete. Regenerate with `deploy`.
- **Balanced framing: a big graph becomes several readable frames, not one wall or a scatter of
  husks.** Deploying a large entry point used to either ship one enormous unreadable frame or, when
  it did cut, fragment into tiny pass-through husks. Framing is now a partition aimed at a size:
  a graph under ~20 screenshots ships whole; a bigger one is split so each piece lands near a
  readable **target of ~15 screenshots** (never below 6, so no husks), at most 6 pieces per frame,
  and a piece that is itself still too big becomes its own frame and is split again the same way —
  so the result is a shallow hierarchy of frames you can actually read, not a 30-layer canvas.
  Which branch is cut is chosen for BALANCE (a piece whose size is near the target, that severs the
  fewest cross-frame arrows) rather than just "the biggest", and a densely-shared node can now be
  lifted out as a whole (each caller keeps a link card to it) — the only way to partition a graph
  where every helper is reused. Depth counts against a frame (a deep-narrow frame runs off-screen),
  and recycling reuses only frames STILL on the board, so a frame you delete by hand is never
  silently re-created. Regenerate with `deploy`. _Re-read: workflow.md (the framing/link-card
  policy under `deploy`)._

## 0.22.9
- **Incremental `deploy --refresh-links`: swap newly-framed callees for link cards without
  re-deploying.** After you've hand-arranged a frame, deploying one of its callees as its own
  entry point means that callee should become a link card in the parent — but a full redeploy
  would blow away your manual layout. `deploy --entry-point <fn> --refresh-links` now does this
  SURGICALLY: it touches ONLY the callees that gained a frame since the last deploy — deletes each
  one's screenshot + its connectors and drops a link card + one arrow in its place — and leaves
  every other box, connector and your manual positioning exactly as-is (no re-render, no re-layout).
  It's idempotent (a second run reports "nothing to refresh") and never deletes existing link
  cards. A frame first deployed by an older bat-cli has no recorded positions, so refresh asks you
  to deploy it once (full) first. _Re-read: workflow.md (the `--refresh-links` incremental flow)._

## 0.22.8
- **Readable fan-out: per-target arrow colours, shared-node duplication, and marker fixes.** The
  diagram is much easier to follow on a busy frame: (1) each callee's arrows get a colour ranked
  within its depth, so different functions at one layer are distinct hues while the same function
  keeps one colour; (2) a shared NON-leaf reused across callers (e.g. `getNominalICR` ×8) is now
  DUPLICATED near each caller (its private subtree, capped so the frame grows ≤ ~1.4×) instead of
  drawn once with long edges crossing over other screenshots — generalises the shared-leaf copy
  rule; (3) boxes are nudged slightly in x so connectors leave from distinct points; (4) a
  substring bug that put a call/marker on the wrong line (`mint` matching `mintedAlmShares`,
  `stake` matching `_stake`) is fixed with whole-identifier matching. Regenerate with `sonar`,
  then `deploy`. _Re-read: nothing (diagram only; metadata shape unchanged)._

## 0.22.7
- **Calls on an interface CAST receiver are no longer dropped.** A call written as
  `IFace(addr).method()` — e.g. `IClammReferenceFeed($.referenceFeed).latestReference()` — was
  silently lost by the deploy's call-site extractor (it only understood a plain variable receiver),
  so the callee was neither drawn nor linked to its own frame. The receiver now renders as
  `IFace().method`, matching the metadata analysis, so it resolves (to a unique in-scope
  implementation) and, if that function already has a frame, links to it. _Re-read: nothing
  (behavioral; metadata shape unchanged)._

## 0.22.6
- **Modifiers that write storage are now detected and marked.** A modifier runs as part of every
  function it guards, so its state changes are real — e.g. OpenZeppelin's `initializer` sets
  `$._initialized` / `$._initializing`. Its body is now analyzed like a function's, each
  `contracts[].modifiers[]` carries `storage_writes` + `storage_write_sites`, and the modifier node
  on the diagram gets the red border and per-line bands. Regenerate with `sonar`, then `deploy`.
  (External `__…_init` FUNCTIONS such as `__ERC20_init` / `__UUPSUpgradeable_init` live in `lib/`;
  their writes are always analyzed but only DRAWN with `--include-external`.) _Re-read: metadata.md,
  workflow.md._

## 0.22.5
- **Deploy recycles already-deployed frames instead of redrawing them.** When a function's tree
  reaches a callee that ALREADY has its own frame on the board, that callee is now referenced with
  a link card pointing at its frame — its whole subtree is no longer redrawn inside this one, so a
  redeploy reuses what is there rather than cluttering the frame with duplicates. It is recomputed
  every deploy from the current frames, so deploying more of the tree's functions as their own
  frames progressively thins the parent (e.g. `execute` links `swapExactInX96`, 44 → 33
  screenshots). A card whose frame was deleted is re-deployed on demand. _Re-read: workflow.md._

## 0.22.4
- **Interface calls with a single in-scope implementation now deploy — deterministically, no
  `resolve` needed.** A call on an interface-typed receiver that nothing declares `is …` (so
  inheritance can't pin it) but whose method is defined by EXACTLY ONE in-scope contract is now
  drawn straight to that contract — e.g. `alm.getReservesAtSqrtPrice(...)` → `CvammALM`. Before,
  such a call was pushed to the AI work-list and then dropped by the storage-write prune when the
  method was a `view` read, so it silently never appeared. This is pure static analysis (a
  uniqueness gate: a common name like `transfer`, defined by many, still never auto-binds and stays
  an external boundary / `resolve` target). Deploy-time only; `sonar` is unchanged.
  _Re-read: workflow.md._

## 0.22.3
- **Connector fix: arrows into a full-width line no longer pile up at the image edge.** When
  several dependencies reach one line whose text runs to the screenshot's right edge (a signature
  line carrying modifiers, say), their arrows used to converge flush on the boundary and became
  impossible to tell apart. Their shared convergence point is now pushed into the gutter for that
  case only; shorter lines and the left side are unchanged. _Re-read: nothing (visual only)._

## 0.22.2
- **Node-level marker for probable external state changes.** A function that makes a non-view
  call to a sourceless external contract but writes NO storage of its own now gets a hollow SOLID
  amber rectangle around its whole screenshot — the amber counterpart of the solid-red storage
  border. So at a glance: red border = proven storage write, amber border = a state change probably
  happens here (unverified). _Re-read: workflow.md._

## 0.22.1
- **External-boundary detection now catches interface CASTS.** A call like
  `IERC20Minimal(addr).transferFrom(...)` — an interface cast of a runtime address with no
  in-scope implementer — used to be misfiled as a resolvable call (some unrelated in-scope contract
  happens to define `transferFrom`) and then pruned, so the line was flagged as nothing. A bare
  interface cast with no type-proven implementer is now correctly a dashed-amber external boundary.
  A wired receiver (`$.borrowerOps`, `_s().CORE`) is unaffected. _Re-read: metadata.md, workflow.md._

## 0.22.0
- **Exact storage-write lines on the board.** A red frame border said a function mutates state but
  not WHICH. Each `contracts[].functions[]` now carries **`storage_write_sites`** — every write
  with the `name` (lvalue path) and the FILE `line` it happens on. On the board, `deploy` draws a
  translucent red band over each of those exact lines, so you read off precisely which state a
  function changes, statement by statement. Regenerate with `sonar`, then `deploy`.
  _Re-read: metadata.md, workflow.md._
- **External-boundary markers — calls to contracts whose source you don't have.** A call on an
  interface-typed receiver that NOTHING in the repo implements (an ERC-20 passed by address, say)
  reaches a contract with no in-scope source, so its storage effect is unknowable. Each function
  now carries **`unknown_external_calls`** (interface-typed receivers only), and `deploy` marks
  each such line with a DASHED AMBER band — visually distinct from the solid-red proven write: it
  means "unverified external state-change boundary", not a fact. A `view`/`pure` method is never
  flagged (the compiler guarantees no mutation). _Re-read: metadata.md, workflow.md._
- **No more duplicate interface/abstract screenshots.** A call resolving to a bodyless interface
  declaration or an empty `virtual {}` stub was drawn as its own node next to the real one. Now a
  stub is redirected to its single concrete override (`is_stub` on each function); a pure
  declaration with no in-scope override is drawn as nothing. _Re-read: workflow.md._
- **Redeploy recycles the frame in place.** Deploying a function again wipes its contents and
  reflows them into the SAME frame (same id/position), instead of leaving a duplicate — so links
  that point at a frame by URL keep working. _Re-read: workflow.md._

## 0.21.0
- **Cross-contract storage coverage — deploy an entry point, see EVERY storage change it causes,
  across contracts.** A call on an interface-typed receiver (`$.borrowerOps.adjustPosition`) has a
  concrete target bound at runtime that static analysis can't pin, so those hops used to be
  dropped and their downstream writes invisible. Now:
  - Each `contracts[].functions[]` carries **`unresolved_calls`** — the interface hops that need
    resolving, each with `inferred_type` (the receiver's interface, resolved through struct-field,
    local, parameter and accessor-return types, following field chains of any depth like
    `_s().CORE.owner`), in-scope `candidates`, and `assigned_in` (the functions that WRITE the
    receiver — where its address is wired, so you know which candidate is real). The list is
    pruned to only the hops that can actually reach a storage write.
  - **`deploy` walks the whole tree and STOPS** listing the unresolved hops (transitively — the
    entire tree at once) instead of drawing a partial graph. Record each with
    `bat-cli resolve <INTERFACE> <CONTRACT>` (stored in the metadata's `resolutions`, preserved
    across `sonar` like `miro`); deploy follows them, drawing the concrete downstream functions
    with their red storage markers. `--allow-unresolved` draws the partial graph as-is.
- **Storage-write recall fix.** Functions with a MULTI-LINE signature were parsed from the wrong
  line and silently lost their writes/calls; the whole function is parsed now, so their storage
  writes (and everything above) are detected. Also a large internal speedup — one parse per
  function instead of several. _Re-read: metadata.md, workflow.md._

## 0.19.1
- **Storage-write detection now covers writes through `storage` PARAMETERS** — a library that
  takes the storage struct as a reference (`execute(CvammStorage storage $, …)` then
  `$.reserveStable += …`) is the common EVM pattern and was previously missed. The red
  storage-write marker on the board is also thicker/more prominent. _Re-read: nothing (metadata
  shape unchanged; just more complete)._

## 0.19.0
- **Storage-write detection + diagram markers.** Each function now records the contract storage
  it writes in `storage_writes` (state vars, `mapping[k]=`, storage-pointer `$.x`, accessor
  `_s().x`, `++`/`--`, `delete`, `.push`/`.pop`; inherited state vars resolved). On the Miro
  board, every node whose function mutates storage is drawn inside a hollow red rectangle, so
  state changes stand out. Regenerate with `sonar`, then `deploy`. _Re-read: metadata.md, workflow.md._
- **`init --yes` is fully non-interactive** (for scripts / AI): project name = folder name, and
  the Miro board is resolved with no prompt — it reuses an existing board named after the folder,
  else creates one, else `--board-url <URL>` attaches a specific board. The AI-drivable setup is
  now `! bat-cli login` (once, human) → `bat-cli init --yes` → `sonar` → `deploy --entry-point`.
  _Re-read: workflow.md._

## 0.18.1
- The generated docs' header said they are regenerated on `init`/`sonar`/`deploy`; every
  bat-cli command regenerates them. Wording only. _Re-read: nothing._

## 0.18.0
- **bat-cli now generates its own AI guide, once per machine.** Every command regenerates
  `~/.config/bat-cli/ai_context/` (this file, `README.md`, `workflow.md`, `metadata.md`) from
  the running binary, so the guide always documents the version installed right now and never
  lands inside the repository being audited. It also installs a version-agnostic router —
  `~/.claude/skills/bat-cli/SKILL.md`, `~/.agents/skills/bat-cli/SKILL.md`, and a managed block
  in `~/.gemini/GEMINI.md` — that only says where the guide lives, so upgrading never rewrites
  it and no assistant session needs a second restart.
- **`Bat.toml` carries `bat_cli_version`**, stamped by `init`/`sonar`/`deploy`: which binary
  last scanned this project, and therefore whether `BatMetadata.json` came from the parser you
  are running today.
- **`bat-cli update` now publishes the new version's guide immediately**, by asking the binary
  it just installed to regenerate it — the updating process is the outgoing version, so the
  guide used to describe the replaced version until you happened to run something else.
  `bat-cli refresh-ai-guide` does the same on demand and needs no project.
  _Re-read: README.md, workflow.md, metadata.md._
"##;

/// The global Claude Code / Codex skill. Byte-stable and version-agnostic on purpose: it is a
/// router, not the instructions, so upgrading bat-cli never rewrites it and a running session
/// never needs another restart.
const GLOBAL_SKILL_MD: &str = r##"---
name: bat-cli
description: Drive bat-cli, the Blockchain Auditor Toolkit that parses a smart-contract codebase into BatMetadata.json and draws its call graph onto a Miro board. Use whenever the user mentions bat-cli, or wants to "init the bat project", "run sonar", "rescan the metadata", "deploy this function/entry point to Miro", "draw the call graph", "preview the diagram", or wants to query the parsed codebase (entry points, call graph, storage, access control) in a folder that contains a Bat.toml.
---

bat-cli parses the smart-contract codebase of an audited repository into `BatMetadata.json`,
and draws a function's call graph onto a Miro board — every function a syntax-highlighted
screenshot, laid out and uploaded already positioned, with every arrow landing on the exact
line that makes the call.

**You are in a bat-cli project when the current folder — or any parent — contains a
`Bat.toml`.** If there is none, this is not a bat-cli project; do nothing bat-cli-specific
(beyond telling the user `bat-cli init` is what creates one). Run every bat-cli command from
the directory holding `Bat.toml`.

**The authoritative instructions live in one machine-global guide**, written by the installed
binary — not in the audited repository. Read it once, then stay current cheaply; do NOT
re-read everything before every action:

1. The guide is `~/.config/bat-cli/ai_context/` (or `$XDG_CONFIG_HOME/bat-cli/ai_context/`, or
   `$BAT_CLI_CONFIG_DIR/ai_context/` when either is set). Read it and follow `README.md`'s
   "read once" rules from then on:
   - `README.md` — what bat-cli is, and the golden rules
   - `workflow.md` — the commands, their flags, what is interactive, failure modes
   - `metadata.md` — the `BatMetadata.json` schema and `jq` recipes
   - `changelog.md` — what is NEW per version; your source of truth for new capabilities
2. **`changelog.md` is how you learn what changed.** The docs carry the version that generated
   them in their header. If `bat-cli --version` is HIGHER, bat-cli was updated: read
   `changelog.md` FIRST — each entry lists the new capabilities AND a `Re-read:` line naming
   exactly which docs changed, so you re-open only those. If it matches, the guide is current.
3. `Bat.toml`'s `bat_cli_version` is a different signal: it says which binary last scanned
   THIS project. Behind `bat-cli --version` means `BatMetadata.json` predates your parser —
   run `bat-cli sonar` before trusting it.

If the guide directory does not exist, no bat-cli command has run since it was installed:
run `bat-cli refresh-ai-guide` (harmless, needs no project) to publish it, then read it.

Non-negotiables: run from the project root; never answer an interactive `dialoguer` prompt by
guessing — hand those commands to the user; prefer `--dry-run` while checking a graph, since a
real deploy puts dozens of objects on a shared board.
"##;

/// Body of the managed block in `~/.gemini/GEMINI.md`. Same router, same byte-stability.
const GLOBAL_AGENTS_BODY: &str = r##"## bat-cli projects — instructions for AI coding agents

bat-cli parses the smart-contract codebase of an audited repository into `BatMetadata.json`,
and draws a function's call graph onto a Miro board.

**You are in a bat-cli project when the current folder — or any parent — contains a
`Bat.toml`.** If there is none, ignore this section. Run every bat-cli command from the
directory holding `Bat.toml`.

**The authoritative instructions live in one machine-global guide**, written by the installed
binary — not in the audited repository. Read it once, then stay current cheaply:

1. The guide is `~/.config/bat-cli/ai_context/` (or under `$XDG_CONFIG_HOME/bat-cli` /
   `$BAT_CLI_CONFIG_DIR` when either is set): `README.md`, `workflow.md`, `metadata.md`,
   `changelog.md`. Follow README's "read once" rules thereafter. If the directory does not
   exist, run `bat-cli refresh-ai-guide` once to publish it.
2. **`changelog.md` is how you learn what changed.** The docs carry the version that generated
   them. If `bat-cli --version` is HIGHER, read `changelog.md` FIRST — each entry lists the new
   capabilities AND a `Re-read:` line naming exactly which docs changed, so you re-open only
   those.
3. `Bat.toml`'s `bat_cli_version` says which binary last scanned THIS project; behind
   `bat-cli --version` means the metadata predates your parser — run `bat-cli sonar`.

Non-negotiables: run from the project root; never guess at an interactive prompt — hand those
commands to the user; prefer `--dry-run` while checking a graph.
"##;

#[cfg(test)]
mod tests {
    use super::{
        ensure_ai_guide_at, ensure_global_ai_skills_at, write_managed_block, AGENTS_BEGIN,
        AGENTS_END, GLOBAL_SKILL_MD,
    };

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bat-cli-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn writes_guide_and_is_idempotent() {
        let dir = temp_dir("guide");

        assert!(ensure_ai_guide_at(&dir).unwrap()); // first run creates the files
        for name in ["README.md", "workflow.md", "metadata.md", "changelog.md"] {
            assert!(dir.join(name).exists(), "{name} missing");
        }

        // The placeholder is replaced with the running version, in every file.
        let readme = std::fs::read_to_string(dir.join("README.md")).unwrap();
        assert!(readme.contains(&format!("Generated by bat-cli {}", env!("CARGO_PKG_VERSION"))));
        assert!(!readme.contains("{BAT_CLI_VERSION}"));
        let changelog = std::fs::read_to_string(dir.join("changelog.md")).unwrap();
        assert!(changelog.contains(&format!("**{}**", env!("CARGO_PKG_VERSION"))));
        assert!(!changelog.contains("{BAT_CLI_VERSION}"));

        // The metadata guide teaches the join rule the recipes depend on.
        let metadata = std::fs::read_to_string(dir.join("metadata.md")).unwrap();
        assert!(metadata.contains("metadata_id"));

        assert!(!ensure_ai_guide_at(&dir).unwrap()); // second run is a no-op, no churn

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn global_skill_is_a_stable_router() {
        // It carries the description Claude matches on, routes to the project's
        // Bat.toml/ai_context, and is byte-stable — no version stamp, so upgrading bat-cli
        // never rewrites it and no session needs a second restart.
        assert!(GLOBAL_SKILL_MD.starts_with("---\nname: bat-cli"));
        assert!(GLOBAL_SKILL_MD.contains("description:"));
        assert!(GLOBAL_SKILL_MD.contains("Bat.toml"));
        // It names the ONE global location; a per-project path would be a regression.
        assert!(GLOBAL_SKILL_MD.contains("~/.config/bat-cli/ai_context/"));
        assert!(GLOBAL_SKILL_MD.contains("BAT_CLI_CONFIG_DIR"));
        assert!(!GLOBAL_SKILL_MD.contains("{BAT_CLI_VERSION}"));
    }

    #[test]
    fn installs_routers_without_touching_foreign_content() {
        let home = temp_dir("global-skills");

        // A GEMINI.md the user already wrote: their content must survive.
        let gemini = home.join(".gemini/GEMINI.md");
        std::fs::create_dir_all(gemini.parent().unwrap()).unwrap();
        std::fs::write(&gemini, "# My own rules\n").unwrap();

        // ~/.claude/skills did not exist → the caller is told a restart is needed, once.
        assert!(ensure_global_ai_skills_at(&home).unwrap());
        let claude_skill = home.join(".claude/skills/bat-cli/SKILL.md");
        assert_eq!(
            std::fs::read_to_string(claude_skill).unwrap(),
            GLOBAL_SKILL_MD
        );
        let codex_skill = home.join(".agents/skills/bat-cli/SKILL.md");
        assert_eq!(
            std::fs::read_to_string(codex_skill).unwrap(),
            GLOBAL_SKILL_MD
        );
        let after = std::fs::read_to_string(&gemini).unwrap();
        assert!(after.starts_with("# My own rules\n"));
        assert!(after.contains(AGENTS_BEGIN) && after.contains(AGENTS_END));

        // Second run: the directory exists now, so no restart hint.
        assert!(!ensure_global_ai_skills_at(&home).unwrap());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn managed_block_appends_then_refreshes_in_place() {
        let dir = temp_dir("agents");
        let path = dir.join("GEMINI.md");
        let mine = "# My own agent rules\nDo not touch this.\n";
        std::fs::write(&path, mine).unwrap();

        assert!(write_managed_block(&path, "bat-cli body\n").unwrap());
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.starts_with(mine)); // their content kept, at the top

        // Refreshing rewrites only the block: their content stays, no duplicate markers.
        assert!(write_managed_block(&path, "bat-cli body v2\n").unwrap());
        let again = std::fs::read_to_string(&path).unwrap();
        assert!(again.starts_with(mine));
        assert!(again.contains("bat-cli body v2"));
        assert_eq!(again.matches(AGENTS_BEGIN).count(), 1);

        assert!(!write_managed_block(&path, "bat-cli body v2\n").unwrap()); // no churn

        let _ = std::fs::remove_dir_all(&dir);
    }
}
