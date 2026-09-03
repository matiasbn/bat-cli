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

## Set up Miro access

**Do this before anything else: `bat-cli login` cannot run until an app exists.**
OAuth needs an app to authorize against, and Miro has no API to create one, so the
first step on a new machine is a one-time, roughly one-minute registration.

```bash
bat-cli login --setup   # create the Miro app (once per machine)
bat-cli login           # opens the browser, press Accept
bat-cli login --status  # who the token belongs to, and its scopes
```

`--setup` opens the Miro apps page and prints the exact values to use. In short:

1. On the page it opens (`https://miro.com/app/settings/user-profile/apps`), click
   **+ Create new app**. If you have no Developer team yet, Miro asks you to create one
   first (tick the terms, "Create team") — the app is assigned to it automatically.
2. Leave **"Expire user authorization token" unchecked** — a CLI wants a token that does
   not expire.
3. Scopes: check **`boards:read`** and **`boards:write`**.
4. Redirect URI for OAuth 2.0: paste exactly **`http://localhost:9871/callback`**.
5. Copy the app's **Client ID** and **Client secret** and paste them back into `--setup`.

The credentials are stored in your user config and reused by every project, so that is
the last copy-paste. One app per user authorizes any team's boards — a custom OAuth app
installs by simply being authorized. On an organization that restricts third-party apps,
an admin may have to approve it once.

`bat-cli login` then runs the OAuth 2.0 authorization code flow, listening on
`http://localhost:9871/callback`, and stores the token in your user config directory.
**Authorization is per machine, not per project:** every project picks the token up
automatically, and the board picker lists only boards you own.

**Sharing one app across a team (optional).** Instead of each person registering an app,
a maintainer can register **one** and distribute its `client_id`/`client_secret` — via
`BAT_MIRO_CLIENT_ID` / `BAT_MIRO_CLIENT_SECRET`, or a private build that injects them.
**Never commit the secret.** Teammates then skip `--setup` entirely: `bat-cli login`
opens the consent page, they pick their team, press Accept. Resolution order is env vars
→ `--setup` → compile-time baked (`src/batbelt/miro/app_credentials.rs`, empty by default
so no secret lives in the repo).

## Getting started

```bash
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
| `--with-documentation` | start each screenshot at the function's NatSpec, so the documented intent rides along with the code |
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

Authorization is per machine, not per project — see [Set up Miro access](#set-up-miro-access)
for the one-time app registration that has to happen first.

```bash
bat-cli login --setup   # register the Miro app (first time on this machine)
bat-cli login           # opens the browser, you press Accept
bat-cli login --status  # who the token belongs to, and its scopes
bat-cli logout          # revoke the stored token and forget it
```

### `screenshot`

A function's screenshot names things it does not explain — the state variable it compares
against, the struct it takes as a parameter. `screenshot` puts that declaration on the frame
you are reading:

```bash
bat-cli screenshot                                            # list the deployed frames
bat-cli screenshot deviationThresholdWad --frame Vault.deposit
bat-cli screenshot PriceFeed.FeedType --frame Vault.deposit --with-documentation
bat-cli screenshot --file src/PriceFeed.sol --lines 36-44 --frame Vault.deposit
```

The symbol is `Name` or `Contract.Name`, resolved from the scan — structs and enums (including
those declared inside a contract) and state variables, `constant` and `immutable` included. A
name declared in several contracts stops and lists the candidates rather than guessing. When a
name is not in the index, `--file` and `--lines` draw the range directly, so the command never
blocks on a gap.

The image lands in free space below the frame's content, rendered exactly as `deploy` renders a
function, and you drag it where you want it. There is deliberately no rule about what deserves
to be drawn: that judgement is yours. It is a manual enrichment of one diagram, so a redeploy
does not bring it back, and `--undeploy` cleans it up with the frame.

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

Four problems show up as soon as a graph is more than a handful of functions,
and each is handled by measuring rather than guessing.

**A helper called from several places.** Drawing a copy per call site was tried
and abandoned: on a real entry point it roughly tripled the screenshot count, with
a three-line arithmetic helper repeated a dozen times over. So functions are shared
by default — except a **small private subtree** (a leaf, or a helper with only a few
non-shared descendants), where a copy costs little and buys a short local arrow
instead of a long crossing one. A duplicated copy keeps its own calls to any shared
function it uses, so the subgraph under it is always complete — never a dead-end
whose call line points at nothing.

**State changes that happen elsewhere.** A function is drawn with a solid red border when it
assigns to storage, and each mutating line gets a red band. But a chain of pass-throughs assigns
nothing on the way down — `mint` calls `_mint` calls `_update`, and only the last one writes —
so the state change used to be invisible unless the final function happened to be on the frame.
The change is now marked once, at the deepest point the frame reaches: on the assignment when
the function holding it is drawn, otherwise on the call that leaves the frame towards it — so one
red mark is one state change, and they can be counted. Reachability is computed across the whole call graph, `lib/`
included, so the mark survives a chain cut short by framing or depth limits.

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

**Too many screenshots on one frame.** A graph small enough to read ships whole. A
bigger one is *partitioned* rather than merely capped: branches are cut out to their
own frames until each piece lands near a readable target of about fifteen screenshots,
and a piece still too big becomes a frame that is split again the same way — so a large
entry point becomes a shallow hierarchy of frames you can actually read, instead of one
wall or a scatter of fragments.

Which branch gets cut is chosen for **balance** — the piece whose size lands nearest the
target while severing the fewest arrows that would have to cross frames — not simply the
biggest one. A helper that everything calls can be lifted out whole, with each caller
keeping a card that links to it; in a densely shared graph that is the only way to
partition anything, since cutting a single edge frees nothing while another caller still
holds the subtree up. Two floors keep the result honest: no piece and no remainder may
fall below six screenshots, so a frame never degenerates into one screenshot pointing at
another frame, and depth counts against a frame's budget because horizontal space runs
out before vertical space does.

One frame per function board-wide, reused by every diagram that needs it — and only
while it is actually still on the board, so a frame you delete by hand is never silently
re-created.

## License

MIT
