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
/// Was this project scanned by the bat-cli that is running now?
///
/// Tells "the index does not model this" apart from "the index is stale", so a message can
/// stop suggesting a rescan that would change nothing.
pub fn scanned_by_this_binary() -> bool {
    crate::config::BatConfig::get_config()
        .map(|config| config.bat_cli_version == env!("CARGO_PKG_VERSION"))
        .unwrap_or(false)
}

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
| `bat-cli screenshot` | draw one declaration's source onto a frame already on the board | no |
| `bat-cli relink` | re-point a frame's record at the frame on the board, after a cut and paste | no |

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

## Reading state changes on the diagram

**One red mark is one state change.** A red band marks either the assignment itself, or — when
the write happens past the edge of this frame — the call that reaches it. A red border means the
function contains at least one such mark. Counting the red marks on a frame counts the distinct
state changes it causes.

The mark sits at the DEEPEST point the frame reaches. In a chain like
`_increaseDebt → DebtToken.mint → ERC20._mint → ERC20._update`, only `_update` assigns anything;
if it is drawn, it carries the mark on its own assignments and nothing above it repeats them. If
it is not drawn, the mark falls on `ERC20._mint`'s call to it — the last place the frame can show
it. Either way the change appears once, never five times along the chain.

A callee cut out to its own frame (a link card) counts as absent, so the caller marks the
boundary and the other frame marks the change itself.

Reachability is computed from the metadata call graph and always crosses into `lib/`. It does
depend on interface resolutions: a call through an
interface you have not resolved (`bat-cli resolve`) stops the walk, so its writes stay invisible
— which is one more reason to finish the resolution loop rather than deploy with
`--allow-unresolved`.

`deploy --dry-run` prints a `state` column (`write` for a direct assignment, `→write` for one
reached through a call) and lists every call line that reaches a state change.

## When a frame's record no longer matches the board

Dragging a frame in Miro keeps its id; **cutting and pasting it (or duplicating it) gives the
frame and every child a new one**, which orphans the registry — and nothing about the two
gestures looks different to whoever is arranging the board.

This heals itself: when the recorded id is gone, `deploy` and `screenshot` look for a frame
titled `auto: <entry point>` and rebuild the record from it, matching each screenshot by the
title it carries. You only step in when the answer is not obvious:

```bash
bat-cli relink --check                       # what the registry still matches, and what moved
bat-cli relink <entry point>                 # re-anchor by title
bat-cli relink <entry point> --frame-url <url>   # when several frames share the title
```

Never deploy again to fix this. A deploy does write a correct record, but it draws a NEW cluster
somewhere else — the arrangement the auditor built is the thing being protected.

Link cards, connector markers and borders carry no title, so they cannot be recognised on a
pasted copy; their ids are dropped from the record. Nothing on the board is stranded by that: the
frame and its items are still there, and deleting the frame in Miro takes its contents with it.

## When the auditor doesn't know what something is

A function's screenshot names things it does not explain: a state variable it compares against,
a struct it takes as a parameter. The declaration is elsewhere in the source, so reading the
diagram means leaving it. `bat-cli screenshot` puts that declaration on the frame.

There is deliberately **no rule** deciding what deserves to be drawn — that judgement is the
auditor's. When they say "I don't know what `X` is", draw `X`:

```bash
bat-cli screenshot                                            # lists the deployed frames
bat-cli screenshot deviationThresholdWad --frame CvammALM.poke
bat-cli screenshot PriceFeed.FeedType --frame CvammALM.poke --with-documentation
bat-cli screenshot --file src/core/PriceFeed.sol --lines 36-44 --frame CvammALM.poke
```

- The symbol is `Name` or `Contract.Name`. Structs, enums, state variables (`constant` and
  `immutable` included), functions, modifiers and events all resolve from the scan — including a
  bodiless interface declaration like `IMorphoBlue.market`, which is what you want beside a call
  that crosses that boundary. You never need to find the line numbers yourself.
- **A name declared in several contracts stops with the candidates listed.** Re-run with the
  qualified form it prints; do not guess.
- **`--file` + `--lines` is the escape hatch.** If a name is not in the index, read the source,
  find the range, and pass it — the command never blocks on a gap in the scan.
- Omitting `--frame` lists the deployed frames, which is how you learn their names.
- The image lands in free space below the frame's content. **The frame itself is never moved or
  resized** — where a frame sits is the auditor's arrangement. If nothing free is left it goes to
  the bottom-left corner, overlapping, which is visible and one drag away; `--grow` extends the
  frame downwards instead. It is drawn the same way `deploy` draws a function, so
  it matches the rest of the diagram.
- It is a manual enrichment of one diagram, not part of the call graph: a later deploy draws a
  new cluster and does not bring it back. Deleting the frame in Miro cleans it up with the frame.

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
it. Deploying an entry point that is already on the board asks nothing and touches nothing that
is there: it draws a NEW cluster in a clean zone and hands you the old frames' URLs to delete.

Safe to run unattended: `init --yes`, `sonar`, `config` (no flag), `update --check`,
`login --status`, `deploy --entry-point <X>` (add `--dry-run` to see the layout first).

## deploy

```bash
bat-cli deploy                                        # fuzzy-pick (interactive)
bat-cli deploy --entry-point Vault.deposit            # Contract.function, or a bare function name
bat-cli deploy --entry-point Vault.deposit --dry-run  # print the layout, contact nothing
bat-cli deploy --entry-point Vault.deposit --preview /tmp/frame.png
```

**Every deploy is fresh.** It never recycles a frame and never links a frame that is already on
the board: the entry point and every branch cut out of it are drawn again, together, in a clean
region below everything else. That is the point — a diagram you are about to read should have its
frames next to each other and drawn by the rules in force today, not scattered across the board by
the deploys that happened to come before. Frames created earlier in the SAME run are still shared,
so a helper called from two branches is drawn once. The previous cluster is not deleted (the API
deletes one item at a time, and slowly): its still-live frame URLs are printed at the end so you
can delete them with one click each in Miro, where a frame takes its contents with it.

| flag | |
|---|---|
| `--entry-point <name>` | `Contract.function` or bare `function`; omit to pick from a list |
| `--dry-run` | compute and print the layout, never touch Miro (no login needed) |
| `--with-documentation` | start each screenshot at the function's NatSpec, so the documented intent is on the diagram |
| `--preview <path>` | compose the frame locally as a PNG |
| `--stroke-width <1-24>` | connector thickness in dp (default 8) |
| `--yes` | answer "this entry point already has a deployment — deploy again?" with yes (scripts, assistants) |
| `--ignore-contract <name-or-path>` | never draw this contract's functions, for this run (repeatable; adds to the saved list — see below) |
| `--inline-all` | draw the whole graph in ONE frame: no branch is cut out, every function is a screenshot |
| `--allow-unresolved` | draw the partial graph instead of stopping to list unresolved interface calls |

**A deployment is an entry point, and it owns every frame it drew.** `deploy --entry-point X`
produces one deployment: the frame for `X` plus a frame for every branch cut out of it, all
recorded together under `X`. Deploying `X` again REPLACES that deployment — it asks first, `--yes`
answers it — and leaves the previous frames on the board for you to delete by hand; no other
deployment is touched. Because every deploy is fresh, a helper reached by two entry points is drawn
once per deployment, so several frames on the board carry the same title with different ids. That is
by design, and it is why a frame is addressed as a name **inside a deployment**:

```bash
bat-cli screenshot                                            # lists frames, grouped by deployment
bat-cli screenshot Book --deployment FLAMM.previewMint        # the deployment's own root frame
bat-cli screenshot Book --deployment FLAMM.previewMint --dependency FLAMMFlowLib.requireFlat
```

A frame is addressed as **a deployment plus a name inside it**, never by title alone: a deployment
draws one frame per function, so the name is unique there, while the same title exists in every
other deployment that reached it. Without `--dependency` the target is the deployment's own frame.
A dependency that was drawn INSIDE the root frame rather than cut out to its own has no frame to
draw on, and the error lists what the deployment did draw. You never need a frame id.

**A type whose fields are types gets its own frame.** `bat-cli screenshot FLAMMSwapLib.Plan
--deployment <X> --dependency <F>` draws a single struct onto the frame, as before — but when that
struct holds other structs (`Plan` holds a `SwapContext`, which holds a `PoolContext`), drawing them
all inline would bury the function the frame is about. So the type becomes its own frame beside the
asking one, laid out by the same rules as a call graph with an arrow from each field to the type it
names, and the asking frame gets a **purple card** pointing at it — the same shape a branch cut out
to its own frame leaves behind, in the colour that means "type" rather than "call". The new frame
belongs to the same deployment, so `--dependency FLAMMSwapLib.Plan` reaches it afterwards.

An overloaded function carries its parameter types, `MMRouterLib.read(uint256,address)`, because
two frames of a deployment would otherwise share a name. The bare name still works when only one
of them is there; when both are, it lists them and asks for the types.

**"I already know that library" — `bat-cli ignore`.** A fixed-point math library called from
thirty places is thirty boxes saying the same thing, and it crowds out the code the review is
actually about: on one real frame, ignoring `Math` took it from 17 screenshots and 49 arrows to 12
and 15, and every remaining arrow stayed inside its own column.

```bash
bat-cli ignore Math                                     # by contract name
bat-cli ignore openzeppelin-contracts/contracts/utils   # or any part of a path
bat-cli ignore --list
bat-cli ignore --remove Math
bat-cli deploy --entry-point <X> --ignore-contract Math # just this run
```

The list lives in `BatMetadata.json` and survives a re-`sonar`, because it is a reading decision,
not something a scan can rediscover; a deploy leaves out the union of the saved list and this run's
flags, and says what it is not drawing. **Nothing about the audited code is hidden**: the calls are
still there in the callers' own screenshots, with their storage and boundary markings — it is the
callee's box that is left out. Deploy it as an entry point of its own on the day the question is
about it. Good candidates are utility maths, logging and string helpers under `lib/`. Bad ones are
anything that writes storage or moves value: that is the code the diagram exists for.

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

**Interface casts** — `IBeacon(addr).implementation()` — are followed when the answer is known: a
recorded `bat-cli resolve`, or a single IN-SCOPE implementation defining the method. Generic
implementations under `lib/` never count: `IERC20(token)` points at some deployed token, not at
OpenZeppelin's `ERC20` template, so a cast with no implementation in `src/` is treated as an
external contract and is never offered for `bat-cli resolve` (a resolution is global — binding
`IERC20` to a template would rebind every IERC20 call in the project). With several
implementations the deploy does not guess. If any of them can change state, the call joins the
unresolved list above and the deploy stops; if none can, it is a read, so the deploy prints a
`note:` naming the candidates and the `bat-cli resolve` that would draw it, and carries on.

**Any function can be deployed, not just an entry point.** A shared helper needs a frame of
its own for anything else to point at, and is worth reading on its own terms. Named with
`--entry-point`, that includes constructors, `fallback`/`receive`, and contracts under `lib/`
(the interactive picker still hides `lib/` and constructors). Contracts under `lib/` are drawn
like any other: if the code is in the repo, it is part of what runs.

`--entry-point` accepts three forms: `function`, `Contract.function`, and
`path/To.sol:Contract.function`. Several matches are narrowed without asking, in this order:
the project's own entry points, then its other functions, then `lib/`; then, among same-named
contracts in different files (three vendored copies of `BeaconProxy`), the one the audited code
imports, resolved through `remappings.txt` exactly as `solc` does. The deploy prints
`from <file>` when the name is shared, so you can see which copy it picked. **Only when the
code cannot decide does it stop** — two in-scope contracts both defining `poke`, or a library
nothing in `src/` imports — and it lists each candidate in the `path:Contract.function` form.
Re-run with one of those; do not guess.

**Creating a contract is a call to its constructor.** `new X(...)` (and `new X{salt: s}(...)`) is
drawn as an arrow to `X.constructor`, anchored on `X`; `new bytes(n)` and `new T[](n)` allocate
memory and are not calls.

**A constructor is drawn with the constructors it runs.** Base constructors execute before the
body whether the header invokes them (`BeaconProxy(beacon, data)`) or not, so deploying
`FLAMMProxy.constructor` draws `BeaconProxy.constructor` and what it calls — not every inherited
function, only what runs during construction.

**One frame per function, per cluster.** Inside a run, a function already drawn is pointed at
rather than redrawn, so a helper two branches reach is drawn once. Across runs nothing is reused:
deploying a function that is already on the board draws a new cluster and asks nothing (there is no
prompt any more), and prints the previous cluster's frame URLs for you to delete.

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
(e.g. an ERC-20 by address), via a non-view method (from `unknown_external_calls`) — **or a call
that resolves only into `lib/`**, like `SafeERC20.safeTransferFrom(...)`, which moves tokens in a
contract the repository does not contain. It means the
flow leaves the audited code and the callee MIGHT mutate its own state — unverified, so it is
deliberately distinct from the solid-red proven write. `view`/`pure` calls are never flagged.
A function that makes such a call but writes no storage of its own also gets a **solid amber
rectangle** around its whole node — the amber counterpart of the red storage border, so
"probably a state change here" reads at a glance alongside the proven-write nodes.
A call the deploy itself draws into in-scope code is never amber, even when it is listed in
`unknown_external_calls` (the scan finds implementers by inheritance only, so a contract that
matches the interface without declaring `is <interface>` stays listed there). `--dry-run` prints
the amber lines under "external boundary line(s)".

**What is reused, and what is not.** Nothing another deploy left on the board is reused: a deploy
draws its whole cluster fresh (see "Every deploy is fresh" above). Within one run, a callee that
this run already gave a frame is referenced with a link card instead of being redrawn with its
subtree. Interface/abstract stubs (bodyless declarations, empty `virtual {}`) are never drawn on
their own: a stub is redirected to its concrete override. Linking never thins a frame into a husk —
a callee is linked out only while this frame keeps at least 6 screenshots; otherwise it is drawn
inline and the deploy says so ("drawn inline despite having a frame"). So a two-line function does
NOT become one screenshot and two cards.

**A frame's background says whether it changes state.** Very pale red (`#fff0ef`) when something
drawn in it writes storage or reaches a write, pale amber (`#fff7ec`) when something in it only
calls out past the audited code, and red wins when both are true — the same precedence the per-node
markings have. It is the frame's own fill, and it exists for the distance where a whole cluster fits
on screen and a node's own red border is two pixels wide. Inside the frame, the per-function
markings are what answer the question precisely. Frames drawn before 0.26.11 stay white.

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

- **`contracts[].functions[].resolved_calls[]`** — interface calls the scan pinned by type to a
  single in-scope implementer, as `{receiver, method, contract}` (e.g. `$.priceFeed` → `PriceFeed`).
- **`file_items[]`** is the declaration index — structs, enums, errors, type aliases, constants,
  free functions and events — each with `file_path`, `line`, `end_line` and an `owner` naming the
  contract it was declared inside (empty when it sits at file level). Since 0.24 this includes
  declarations inside a contract, which is where Solidity puts most structs and enums.

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

## 0.26.14
- **A hiccup on the Miro API no longer deletes a deployment's records.** `item_exists` answered
  `false` for every failure — a rate limit, a 500, a dropped connection, an expired token — and the
  caller acts on that answer by FORGETTING the record ("the frame recorded for X is gone from the
  board"). So one unlucky request silently dropped a deployment from the registry and left its
  frames on the board with nothing able to address them. A 404 is an answer; everything else is the
  absence of one, and the record is kept.
  _Re-read: workflow.md._

## 0.26.13
- **A struct whose fields are structs is drawn as its own frame.** `screenshot <Struct>` used to
  put one screenshot on the frame; a nested type needs the whole tree, and inline that buries the
  function the frame is about. It now draws a frame beside the asking one — same layout rules as a
  call graph, an arrow from each field to the type it names — and leaves a purple card on the asking
  frame pointing at it. The frame belongs to the same deployment, so `--dependency <Struct>` reaches
  it later. Miro refuses to create a frame overlapping another, so free space is found by testing
  candidate rectangles against the frames the registry knows, spiralling out from the host.
- **The entry point sits at the top-left of its frame.** The tree layout centred each parent on its
  children, so on a tree-shaped graph the function you start reading from floated halfway down with
  empty space above it. It is anchored at the top of its band now, which is what the layered path
  always did, and leaves the free space below — where a declaration screenshot goes.
- **A deployment owns its frames, so two deployments no longer fight over a name.** The registry
  held one record per function name, board-wide — true while a callee already on the board was
  linked rather than redrawn. Since every deploy became fresh, a shared helper is drawn once per
  entry point, so the second deployment's record displaced the first's and those frames became
  unreachable from the CLI although they were plainly on the board. Records are now keyed by
  (deployment, frame), a deployment being the entry point its cluster was drawn for.
- **`screenshot` addresses a frame as `--deployment <entry point> [--dependency <function>]`.**
  `--frame` is gone: a title is not an address once the same helper is drawn in every deployment
  that reaches it. Inside one deployment a name IS unique, so that is the address; omitting
  `--dependency` means the deployment's own root frame. `bat-cli screenshot` with no arguments lists
  frames grouped by deployment.
- **Overloads carry their signature.** A frame for an overloaded function is named
  `Contract.read(uint256,address)` — the discriminator used to be a line number, which said nothing
  to a reader. The bare name still resolves when only one overload was drawn.
- **A deploy no longer prints the previous cluster's frame URLs.** It cost one API call per frame to
  check they were still there, and the frames are visible on the board anyway.
- **Deploying an entry point that already has a deployment asks first**, since it replaces that
  deployment and leaves the old frames on the board; `--yes` answers it for scripts and assistants.
  (`--yes` existed as a flag before 0.26.11 but had no prompt left to answer; it does now.)
  _Re-read: workflow.md._

## 0.26.12
- **Fixes to what 0.26.11 SAID, not to what it did.** `ignore --help` opened with `resolve`'s
  description (a misplaced doc comment); the workflow guide still described frame recycling and a
  "redeploy prompts yes/no" that no longer exist; the frame tinting shipped in 0.26.11 was only in
  the changelog, not in the workflow guide; and `deploy` was called a six-flag command when it has
  eight (`--allow-unresolved` and `--ignore-contract` were missed). `bat-cli ignore` also lists when
  given no pattern, so `--list` is a synonym rather than the only way.
  _Re-read: workflow.md._

## 0.26.11
- **`bat-cli ignore <NAME_OR_PATH>` — stop drawing a library you have already read.** Saved in
  `BatMetadata.json` (survives a re-`sonar`), with `--list`/`--remove`, plus
  `deploy --ignore-contract` for one run; a deploy leaves out the union and prints what it skipped.
  The calls stay visible in the callers' screenshots, markings included — only the callee's boxes
  go. On one real frame, ignoring `Math`: 17 screenshots and 49 arrows → 12 and 15.
- **Every arrow gets its own vertical lane.** Miro routes a connector itself, so every arrow
  leaving a caller turned on the same x and they stacked into what looked like one line. Forward
  arrows are now drawn as three straight legs between markers bat-cli places — out at the call
  line, down this arrow's own lane in the gutter, in at the callee's signature — so nothing is left
  for Miro to route. Lanes are ordered by where each arrow starts and ends, which keeps arrows that
  need not cross from crossing, and narrow automatically when a gutter cannot fit them.
- **A frame says whether it changes state, in its own background.** Red (`#ffc6c6`) when something
  drawn in it writes storage or reaches a write, amber (`#ffe2c2`) when something in it only calls
  out past the audited code. The per-node red border answers that question once you are inside a
  frame; from a cluster of thirty frames, which is where you decide what to open, it is two pixels.
- **A frame says whether it changes state, in its own background.** A very pale red (`#fff0ef`)
  when something drawn in it writes storage or reaches a write, a pale amber (`#fff7ec`) when
  something in it only calls out past the audited code; red wins when both are true, the same
  precedence a node's borders have. The per-node border answers that question once you are inside a
  frame — from the distance where a whole cluster fits on screen, which is where you decide what to
  open, it is two pixels wide.
- **Arrow colours come from a conflict graph, not a ranking.** Two arrows never share a colour when
  a reader has to tell them apart — running in neighbouring lanes of one gutter, leaving the same
  screenshot, or landing on boxes that sit next to each other in the next column. Two arrows that
  reach the SAME function still share one colour, so a helper drawn in several places stays
  recognisable. (Ranking by depth gave the same colour to the first callee of one column and the
  first of the next, which are usually the two arrows side by side in the gutter.)
- **Repeating a callee and enlarging one are charged separately.** A copy beside a far caller is
  what removes a crossing arrow, and one constant decided both how big a thing could be copied and,
  implicitly, how often — so a helper the size of a getter was still copied for every caller that
  reached it (one contract: 203 boxes for 30 functions). Past three far callers a callee now gets a
  frame of its own and a card beside each caller, however small it is.
- **A call that flies over a column can become a card.** When a shared callee is too big to copy
  next to a far caller (its closure reaches the frame floor), that ONE call is replaced by a card to
  the callee's own frame; the caller sitting next to it keeps reading it as a screenshot.
  `--dry-run` now reports how many connectors fly over a column, and the worst span.
- **A frame is cut until it reads, not ten times.** The framing loop gave up two ways without
  saying so: a fixed ten passes, and a cut budget that never moved, so "nothing worth cutting AT
  THIS SIZE" was treated as "nothing worth cutting" and the rest was drawn as one wall —
  `FLAMMSwapLib.execute` landed on the board as 250 screenshots and 641 connectors. It now lowers
  the budget (down to the husk floor) before giving up, and says so when it truly cannot cut.
  Same function, same rules: 16 screenshots.
- **Localization checks its premise.** Copying a small helper next to each far caller is cheap
  *because the frame is already small*. On a frame framing could not bring under the readable max
  that premise is false, so it is skipped rather than adding screenshots to a wall.
- **Callees are drawn in call order.** The crossing count keyed each edge by its caller's slot, so
  two edges leaving the SAME caller were never counted as crossing — the metric was blind to the
  disorder a reader notices first, and a leaf (no successors) was sorted to the bottom of its
  column. Both fixed: on one real entry point the root's 18 callees now sit in source order, with
  the only exception a function genuinely called from two lines.
- **The per-column x stagger is gone**, along with the frame width it cost.
- **`deploy` is one command with eight flags, and it is always fresh.** A deploy no longer recycles
  the frame it finds, and no longer links a callee that already has a frame somewhere else: the
  entry point and every branch cut out of it are drawn again, together, in a clean region. The
  reason is that a recycled frame is not reproducible — what it drew depended on which frames
  happened to exist and where they sat — while the diagram you are about to read wants its frames
  next to each other and drawn by today's rules. The previous cluster's URLs are printed for
  one-click deletion in Miro.
- **Gone: `--redeploy`/`--fresh-frames` (now the only behaviour), `--recycle`, `--refresh-links`,
  `--undeploy`, `--yes`, `--all`, `--max-depth`, `--max-nodes`.** Delete a frame in Miro yourself —
  the web UI deletes it with its contents, which the API cannot do. `--max-depth`/`--max-nodes`
  truncated the graph, which hides code from an audit; the frame is bounded by cutting branches
  out to their own frames instead.
- **`lib/` is always drawn.** `--include-external` is gone: if the code is in the repo it is part
  of what runs, so a call into a vendored library is followed like any other.
  _Re-read: workflow.md._

## 0.26.10
- **`--fresh-frames`: deploy with its own frames, ignoring the board.** Nothing new happens — this
  is the existing `--redeploy` under a name that says what it does. A plain deploy reuses the board
  (it recycles this entry point's frame and turns an already-framed callee into a link card);
  `deploy --entry-point <X> --fresh-frames` draws a self-contained cluster instead, recycling
  nothing and linking no pre-existing frame. Reach for it when a diagram has drifted and you want
  one built from scratch rather than patched. `--redeploy` keeps working and means exactly the same.
  _Re-read: workflow.md._

## 0.26.9
- **A transfer through `lib/` is marked as the external boundary it is.**
  `SafeERC20.safeTransferFrom(...)` — a library call written by name rather than on an
  interface-typed receiver — passed through both nets: the scan counts `SafeERC20` as a known
  contract, and the deploy drops it for living in `lib/`. So a line that moves tokens showed
  nothing at all. Any call that resolves only into `lib/` and is not `view`/`pure` now gets the
  amber boundary band, the same marking an unresolvable interface call gets.
  _Re-read: workflow.md._

## 0.26.8
- **A frame that was cut and pasted no longer orphans its record.** Miro gives a pasted frame and
  all its children new ids, and bat-cli reacted by DELETING the registry entry — so by the time
  anyone noticed, the thing that would have let them repair it was already gone, and the only way
  back was a redeploy that placed the frame by auto-layout instead of where the auditor had put
  it. Now `deploy` and `screenshot` find the frame by its title and rebuild the record from the
  board, matching each screenshot by the title it carries.
- **`bat-cli relink`** re-anchors on demand: `--check` reviews the whole registry and says what
  moved, `relink <entry point>` re-anchors by title, and `--frame-url <url>` picks one when
  several frames share a title (the deploy stops and lists them rather than guessing).
- **`--undeploy` deletes the frame's live children** instead of the ids it recorded, so a pasted
  frame is still cleaned up completely.
  _Re-read: workflow.md._

## 0.26.7
- **No amber on a call the diagram follows into the repo.** An interface declared next to its
  caller (`interface ITrancheController` in `TrancheToken.sol`) and implemented by a contract that
  never writes `is ITrancheController` has no implementer by inheritance, so the scan lists the
  call in `unknown_external_calls` — yet the deploy draws its arrow to the in-scope
  `TrancheController.depositFor`. The line was painted as an external boundary anyway; now a call
  with an arrow into in-scope code is never amber. `--dry-run` also lists the amber lines, so you
  can check them before deploying.
- **`this.x()` and `super.x()` are drawn.** The call extractor treated `this` and `super` as
  keywords and dropped the whole call, so `try this.cross(base, quote)` in
  `PriceFeed.peekCross` left the diagram with no arrow at all, though the resolver already follows
  both. They are kept now.
- **Two functions called on one line keep their own colours.** Arrows from the same caller line
  share one stub, and the whole group took the first callee's colour — so in `_usd(_token(x))` the
  branch into `_token` was painted `_usd`'s colour, and the two read as one. Each branch now takes
  the colour of the function it reaches (the shared stub keeps the first).
- **No husk frames from already-deployed callees.** Deploying a function's callees as their own
  frames turned the function into a husk on its next redeploy (`FLAMMGateLib.priced`: one
  screenshot, two link cards). Linking to an existing frame now keeps the same floor as the
  automatic cut: a callee is linked out only while the frame keeps 6 screenshots, else it is drawn
  inline. _Re-read: workflow.md._

## 0.26.6
- **A struct used from another contract types correctly.** `MMRouterLib.Venue storage v = …` in a
  contract that does not declare `Venue` itself fell back to a project-wide lookup by bare name,
  where an unrelated `Venue` (with `address account` instead of `IFinancingAccount account`) could
  win — so `v.account.supply(...)` typed as `address` and no `bat-cli resolve` could fix it. Struct
  types now resolve by their qualified spelling first, and a bare name prefers the struct declared
  in a file this contract imports.
- **An interface with no in-scope implementation is an external boundary, whatever the receiver.**
  A call like `MORPHO.supply(...)` on an `IMorphoBlueBorrow` variable asked to be resolved and
  offered candidates that merely define a method of that name — a resolution with no right answer,
  since the contract lives outside the repo. The rule that already covered casts (`IERC20(token)`)
  now covers wired variables too, so these are flagged as the external boundary they are.
  _Re-read: nothing (fewer false questions; shape unchanged)._

## 0.26.5
- **`screenshot` leaves the frame alone and places tightly.** Free space was computed from the
  registry, which stores each screenshot's PNG size — but a deep node is drawn SCALED DOWN, so the
  content looked far taller than it is and every addition was pushed below it, stretching the frame
  by a screenful each time. Placement now asks the board what the frame actually holds, so a small
  struct lands right under the content. The frame is never moved or resized: a drawing with no room
  left goes to the bottom-left corner (overlapping, which is visible and one drag away), and
  `--grow` extends the frame downwards from its LIVE geometry when that is what you want.
- **Drawing the same symbol twice is a no-op** — it says it is already on that frame instead of
  stacking an identical copy.
- **Interface function declarations are indexed.** `bat-cli screenshot IMorphoBlue.market` used to
  answer "no declaration named", while the same interface's structs resolved. Functions, modifiers
  and events now resolve too, from any contract or interface. The not-found message also stops
  suggesting a rescan when the project was already scanned by the running version.
  _Re-read: workflow.md._

## 0.26.4
- **Calls through a storage pointer received as a PARAMETER are typed too.** A library written
  against `read(Venue storage v)` calling `v.account.tryPosition(...)` had an untyped receiver:
  the body analysis never sees the signature, so parameters and named returns were missing from
  the type table. They are added now, and struct names are resolved per contract — two different
  `Venue` structs (one with `IFinancingAccount account`, one with `address account`) used to
  collide on the bare name, so whichever was parsed last decided the type for both. On one pool
  codebase the typed-call count went from 53 to 279. Rescan with `bat-cli sonar`.
  _Re-read: nothing (the scan records more, the shape is unchanged)._

## 0.26.3
- **Calls through a storage-struct field are drawn.** `$.priceFeed.pegOk(...)` — a field of a
  storage struct typed as an interface with ONE in-scope implementation — was typed correctly by
  the scan and then discarded as "the deploy follows it", which the deploy cannot: only the scan
  knows the struct's field types. The scan now records these in `resolved_calls` and the deploy
  draws them (and follows them for state-change marks). On one pool codebase this recovered 53
  such calls. Rescan with `bat-cli sonar`; a deeper graph can surface interface calls that now
  need a `bat-cli resolve`.
- **Interface casts ignore `lib/` implementations**, as the scan always did: `IERC20(token).balanceOf`
  is no longer reported as "3 implementations (ERC20, ERC20Upgradeable, ERC777)" with a `resolve`
  suggestion that would bind every IERC20 call to a template. Left-out reads are printed once per
  interface method, with every call location, instead of once per call.
  _Re-read: workflow.md, metadata.md._

## 0.26.2
- **`new X(...)` is drawn as a call to `X.constructor`.** Contract creation was not recorded as a
  call at all, so a factory like `MorphoAccountDeployer.deploy` — whose whole job is
  `new MorphoBlueAccount(...)` — drew as one screenshot with `callees: []`. Both the deploy and the
  scan now treat creation (salted too) as a call to the created contract's constructor. Rescan with
  `bat-cli sonar` to refresh the stored call graph; the deploy sees it without one.
  _Re-read: workflow.md._

## 0.26.1
- **Calls inside a cast are no longer lost.** In `return IBeacon(_getBeacon()).implementation();`
  both call extractors dropped `_getBeacon()` — the deploy walked the cast but not its argument,
  and the scan ignored any call whose receiver was not a plain name — so
  `BeaconProxy._implementation` drew as a lone screenshot. Rescan with `bat-cli sonar` to refresh
  the call graph the scan stores; the deploy picks the argument up without one.
- **Interface casts resolve like interface-typed variables.** `IFace(addr).method()` follows a
  `bat-cli resolve` or a single implementation. Several implementations stop the deploy when one
  can reach a write, and only print a `note:` when they are reads.
  _Re-read: workflow.md._

## 0.26.0
- **Deploy constructors, `fallback`, and contracts under `lib/` by name.** `--entry-point` used to
  refuse anything outside the project's own non-constructor functions, so an empty
  `FLAMMProxy.constructor` — whose whole behaviour lives in the `BeaconProxy` it inherits — could
  not be drawn at all. Named explicitly, anything is reachable now; the picker is unchanged.
- **A constructor is drawn with the base constructors it runs.** `constructor(...)
  BeaconProxy(beacon, data) {}` was parsed as a modifier that resolved to nothing, so the diagram
  stopped at one screenshot. The base constructor is now an edge anchored on that header token,
  and implicit base constructors are followed too.
- **A contract name resolves through imports, like the compiler.** With several vendored copies
  of a library under `lib/`, a name used to mean the first copy found in the whole project —
  possibly one the audited code never touches. Every lookup (the root, base contracts, library
  calls, interface implementations) now picks the copy reachable through the using file's
  imports and `remappings.txt`, nearest first. `--entry-point` accepts
  `path/To.sol:Contract.function`, prints `from <file>` when a name is shared, and stops with the
  candidates listed only when the code cannot decide.
- **A root under `lib/` includes external calls automatically**, so `bat-cli deploy --entry-point
  BeaconProxy.constructor` draws the whole construction without remembering `--include-external`.
- Remappings now apply longest prefix first, as forge does.
  _Re-read: workflow.md._

## 0.25.0
- **A state change is never invisible again.** A node was drawn red only when it held the
  assignment itself, so a chain of pass-throughs looked inert:
  `PositionManager._increaseDebt` calls `debtToken.mint`, which calls `ERC20._mint`, which in
  OpenZeppelin v5 only validates and delegates to `_update` — the one function in the chain that
  assigns anything. Every hop in between, and the call lines reaching them, were unmarked.
  Now the change is marked ONCE, at the deepest point the frame reaches: on the assignment when
  the function holding it is drawn, otherwise on the call that leaves the frame towards it. One
  red mark is one state change, so they can be counted. The reachability walk runs over the metadata call graph and always crosses into `lib/`,
  so the mark survives a chain cut short by framing, `--max-depth`, or leaving
  `--include-external` off — you see the state change even when the function that performs it is
  not on the frame. On a real project this moved 293 marked functions to 401, not a flood.
  `--dry-run` now prints a `state` column (`write` / `→write`) and lists the call lines that
  reach a state change. One limitation worth knowing: an interface left unresolved
  (`bat-cli resolve`) stops this walk exactly as it stops the graph.
  _Re-read: workflow.md._

## 0.24.0
- **`bat-cli screenshot` — put any declaration on a frame you are reading.** A function's
  screenshot names things it cannot explain: the state variable it compares against, the struct it
  takes as a parameter. Name the symbol and its source lands on the frame, in free space, for you
  to drag where you want it — `bat-cli screenshot deviationThresholdWad --frame CvammALM.poke`.
  It resolves `Name` or `Contract.Name` from the scan, stops and lists the candidates when a name
  is declared in several contracts, and takes `--file`/`--lines` for anything the index misses, so
  it never blocks. Run it with no `--frame` to list the deployed frames. Accepts
  `--with-documentation` and renders exactly like `deploy`, so the image matches the diagram.
  There is deliberately no rule about what deserves to be drawn: that judgement is the auditor's.
- **Structs and enums declared inside a contract are now indexed.** Solidity puts most of them
  there rather than at file level, and the scan used to parse them for type inference and then
  throw the location away — on one real project that meant 5 of 174 types were locatable. They now
  carry their file and line range, qualified by the contract that declares them
  (`PriceFeed.FeedType`). Rescan with `bat-cli sonar` to pick them up.
  _Re-read: workflow.md, metadata.md._

## 0.23.1
- Documentation only: the guide and README no longer name functions from the codebase this was
  developed against, and the README's framing description now matches the current partitioning
  (target ~15 screenshots, floor of 6) instead of the superseded 45/65 caps. _Re-read: nothing._

## 0.23.0
- **`deploy --with-documentation` puts the NatSpec on the diagram.** Each screenshot normally starts
  at the function signature, so the `/// @notice …` above it — the author's statement of what the
  function is FOR — never made it onto the board, and the reviewer had to read intent back out of the
  code. With the flag, a screenshot starts at the top of the NatSpec block written directly above the
  declaration: a run of `///` lines or one `/** … */` block, modifiers included. An ordinary `//`
  note is left out (it is a remark, not documentation), and so is a comment separated from the
  declaration by a blank line, which belongs to whatever came before it as often as not. Arrows still
  land on the exact calling line — the anchors shift by however many documentation lines were added —
  and the screenshots are cached separately, so a documented and an undocumented deploy never reuse
  each other's images. Everything is unchanged without the flag.
  _Re-read: workflow.md._

## 0.22.13
- **Columns are top-aligned, so a tall caller's arrows stop crossing.** Deeper layers used to be
  centred vertically against the tallest column. When the entry point is a long function at the
  top-left, centring pushed its callees to the middle of the frame, so arrows from its top AND
  bottom call sites both converged inward and crossed. Every layer now starts at the top: the
  cascade flows down-and-right from the top, the fan-out stays monotonic (no convergence), and deep
  chains no longer drift far down. _Re-read: nothing (layout only)._

## 0.22.12
- **Boards bat-cli creates are PRIVATE.** When `init` creates a Miro board for you, it now sets the
  sharing policy so only you can open it — `access`, `teamAccess` and `organizationAccess` are all
  `private`, so no link, no teammate, and no one else in the org can see it. Audit diagrams are
  sensitive, and on a Business/Enterprise org a new board is team-shared by default; this locks it.
  _Re-read: nothing (behaviour only)._

## 0.22.11
- **The board picker only lists boards you OWN, and no longer hangs.** `init` (and any board
  selection) used to fetch every board you can see — on a big org that is hundreds (243 in one case,
  five silent pages), so it looked hung, and it offered boards owned by other people that you could
  accidentally edit. It now filters by owner on the server (`?owner=<your user id>`), so the list is
  just your own boards (3 instead of 243) and returns instantly, with a spinner while it loads.
  _Re-read: nothing (behaviour only)._

## 0.22.10
- **Readable frames for tangled graphs: frame the big shared subtrees, localize the small crossers.**
  A function reused all over a diagram used to draw arrows that cross every screenshot, turning a big
  frame into an unreadable mesh (Miro auto-routes connectors, so the layout can't bend around them).
  Now: (1) big shared subtrees (≥ 6 screenshots) are cut to their own sub-frames; (2) THEN the small
  helpers still crossing (a caller ≥ 2 columns back) get a local copy per far caller — cheap, and no
  longer whack-a-mole because the deep floor is already framed out. On the entry point this was
  developed against, the main frame went from a 178-screenshot mesh to a readable ~37.
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
  several times (Solidity overloads — e.g. a public `quote(...)` that forwards to an
  internal `quote(curve, ...)`), the deploy used to map every call to the FIRST definition,
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
  `_positionValue` that just calls the shared `getPositionValue`) used to be drawn as a
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
  drawn straight to that contract — e.g. `pool.getReserves(...)` → `Pool`. Before,
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
