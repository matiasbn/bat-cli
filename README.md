<p align="center">
  <img src="https://raw.githubusercontent.com/matiasbn/bat-cli/main/assets/logo.png" width="400" alt="BAT CLI logo">
</p>

# bat-cli — Blockchain Auditor Toolkit

A Rust CLI that performs full codebase analysis of blockchain projects by building AST-based metadata to extract function dependencies, access control patterns, and storage layouts. It also deploys annotated code screenshots to Miro boards for manual code review.

Supports **Anchor**, **Pinocchio**, **vanilla Rust** (Solana), and **Foundry** (Solidity/EVM) projects.

## Install

```bash
cargo install bat-cli
```

## What it does

### Initialize (`init`)

Sets up the audit workspace: detects the project framework (Anchor, Pinocchio, or Foundry), configures Miro integration (with API validation), and runs the initial sonar analysis.

### Static analysis (`sonar`)

Parses the entire codebase via AST and extracts metadata into a single `BatMetadata.json`:

**Solana (Anchor / Pinocchio / vanilla Rust):**
- Functions, structs, traits, enums
- Entry points and their context accounts
- Recursive function dependency graphs (caller → callee resolution across files, impl blocks, and trait impls)
- **Anchor**: account constraints and validations (`#[account(...)]`, `has_one`, `seeds`, `constraint`)
- **Pinocchio**: heuristic-based check detection from `TryFrom` impls (signer, writable, program-owned, mint, token accounts)

**EVM (Foundry / Solidity):**
- Contracts, interfaces, libraries, abstract contracts
- Functions with visibility, mutability, modifiers, and parameters
- Storage variables, events, and modifier definitions
- Inheritance resolution via C3 linearization
- Recursive function dependency graphs (caller → callee resolution across contracts and inherited functions)
- Import resolution with Foundry remappings, `lib/`, and `node_modules/` support
- Access control detection (onlyOwner, role-based, custom modifiers)
- Solidity parsing via [solar-parse](https://github.com/paradigmxyz/solar) — native Solidity lexer, no preprocessor workarounds

### Code overhaul workflow (`code-overhaul`)

Structured audit workflow per entry point:

- `code-overhaul start` — generates a template with the entry point metadata (access control, parameters, contract info, validations). Optionally deploys screenshots to Miro
- `code-overhaul finish` — marks an entry point as reviewed

### Miro board visualization (`miro`)

Deploys annotated code screenshots and dependency graphs to a Miro board for manual code analysis:

- `miro code-overhaul-frames` — creates frames for each entry point
- `miro code-overhaul-screenshots` — deploys entry point and dependency screenshots with caller→callee arrows
- `miro entrypoint-screenshots` — deploys entry point and context accounts to a selected frame
- `miro source-code-screenshots` — deploys arbitrary source code screenshots
- `miro function-dependencies` — deploys a function and its dependency tree
- Interactive BFS deployment of dependency screenshots with caller→callee arrows
- Screenshots use Dracula theme with syntax highlighting via [silicon](https://github.com/Aloxaf/silicon)
- Board URL is validated against the Miro API during setup

### Utilities (`tool`)

- `tool open-source-code` — open any function, struct, trait, or enum directly in your editor from metadata
- `tool open-code-overhaul-file` — open a started code-overhaul file and its entry point source
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
│   ├── to-review/            # Pending entry points
│   ├── started/              # In progress
│   └── finished/             # Reviewed
└── notes/
    └── <auditor>-notes/
        └── code-overhaul/    # Per-entry-point audit notes
```

## Quick start

```bash
# Initialize a new audit project
bat-cli init

# Start reviewing an entry point (runs sonar + deploys to Miro)
bat-cli code-overhaul start

# Finish reviewing an entry point
bat-cli code-overhaul finish

# Deploy code-overhaul frames to Miro
bat-cli miro code-overhaul-frames

# Deploy screenshots to Miro
bat-cli miro code-overhaul-screenshots
```

## License

MIT
