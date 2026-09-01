<p align="center">
  <img src="https://raw.githubusercontent.com/matiasbn/bat-cli/main/assets/logo.png" width="400" alt="BAT CLI logo">
</p>

# bat-cli — Blockchain Auditor Toolkit

A Rust CLI that draws the call graph of a smart contract onto a Miro board. It
parses the codebase, renders every function it reaches as a syntax-highlighted
screenshot, works out where each one goes, and uploads the whole diagram already
laid out — with every arrow landing on the exact line that makes the call.

Nothing is dragged into place by hand.

Supports **Foundry** (Solidity/EVM) today. The Solana parsers (Anchor,
Pinocchio, vanilla Rust) still ship and still scan, but have no deploy path yet.

## Install

```bash
cargo install bat-cli --locked
```

`--locked` matters: without it cargo re-resolves the dependency graph and can
pull crates needing a newer rustc than the published `Cargo.lock` pins.

## Getting started

```bash
bat-cli login     # once per machine: browser, press Accept
cd my-audit-repo
bat-cli init      # detect the framework, create the board, scan the source
bat-cli deploy    # pick a function, draw it
```

A project is two files at the root of the repository being audited:

| file | holds |
|---|---|
| `Bat.toml` | project type, program paths, the Miro board |
| `BatMetadata.json` | the parsed codebase, and what has been deployed |

Screenshots are rendered to the system temp directory and deleted once they are
on the board, so nothing else is left behind. bat-cli creates no branches and no
commits: what you do with version control is yours to decide.

## Commands

### `deploy`

Run it with no arguments to pick a function from a fuzzy-searchable list — entry
points first and marked, then every other function the project defines.

For one function it renders the call graph, measures each screenshot, and lays
the whole thing out:

- **Layers** come from the longest path to the root, so no arrow ever points
  backwards.
- **Order within a layer** follows the line that makes the call, so a callee
  invoked near the top of its caller is drawn above one invoked lower down.
- **Every arrow lands on its calling line** — past the end of it when the line
  makes one call, on the called token itself when it makes several, since then
  the column is the only thing telling them apart.
- **The entry point sits in the top-left corner**, so the frame reads as "the
  calls start here".

Then it uploads: one frame, one image per function already positioned, and one
connector per call site.

Useful flags, though none are needed:

| flag | |
|---|---|
| `--dry-run` | print the computed layout without contacting Miro |
| `--preview <path>` | compose the frame locally as a PNG |
| `--max-depth` / `--max-nodes` | bound a graph by hand; unset draws all of it |
| `--stroke-width` | connector thickness in dp |
| `--refresh-links` | after a callee gains its own frame, swap it for a link card in place — no re-render, no re-layout, your manual arrangement untouched |
| `--undeploy` | remove this entry point's frame from the board and registry entirely (a helper that shouldn't be its own frame) |

### `sonar`

`init` scans once. Run `sonar` after the source changes to rebuild
`BatMetadata.json`, which is what `deploy` reads. It extracts contracts,
interfaces and libraries; functions with their visibility, mutability and
modifiers; storage, events and modifier definitions; inheritance by C3
linearization; imports through Foundry remappings, `lib/` and `node_modules/`;
access control; and the call graph. Solidity is parsed with
[solar-parse](https://github.com/paradigmxyz/solar).

### `login` / `logout`

Miro authorization happens once per machine, not once per project.

```bash
bat-cli login --setup   # first time: register your Miro app credentials
bat-cli login           # opens the browser, you press Accept
bat-cli login --status  # who the token belongs to, and its scopes
```

It runs the OAuth 2.0 authorization code flow, listening on
`http://localhost:9871/callback`, and stores the token in your user config
directory. Every project picks it up automatically.

Only **one** Miro app is ever needed, no matter how many people use bat-cli.
Fill its credentials into `src/batbelt/miro/app_credentials.rs` and everyone
else skips app creation entirely: `bat-cli login` opens the consent page, they
pick their team, press Accept. The setup screen only appears when no shared app
is configured — Miro documents no PKCE and exposes no API to discover a user's
apps, so without one there is nothing to authorize against.

### `config`

Everything that belongs to you rather than to a project lives in
`~/.config/bat-cli/` (or `$XDG_CONFIG_HOME/bat-cli`, or `BAT_CLI_CONFIG_DIR`):

| file | holds |
|---|---|
| `config.toml` | `auditor_name`, `code_editor` |
| `miro.toml` | the OAuth credentials (`0600`) |

```bash
bat-cli config          # show the effective preferences and where they live
bat-cli config --edit   # re-answer them
```

### `update`

```bash
bat-cli update           # install the latest version from crates.io
bat-cli update --check   # only report whether a newer one exists
```

## How the diagram stays readable

Three problems show up as soon as a graph is more than a handful of functions,
and each is handled by measuring rather than guessing.

**A helper called from several places.** Drawing a copy per call site was tried:
`Vault.depositWithReferral` came to 77 screenshots for 27 distinct functions,
with a three-line arithmetic helper repeated fourteen times. So functions are
shared — except a **small private subtree** (a leaf, or a helper with only a few
non-shared descendants), where a copy costs little and buys a short local arrow
instead of a long crossing one. A duplicated copy keeps its own calls to any
shared function it uses, so the subgraph under it is always complete — never a
dead-end whose call line points at nothing.

**Overloaded functions.** When a contract defines the same name several times
(e.g. a public `quote(...)` forwarding to an internal `quote(curve, ...)`), each
call is matched to the overload whose argument count fits, and each overload is
its own node — so a wrapper calling its sibling is drawn as a real edge, not
mistaken for a self-call and dropped.

**Arrows crossing the code.** An edge between adjacent layers runs down the empty
corridor between them; one that skips a layer has to cross the column of
screenshots living there. Layering inserts a placeholder in each skipped layer
([Sugiyama's dummy nodes](https://en.wikipedia.org/wiki/Layered_graph_drawing)),
which claims a slot in the ordering and pushes the columns apart, so the corridor
is reserved rather than hoped for.

**Too many screenshots on one frame.** A whole call tree is drawn inline until it
grows past ~45 screenshots; only then is a branch replaced by a card linking to
its own frame. The choice is deliberately conservative so the board doesn't
fragment into tiny frames: between 45 and 65 screenshots only a branch big enough
to stand alone (≥ 8 screenshots, or ≥ 5 if it's reused ≥ 3× in the tree) is linked
out, and if none qualifies the slightly-bigger whole frame ships intact; past 65 a
smaller cut is forced. A cut that would leave a "pass-through" husk — one
screenshot pointing at another frame — is never taken; the heavy child is linked
out instead. One frame per function board-wide, reused by every diagram that needs
it (and only while it's actually still on the board), so the fan-in stays
answerable and a frame you delete by hand is never silently re-created.

## License

MIT
