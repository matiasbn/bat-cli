<p align="center">
  <img src="https://raw.githubusercontent.com/matiasbn/bat-cli/main/assets/logo.png" width="200" alt="BAT CLI logo">
</p>

# BAT CLI — Blockchain Audit Toolkit

A Rust CLI that automates the repetitive parts of Solana/Anchor security audits: static analysis, dependency graphing, Miro board generation, and findings management.

## Install

```bash
cargo install bat-cli
```

## What it does

### Static analysis (`sonar`)

Scans every Rust file in the program and extracts metadata into a single `BatMetadata.json`:

- Functions, structs, traits, enums
- Entry points and their context accounts
- Recursive function dependency graphs (caller → callee resolution across files, impl blocks, and trait impls)
- Account constraints and validations (`#[account(...)]`, `has_one`, `seeds`, `constraint`)

### Code overhaul workflow (`co`)

Structured audit workflow per instruction:

- `co start` — generates a template with the entry point, context accounts, signers, and detected validations
- `co finish` — marks an instruction as reviewed
- `co summary` — generates an audit summary from all finished reviews

### Miro board visualization (`miro`)

Deploys annotated code screenshots and dependency graphs to a Miro board:

- Entry point, context accounts, and validations screenshots
- Interactive BFS deployment of dependency screenshots with caller→callee arrows
- Signer diagrams with sticky notes and connectors
- Screenshots use Dracula theme with syntax highlighting via [silicon](https://github.com/Aloxaf/silicon)

### Findings management (`finding`)

- `finding create` — scaffolds a new finding from template
- `finding finish` — finalizes a finding
- `finding accept-all` / `finding reject` — triage findings

### Utilities (`tool`)

- Open any function, struct, trait, or enum directly in your editor from metadata
- Count and list code-overhaul progress (to-review / started / finished)
- List entry points with file paths

### Repository management (`repo`)

- Branch sync, remote fetch, local cleanup
- Structured commits for code-overhaul files, findings, and notes

## Project structure

After `bat-cli new`, the audit workspace looks like:

```
bat-audit/
├── Bat.toml                  # Project config
├── BatMetadata.json          # Sonar analysis cache
├── code-overhaul/
│   ├── to-review/            # Pending instructions
│   ├── started/              # In progress
│   └── finished/             # Reviewed
├── findings/
│   ├── to-review/
│   ├── accepted/
│   └── rejected/
└── notes/
    ├── open_questions.md
    ├── finding_candidate.md
    └── threat_modeling.md
```

## Quick start

```bash
# Initialize a new audit project
bat-cli new

# Run static analysis
bat-cli sonar

# Start reviewing an instruction
bat-cli co start

# Deploy screenshots to Miro
bat-cli miro code-overhaul-screenshots

# Create a finding
bat-cli finding create
```

## License

MIT
