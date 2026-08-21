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

Running the CLI against a real project: the binary must be executed from inside the audited project's audit workspace (or a directory whose child `bat-audit/` holds `Bat.toml` — `auto_detect_bat_audit_dir()` in `src/main.rs` will `cd` into it). Test projects are kept in a gitignored `test-workspace/` and driven via each project's `package.json` scripts (`cargo run --manifest-path ../../Cargo.toml -- init`); see `PACKAGE.md` for the template.

Release/format go through the CLI itself (dev-only subcommands, gated on `#[cfg(debug_assertions)]`):

```bash
cargo run package format    # clippy --fix + cargo fix + fmt + commit
cargo run package release   # bump version, git flow release, tag, cargo publish, cargo install
```

When cutting a release by hand (git-flow: branch off `develop`, bump `Cargo.toml`,
`git flow release finish`, push `main`/`develop`/tags), always finish with **both** of these:

```bash
cargo publish                              # run this in a background agent, it takes minutes
cargo install --path . --force --locked
```

Installing from the local path rather than waiting for crates.io means the new version
is usable immediately. `--locked` is required: without it `cargo install` re-resolves
dependencies and pulls transitive crates needing a newer rustc than `Cargo.lock` pins.


## Architecture

bat-cli is a single binary (`src/main.rs`) whose `BatCommands` clap enum dispatches to `src/commands/*`. Every non-`Init`/`Reload`/`Package` command first runs `validate_command()`, which enforces two invariants: the metadata cache is initialized, and the user is on the auditor branch (`{auditor_name}-{project_name}`, see `batbelt::git::get_auditor_branch_name`).

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

`BatMetadata` = `{ source_code, entry_points, function_dependencies, traits, context_accounts, miro }`. Every item carries a random 30-char `MetadataId`; cross-references (dependencies, entry points, Miro items) are stored by id, never by name. The `BatMetadataParser<U>` trait is the common interface (`name/path/metadata_id/start_line_index/end_line_index/metadata_sub_type`) implemented by each `*_source_code_metadata.rs`. `BatAuditorConfig::external_bat_metadata` lets one audit reference `BatMetadata.json` files from sibling projects.

### Paths & workspace layout

Never hardcode audit paths: `BatFile` and `BatFolder` (`src/batbelt/path.rs`) are enums that resolve every path from `Bat.toml`/`BatAuditor.toml`. The workspace is:

```
bat-audit/
├── Bat.toml            # project config (BatConfig: programs, project_type, miro board, auditors)
├── BatAuditor.toml     # per-auditor config (BatAuditorConfig: name, miro token, editor) — gitignored
├── BatMetadata.json    # sonar cache
├── code-overhaul/{to-review,started,finished}/
└── notes/<auditor>-notes/
```

Config is loaded with `figment` from those TOMLs; `BatConfig::get_config()` is called ad hoc from deep in the tree (including `metadata::derive_program_name_from_path`), so a valid `Bat.toml` in the cwd is a precondition for most code paths.

### Miro integration

`batbelt/miro/` wraps the Miro REST API (frames, images, shapes, connectors, sticky notes) with the OAuth token from `BatAuditor.toml`; `src/commands/miro_commands.rs` orchestrates deployments. Screenshots are rendered locally by `batbelt/silicon.rs` (silicon + syntect, Dracula theme, background `#282a36`). Frame geometry constants live in `batbelt/miro/frame.rs` (`MIRO_FRAME_WIDTH = 5600`, `MIRO_FRAME_HEIGHT = 2600`, `MIRO_BOARD_COLUMNS = 5`, `MIRO_INITIAL_X = 4800`). Dependency screenshots are deployed by an interactive BFS over the dependency graph (`VecDeque` + `HashSet<MetadataId>`), prompting per function and drawing caller→callee arrows as it goes.

### Git side effects

Most commands end by creating a commit through the `GitCommit` enum (`batbelt/git/git_commit.rs`) — e.g. `UpdateMetadataJson`, `StartCO`, `FinishCO`. Commit messages are generated, not free-form; add a variant instead of shelling out to `git commit`.

## Conventions

- Enums that back CLI choices implement `BatEnumerator` (via `strum` `Display`/`EnumIter`), which provides `get_type_vec`, `from_index`, snake/sentence-case conversions and drives the `dialoguer` prompts in `batbelt/bat_dialoguer.rs`. Command enums additionally implement `BatCommandEnumerator` (`execute_command`, `check_metadata_is_initialized`, `check_correct_branch`), and that same enum shape is reflected into generated `package.json` scripts.
- Several modules are dormant behind `#[allow(dead_code, unused_imports)]` (`analytics`, `finding_commands`, `repository_commands`) and are commented out of `BatCommands` — leave them unless asked.
- Logging goes to `Batlog.log` via `log4rs` (level from `-v` flags); `println!` is reserved for user-facing CLI output, colored with `colored`.
