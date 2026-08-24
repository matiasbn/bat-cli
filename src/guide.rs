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
other function, with `(deployed)` on what is already on the board — pass `--entry-point` to skip it.

Safe to run unattended: `init --yes`, `sonar`, `config` (no flag), `update --check`,
`login --status`, `deploy --entry-point <X>` (and `--dry-run`).

## deploy

```bash
bat-cli deploy                                        # fuzzy-pick (interactive)
bat-cli deploy --entry-point Vault.deposit            # Contract.function, or a bare function name
bat-cli deploy --entry-point Vault.deposit --dry-run  # print the layout, contact nothing
bat-cli deploy --entry-point Vault.deposit --preview /tmp/frame.png
```

| flag | |
|---|---|
| `--entry-point <name>` | `Contract.function` or bare `function`; omit to pick from a list |
| `--dry-run` | compute and print the layout, never touch Miro (no login needed) |
| `--preview <path>` | compose the frame locally as a PNG |
| `--max-depth <n>` / `--max-nodes <n>` | bound the graph by hand; unset draws all of it |
| `--include-external` | include contracts coming from `lib/` |
| `--stroke-width <1-24>` | connector thickness in dp (default 8) |
| `--all` | every entry point at once — **discouraged**; it warns and asks first |

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
    `modifiers`, `params`, `returns`, `line`, `end_line`, `is_constructor`.
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
Match the exact string (the recipe below finds writers of one var).
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
