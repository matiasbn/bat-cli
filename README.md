<p align="center">
  <img src="https://raw.githubusercontent.com/matiasbn/bat-cli/main/assets/logo.png" width="400" alt="BAT CLI logo">
</p>

# bat-cli — Blockchain Auditor Toolkit

A Rust CLI that automates the repetitive parts of Solana security audits: static analysis, dependency graphing, Miro board generation, and code-overhaul workflows. Supports both **Anchor** and **Pinocchio** frameworks.

## Install

```bash
cargo install bat-cli
```

## What it does

### Initialize (`init`)

Sets up the audit workspace: detects the program framework (Anchor or Pinocchio), configures Miro integration (with API validation), and runs the initial sonar analysis.

### Static analysis (`sonar`)

Scans every Rust file in the program and extracts metadata into a single `BatMetadata.json`:

- Functions, structs, traits, enums
- Entry points and their context accounts
- Recursive function dependency graphs (caller → callee resolution across files, impl blocks, and trait impls)
- **Anchor**: account constraints and validations (`#[account(...)]`, `has_one`, `seeds`, `constraint`)
- **Pinocchio**: heuristic-based check detection from `TryFrom` impls (signer, writable, program-owned, mint, token accounts)

### Code overhaul workflow (`code-overhaul`)

Structured audit workflow per instruction:

- `code-overhaul start` — generates a template with the entry point, context accounts, signers, and detected validations. For Pinocchio, signers and validations are inferred from the `TryFrom` implementation. Optionally deploys screenshots to Miro
- `code-overhaul finish` — marks an instruction as reviewed

### Miro board visualization (`miro`)

Deploys annotated code screenshots and dependency graphs to a Miro board:

- `miro code-overhaul-frames` — creates frames for each instruction
- `miro code-overhaul-screenshots` — deploys entry point, context accounts, validations, and signer screenshots
- `miro entrypoint-screenshots` — deploys entry point and context accounts to a selected frame
- `miro source-code-screenshots` — deploys arbitrary source code screenshots
- `miro function-dependencies` — deploys a function and its dependency tree
- Interactive BFS deployment of dependency screenshots with caller→callee arrows
- Screenshots use Dracula theme with syntax highlighting via [silicon](https://github.com/Aloxaf/silicon)
- Board URL is validated against the Miro API during setup

### Utilities (`tool`)

- `tool open-source-code` — open any function, struct, trait, or enum directly in your editor from metadata
- `tool open-code-overhaul-file` — open a started code-overhaul file and its instruction source
- `tool get-metadata-by-id` — search and open source code by metadata ID
- `tool count-code-overhaul` — count to-review, started, and finished code-overhaul files
- `tool list-entry-points-path` — list entry points with file paths
- `tool list-code-overhaul` — list code-overhaul files and their status
- `tool customize-package-json` — configure package.json log level scripts

## Project structure

After `bat-cli init`, the audit workspace looks like:

```
bat-audit/
├── Bat.toml                  # Project config
├── BatMetadata.json          # Sonar analysis cache
├── code-overhaul/
│   ├── to-review/            # Pending instructions
│   ├── started/              # In progress
│   └── finished/             # Reviewed
└── notes/
    └── <auditor>-notes/
        └── code-overhaul/    # Per-instruction audit notes
```

## Quick start

```bash
# Initialize a new audit project
bat-cli init

# Start reviewing an instruction (runs sonar + deploys to Miro)
bat-cli code-overhaul start

# Finish reviewing an instruction
bat-cli code-overhaul finish

# Deploy code-overhaul frames to Miro
bat-cli miro code-overhaul-frames

# Deploy screenshots to Miro
bat-cli miro code-overhaul-screenshots
```

## License

MIT
