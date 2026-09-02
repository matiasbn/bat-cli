# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & test

```bash
cargo build                 # dev build (lto + codegen-units=1, so it is slow — run it ONCE per change)
cargo build --release
cargo test                  # unit tests live in `#[cfg(test)] mod tests` inside src/batbelt/parser/* and sonar_interactive.rs
cargo test type_resolver    # single module
cargo test -- --nocapture some_test_name
```

The dev profile uses `lto = true` and `codegen-units = 1`; builds are expensive. Do not chain multiple build invocations or `touch` files to force rebuilds.

Running the CLI against a real project: the binary must be executed from the root of the audited repository, where `Bat.toml` and `BatMetadata.json` live. Test projects are kept in a gitignored `test-workspace/` and driven via each project's `package.json` scripts (`cargo run --manifest-path ../../Cargo.toml -- init`); see `PACKAGE.md` for the template.

## Releasing (git-flow)

The repo uses **git-flow** (`main` = master, `develop` = develop, version tags carry the **`v`
prefix**, e.g. `v0.18.0` — `gitflow.prefix.versiontag` is `v`, so pass the bare `X.Y.Z` to the
git-flow commands and let it add the `v`). The CLI carries no release tooling of its own. To
cut `X.Y.Z`:

1. `git flow release start X.Y.Z` — branches off `develop`.
   - Confirm the top `## X.Y.Z` heading of the `CHANGELOG` const (see "Telling the AI what
     changed" below) matches the version actually being cut, and fix it if the guess was off.
2. Bump `version` in `Cargo.toml`, then `cargo build` so `Cargo.lock` updates. Commit both
   with the message `version bump`.
3. `git flow release finish X.Y.Z` — merges into `main` and back into `develop` and tags.
   Supply a **one-line release description** as the tag message. Non-interactively on macOS,
   where git-flow's `getopt` rejects `-m "with spaces"`, pass it through the editor:
   ```bash
   printf '%s\n' "<desc>" > /tmp/tagmsg && \
     GIT_MERGE_AUTOEDIT=no GIT_EDITOR='cp /tmp/tagmsg' git flow release finish X.Y.Z
   ```
4. `git push origin main develop --tags`.
5. Publish and install — **both**, always, as the last step, without being asked:
   ```bash
   cargo publish                              # run in a background agent, it takes minutes
   cargo install --path . --force --locked
   bat-cli --version                          # must print the X.Y.Z just released
   ```
   Installing from the local path rather than waiting for crates.io means the new version is
   usable immediately. `--locked` is required: without it `cargo install` re-resolves
   dependencies and pulls transitive crates needing a newer rustc than `Cargo.lock` pins.
   If the install fails or `--version` does not match the tag, surface it before moving on.

**Changelog convention — 1 PR = 1 release.** Every PR with code changes results in its own
release, so each PR owns exactly one `## X.Y.Z` heading and there is never contention over a
shared version. Write the entry under your **best guess of the next version** (usually the
next patch); whoever cuts the release fixes the heading if the ordering changed.

### Doc-only changes → hotfix, do NOT advance the version

When a change touches **only non-compiled files** — `README.md`, `CLAUDE.md`, `PACKAGE.md` —
ship it as a hotfix that keeps the current version: no `Cargo.toml` bump, no tag, no publish.
Just get the commit onto `main` and `develop`:

```bash
git checkout main && GIT_MERGE_AUTOEDIT=no git merge develop && \
  git push origin main develop && git checkout develop
```

Reserve version bumps and git-flow releases for changes under `src/` — anything compiled into
the binary.

## Telling the AI what changed

An auditor drives bat-cli by talking to an assistant, not by reading `--help`. So a new
capability is only half-shipped when it compiles: the assistant-facing guide has to learn it in
the same change. `src/guide.rs` owns that surface. The design comes from `rover` (`~/rover/src/guide.rs`) with
one deliberate departure: **everything here is machine-global.** rover writes its guide per
project because it owns the folder it writes into; bat-cli does not — its `Bat.toml` sits at
the root of somebody else's repository, and four generated markdown files next to it are
litter in a tree the auditor never asked us to touch. The guide also describes the *binary*,
not the project, so one copy per machine is the honest granularity.

- **The guide, version-stamped.** `ensure_ai_guide` writes `README.md`, `workflow.md`,
  `metadata.md` and `changelog.md` into `<config_dir>/ai_context/` (so
  `~/.config/bat-cli/ai_context/`, honouring `XDG_CONFIG_HOME` / `BAT_CLI_CONFIG_DIR`), from
  consts in `src/guide.rs`, replacing `{BAT_CLI_VERSION}` with the running version. The binary
  generates them, so they cannot drift from the code.
- **The routers, version-agnostic and byte-stable.** `ensure_global_ai_skills` installs
  `~/.claude/skills/bat-cli/SKILL.md`, `~/.agents/skills/bat-cli/SKILL.md` and a managed block
  in `~/.gemini/GEMINI.md`. They carry **no instructions** — they only say where the guide
  lives. Keeping them byte-stable is the point: upgrading bat-cli never rewrites them, so a
  running assistant session never needs a second restart. Do not put version-specific content
  in `GLOBAL_SKILL_MD`/`GLOBAL_AGENTS_BODY`.

`refresh_ai_surface()` does both, and `main::run` calls it **before every command** — including
`config` and `update`, which have no project at all. So the guide and the skill are checked on
every single bat-cli run.

There is deliberately **no `build.rs`**: cargo runs nothing after `cargo install`, and the only
hook that would fire during it is a build script, which would also run on every `cargo build`
here and in CI and would be writing into the user's `$HOME` from a compile. First use is close
enough — the README's very next step after installing is `bat-cli login`. Two things close the
remaining gaps: the hidden `bat-cli refresh-ai-guide` subcommand publishes the guide on demand
with no project, and `update_commands::publish_new_guide` runs it **on the binary just
installed**, because the updating process is the outgoing version and would otherwise leave the
machine describing the version it just replaced. Every write goes through `write_if_changed`, so a no-op run touches nothing;
`write_managed_block` appends to a `GEMINI.md` the user already wrote rather than clobbering it.

`Bat.toml`'s `bat_cli_version` is a *different* signal from the guide's stamp, and the docs say
so: the guide's stamp is which binary is installed, while `Bat.toml`'s is **which binary last
scanned this project** — i.e. whether `BatMetadata.json` came from the parser you are running
today. It is written by `record_bat_cli_version`, best-effort, and simply does nothing when
there is no `Bat.toml` here.

**So, when you change user-facing behaviour:**

1. Update the matching const in `src/guide.rs` (`WORKFLOW` for commands and flags, `METADATA`
   for the `BatMetadata.json` shape and its `jq` recipes, `README` for the golden rules).
2. Add a `CHANGELOG` entry at the top, newest first, under your best guess of the next version,
   ending in a `_Re-read: …_` line naming exactly which docs the entry invalidates. That line
   is the whole point — it is what lets an assistant learn what is new from one short file.
3. Ship both in the same commit as the code. Treat it like the README: not optional.

Note `src/guide.rs` is compiled into the binary, so changing the guide is a normal versioned
release, **not** a doc-only hotfix.

## Commit after each successful change

After a change builds, commit it without being asked, with a concise message, keeping the
doc updates in the same commit as the code they document. Commit to the working branch
(normally `develop`), never straight to `main`.

## Architecture

bat-cli is a single binary (`src/main.rs`) whose `BatCommands` clap enum dispatches to `src/commands/*`: `init`, `login`/`logout`, `update`, `config`, `sonar`, `refresh-ai-guide`, `deploy` and `resolve`. Every command first runs `validate_command()`, which only checks that the metadata cache exists for the commands that read it (`deploy`). There is no branch check: bat-cli creates no commits and manages no git.

**`deploy` is where the complexity lives.** `src/batbelt/evm/miro/auto_deploy.rs` (~4.2k lines) plus `src/batbelt/miro/layout.rs` turn a Solidity call graph into a readable Miro diagram, and the design decisions behind it — the pipeline order, framing, localization, overload handling, `--redeploy`, and a list of approaches that were tried and FAILED — live in **`docs/diagram-deploy-design.md`**. Read that before touching either file; it is the "why" the code does not carry.

`src/guide.rs` owns the AI-facing surface — see "Telling the AI what changed" above.

`src/batbelt/` is the library ("bat belt") holding everything else. Errors use `error_stack` throughout: each module defines a unit error struct (`ParserError`, `MetadataError`, `MiroError`, …) plus a `XResult<T>` alias, and calls `.change_context(...)` when crossing module boundaries.

### Two parallel stacks: SVM and EVM

`ProjectType` (`src/config.rs`) is `Anchor | Pinocchio | VanillaSolana | Foundry | GenericRust` and is the top-level branch for nearly every flow:

- **SVM (Rust)** — `batbelt::{sonar, parser, metadata}`, persisted to `BatMetadata.json` via `BatMetadata`.
- **EVM (Solidity/Foundry)** — a mirrored tree under `batbelt::evm/{sonar,parser,metadata,miro,templates}`, persisted via `EvmBatMetadata`. Parsing uses `solar-parse` (real Solidity lexer), with its own import resolver (remappings/`lib/`/`node_modules`), C3-linearization inheritance resolver, and call resolver.

`SonarCommand::execute_run` forks on `ProjectType::Foundry` into `EvmSonar` (a 5-phase scan over `..`) vs. the SVM path. Anything touching metadata must handle both stacks.

### SVM scan pipeline (`sonar`)

`SonarCommand::execute_run_svm` backs up `BatMetadata.json` to `BatMetadata_backup.json` (restored on a crashed previous run), preserves the `miro` section across rescans, regenerates the file, then:

1. `BatSonarInteractive::GetSourceCodeMetadata` — walks all program paths, extracts functions/structs/traits/enums into `source_code`.
2. `BatSonarInteractive::run_post_scan_parallel()` — **order matters**: traits are built first, because `CallResolver` needs `trait_metadata` to resolve `ctx.accounts.method()` / `self.method()` to the right impl block. Then function dependencies and entry points run *sequentially* (both call `FunctionParser`, which read-modify-writes the metadata file); context accounts run on a side thread since they are independent.

`BatMetadata::save_metadata` and friends serialize through a process-global `METADATA_FILE_LOCK` mutex — any new read-modify-write of `BatMetadata.json` must take it.

### Dependency resolution (`src/batbelt/parser/`)

Deterministic, three-layer, built on `syn`:

- `file_scope.rs` — per-file `use`/local-item scope, maps a bare identifier to a canonical path.
- `type_resolver.rs` — resolves receiver/param types (including `Context<T>`'s inner `T`).
- `call_resolver.rs` — turns each call site into `Resolution::Internal(MetadataId) | External(String) | Unresolved(String)`. It deliberately **refuses to guess**: when a name is ambiguous (e.g. 20 functions called `process`) it emits `Unresolved` and logs, rather than picking the first match. Read the module docs at the top of `call_resolver.rs` before changing resolution rules.

`entrypoint_parser.rs` models an entry point with a recursive `dependencies: Vec<FunctionSourceCodeMetadata>` (there is no "handler" concept). Context accounts have three parsers: `syn_context_accounts_parser.rs` (Anchor `#[account(...)]`, `has_one`, `seeds`, `constraint`), `pinocchio_context_accounts_parser.rs` (heuristics over `TryFrom` impls), and the legacy regex-based `context_accounts_parser.rs`.

### Metadata model (`src/batbelt/metadata/`)

Both stacks persist to the same file, `BatMetadata.json` at the repo root, but with different shapes.

**SVM** — `BatMetadata` = `{ source_code, entry_points, function_dependencies, traits, context_accounts, miro }`. Every item carries a random 30-char `MetadataId`; cross-references (dependencies, entry points, Miro items) are stored by id, never by name. The `BatMetadataParser<U>` trait is the common interface (`name/path/metadata_id/start_line_index/end_line_index/metadata_sub_type`) implemented by each `*_source_code_metadata.rs`.

**EVM** — `EvmBatMetadata` = `{ contracts, entry_points, function_dependencies, interfaces, file_items, miro, resolutions }`.

- `contracts[]` nests `functions`, `state_variables`, `events`, `modifiers`; `external: true` marks anything from `lib/`.
- `miro` is the deploy registry — `miro.auto.frames[]` of `AutoDeployedFrame`, keyed by `entry_point` (the frame's ROOT function) plus `cluster_root`, each carrying `images`, `image_dims`, `node_positions`, `callee_connectors` and `link_cards`. This is what makes `--refresh-links`, `--undeploy` and `--redeploy` surgical. Note it records **which functions own a frame**, not every function drawn: the same function appearing as a non-root screenshot in two frames is drawn twice, on purpose (framing cuts branches out; `duplicate_crossing_shared` copies small helpers to shorten arrows).
- `resolutions` maps an interface type name to the concrete in-scope contract it binds to at runtime — what static analysis cannot pin. `deploy` STOPS and lists what to resolve (unless `--allow-unresolved`); `bat-cli resolve <INTERFACE> <CONTRACT>` records one (`--list`, `--remove`).

**`miro` and `resolutions` are preserved across a `sonar` regeneration.** Anything else you add to `EvmBatMetadata` that a rescan must not destroy has to be preserved explicitly too.

### Paths & workspace layout

Never hardcode audit paths: `BatFile` and `BatFolder` (`src/batbelt/path.rs`) are enums that resolve every path from `Bat.toml`. A project is **two files at the root of the audited repository** — bat-cli owns no folder there, which is why the AI guide is machine-global (see above) and why screenshots go to the system temp dir:

```
<audited repo>/
├── Bat.toml           # BatConfig: project_type, program/src paths, miro board, bat_cli_version
└── BatMetadata.json   # sonar cache + the Miro deploy registry
```

`BatFolder::Figures` points at `$TMPDIR/bat-cli/<project>/`, not into the repo. There is **no `bat-audit/` directory, no `BatAuditor.toml`, and no `code-overhaul/`** — the code-overhaul workflow and the per-auditor config were removed; stale references to them survive only in a few doc comments (`miro/auth.rs`, `config.rs`).

Config is loaded with `figment`/`confy`; `BatConfig::get_config()` is called ad hoc from deep in the tree, so a valid `Bat.toml` in the cwd is a precondition for most code paths.

### Miro integration

`batbelt/miro/client.rs` wraps the Miro REST API (frames, images, shapes, connectors) with the machine-wide OAuth token from `~/.config/bat-cli/`. It carries a credit budget, a concurrency semaphore, and retries on 429/5xx. Screenshots are rendered locally by `batbelt/silicon.rs` (silicon + syntect, Dracula theme, background `#282a36`) into the system temp dir, uploaded, then deleted.

`batbelt/evm/miro/auto_deploy.rs` orchestrates a deploy; `batbelt/miro/layout.rs` holds the pure, testable layout (Sugiyama for graphs, Reingold–Tilford for trees, shelf packing for board-level frame placement).

`deploy_one`'s pipeline is **build graph → render → recycle/link → framing → localize → layout → upload**, and that order is a decision, not an accident (localizing before framing was whack-a-mole). The constants that tune it (`FRAME_TARGET`, `FRAME_MAX`, `FRAME_MIN`, `CROSS_LAYERS`, `MAX_CLOSURE`, `REFERENCE_FONT`, …) and the reasoning are in `docs/diagram-deploy-design.md` §2–§6 and §12. **The single hard constraint:** Miro auto-routes connectors — you set the two endpoints, never the waypoints — so a crossing can only be removed by making the callee LOCAL to its caller, never by a smarter layout. And the auditor's rule: never crop, fold or hide source; "make it fit" means more frames or a repeated screenshot, never less code.

Connectors anchor to invisible 24×24 marker shapes rather than to the screenshots, because Miro clips a connector endpoint to the item's border — the marker is what lets an arrow land on an exact line and column, computed from `silicon::line_geometry` plus the call site's AST span.

#### `linkedTo` is not reachable from the REST API

When a branch is too large to draw inline, `auto_deploy` emits a **link card** pointing at a separate frame. That card carries an `<a href="…moveToWidget…">` inside its text rather than Miro's native item link, and this is deliberate — the REST API rejects the field outright:

```
400  Field [linkedTo] is not supported
400  Field [data.linkedTo] is not supported
```

`linkedTo` exists only in the Web SDK, which runs inside a board. Investigated in August 2026 whether the browser call could be replayed from the terminal; it cannot. The editor sends every board mutation over `wss://miro.com/rtc-gateway/mux` as opaque binary frames (`permessage-deflate`), authenticated with a **session cookie** rather than the OAuth token. Nothing about a link appears in Fetch/XHR — only account metadata, JS chunks, Segment analytics, and a favicon fetch for the link target. Reproducing it would mean reversing an undocumented binary protocol and holding a browser session.

**Decision: keep the `<a href>`.** It navigates correctly; it just renders as a text link instead of showing Miro's arrow affordance. Link cards are rare by design — only a branch too big to draw inline produces one, typically one to three per diagram — so the manual alternative is a few seconds of "Link to" per diagram.

A Web SDK app that converted those anchors into native `linkedTo` in one click was built and then removed (commit `2cbf1cb`, reverted here). It worked, but it has to be hosted somewhere and installed by each user against their own Miro app, which is more ceremony than the problem warrants. Recovering it is a `git show` away if that trade ever changes — rebuilding it from scratch is not worth it.

## Conventions

- Enums that back CLI choices implement `BatEnumerator` (via `strum` `Display`/`EnumIter`), which provides `get_type_vec`, `from_index`, snake/sentence-case conversions and drives the `dialoguer` prompts in `batbelt/bat_dialoguer.rs`. Command enums additionally implement `BatCommandEnumerator` (`execute_command`, `check_metadata_is_initialized`, `check_correct_branch`), and that same enum shape is reflected into generated `package.json` scripts.
- Several modules are dormant behind `#[allow(dead_code, unused_imports)]` (`analytics`, `finding_commands`, `repository_commands`) and are commented out of `BatCommands` — leave them unless asked.
- Logging goes to stderr via `env_logger` (level from `-v` flags); `println!` is reserved for user-facing CLI output, colored with `colored`.
