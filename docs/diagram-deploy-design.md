# How bat-cli deploys call-graph diagrams to Miro

This document captures the DESIGN DECISIONS behind `deploy` — how a Solidity
function's call graph becomes a readable Miro diagram — so they aren't lost. It is
the "why", not a line-by-line of the code; the code lives in
`src/batbelt/evm/miro/auto_deploy.rs` and `src/batbelt/miro/layout.rs`.

---

## 1. The hard constraint that shapes everything

**Miro auto-routes connectors.** You can set the two endpoint anchors of a
connector, but NOT its waypoints — Miro draws its own orthogonal elbow between the
endpoints. So the layout engine can compute perfect corridors, but Miro will still
draw a shared node's arrow straight across whatever screenshots sit between the
caller and the callee.

Consequence: **crossings cannot be fixed by the layout alone.** The only way to
remove the arrow that crosses is to make the callee LOCAL to its caller (a copy or
a nearby card), so the arrow is short. Everything below follows from this.

A second hard rule from the auditor: **never crop, fold, or hide source code** —
every screenshot shows the complete function. So "make it fit" is never "show
less code"; it's "split into more frames" or "repeat a screenshot".

---

## 2. The pipeline (order matters)

`deploy_one` runs these in order. The ORDER is a decision, not an accident:

1. **Build the graph** (`build_graph`) — DFS from the entry point, one node per
   function, edges per call site. No duplication here.
2. **Render** each screenshot (`render_and_measure`) — silicon renders one PNG per
   DISTINCT function; see §6.
3. **Recycle / link** — a callee that already has a live frame becomes a link card
   (skipped in `--inline-all` and `--redeploy`).
4. **Framing** (the cut loop) — split the graph into readable sub-frames; §4.
5. **Localize** (`duplicate_crossing_shared`) — copy the small helpers that STILL
   cross, AFTER framing; §5. THIS ORDER IS THE KEY INSIGHT.
6. **Layout** (`layout_graph`, Sugiyama) — position everything; §3.
7. **Upload** — create the frame, images, connectors; record what was made.

---

## 3. Layout: Sugiyama (layered graph drawing)

`layout.rs` implements the Sugiyama framework (the "Japanese algorithm"):

- **Layering** by longest path (`assign_layers`) → left-to-right columns, no
  backward arrows.
- **Dummy/bend nodes** (`insert_bend_points`) reserve a corridor for every
  layer-skipping edge, so long edges route in the gaps between columns.
- **Crossing reduction** by the barycenter/median heuristic with sweeps
  (`order_layers` + `count_crossings`), keeping the fewest-crossings ordering.

`count_crossings` gives the EXACT number of crossings for a layout — this is the
ground truth we measure against (not a distance proxy). A pure tree lays out with
ZERO crossings; crossings ⟹ shared nodes (fan-in ≥ 2) reached from distant callers.

**Every layer is TOP-aligned, not centred.** A caller (a long entry point) sits at
the top-left; centring its callees would push them to the middle, so arrows from the
caller's top AND bottom call sites both converge inward and cross. Top-aligning keeps
the fan-out monotonic — the cascade flows down-and-right from the top — and keeps deep
chains from drifting far down (see `layout.rs`, the `y_cursor` init).

---

## 4. Framing: partition a big graph into readable frames

Constants (`auto_deploy.rs`): `FRAME_TARGET = 15`, `FRAME_MAX = 20`,
`FRAME_MIN = 6`, `MAX_CUTS_PER_FRAME = 10`, plus a depth penalty
(`DEPTH_FREE_LAYERS = 5`, `DEPTH_PENALTY = 0.15`).

**Decisions:**

- **Aim AT a size, don't just cap.** A frame is aimed at ~15 screenshots (readable
  at a normal zoom, ~30 connectors). Above `FRAME_MAX` (20, depth-adjusted via
  `effective_size`) it is split; a piece that is itself still too big becomes its
  own frame and is split again → a shallow HIERARCHY of readable frames, not one
  wall or a scatter of husks.
- **Cut for BALANCE, not "biggest".** `best_cut` scores each candidate on how close
  its subtree size lands to the per-piece budget, minus the cross-frame edges the
  cut severs, plus small bonuses for reuse and an already-existing target frame.
  Cutting the biggest branch first left lopsided halves; nearness-to-target fixes it.
- **`cut_node` lifts a SHARED node out whole.** In a densely-shared DAG, cutting one
  edge frees nothing (the subtree still hangs off another caller). `cut_node`
  replaces EVERY in-edge to a node with its own link card, so the whole subtree
  leaves the frame — the only way to partition a diamond.
- **Husk guard, both directions.** Neither the new piece nor the leftover frame may
  fall below `FRAME_MIN` (6). This forbids "pass-through" frames (one screenshot
  pointing at another frame).
- **Depth counts.** `effective_size` inflates the count for frames deeper than
  `DEPTH_FREE_LAYERS`, because horizontal px is the scarce resource; a deep-narrow
  frame is split sooner than a shallow-wide one of the same count.
- **`MAX_CUTS_PER_FRAME` was silently capped at 5 by a leftover `MAX_CUT_PASSES`.**
  Fixed to 10 — a 182-node graph could otherwise shed only 5 branches and ship a
  110-screenshot residual.

---

## 5. Localize small crossers — AFTER framing (the key insight)

`duplicate_crossing_shared` (constants `CROSS_LAYERS = 2`, `MAX_CLOSURE = 3`):

- **What it does:** lay the graph out, and for a SHARED node whose subtree is small
  (`MAX_CLOSURE`) and whose caller sits ≥ `CROSS_LAYERS` columns back (its arrow
  would skip a column and cross), give that far caller a LOCAL COPY of the node.
  Callers in the adjacent column keep sharing the one node (no needless repeat). The
  nearest caller always stays on the original so it is never orphaned.
- **Why AFTER framing, not before:** running this before framing was whack-a-mole —
  the shared-node count stayed constant because copies re-called the same deep
  helpers, which then gained fan-in. Framing first removes the deep "floor" (the
  big shared subtrees go to their own frames), so the leftover frame is small and
  copying its remaining small leaves is cheap and actually converges.
- **Copies inherit the render** (they run after §6), so no re-render.

**Decision on the metric:** distance (≥2 columns back) is only the heuristic for
WHICH node to copy; the CERTIFICATE that a crossing is gone is `count_crossings`
from the re-layout — "close" is not a guarantee.

Result on the entry point this was developed against: the main frame went from a
178-screenshot mesh to a readable ~37 (framing), with the residual small crossers
copied local.

---

## 6. Render dedup + scaling

Decisions in `render_and_measure` / `make_node`:

- **Render each DISTINCT function once per RUN**, not once per node. The PNG name is
  `fn_<file>_<start>_<end>.png` (no owner prefix), so every frame in a run (a whole
  `--redeploy` cluster) SHARES it; a function that appears in several frames renders
  once. Cleanup is deferred to the end of the run so the shared files survive.
- **One reference font + scale, not a re-render per depth.** Every screenshot renders
  at `REFERENCE_FONT` (32, the depth-0 size); a deeper node reuses that image shrunk
  via `GraphNode.scale` (Miro image `width` only — geometry keeps aspect ratio).
  Always scale DOWN, so text stays crisp. Line fractions are scale-invariant, so
  only `board_width`/`board_height` and the upload width multiply by `scale`.
- **Note:** render is NOT the deploy bottleneck — the API calls (image uploads +
  many connectors, no bulk-create) dominate. Dedup is a modest win.

---

## 7. Overloaded functions (resolved by arity)

A contract with several same-named functions (Solidity overloads, e.g. a public
`quote(...)` forwarding to an internal `quote(curve, ...)`) used
to map EVERY call to the first definition — so a wrapper calling its sibling looked
like a self-call and was dropped, and the whole implementation subtree vanished.

Fix: call sites carry their argument count (`CallSite.arg_count` from the AST);
`find_function` picks the overload whose `params.len()` matches; each overload is a
distinct node (`overload_node_key` appends the definition line ONLY when the name is
overloaded, so non-overloaded graphs are byte-identical). The DFS re-reads the exact
overload by line (`find_function_at`).

---

## 8. `--redeploy`: fresh cluster in a clean zone, old URLs handed back

Deleting is slow on the Miro API: there is NO bulk-delete, and deleting a frame does
NOT cascade to its children (verified empirically — the child stayed alive). So
programmatic teardown is one-by-one and slow.

Decision: `--redeploy` does NOT delete. It:

- Draws the WHOLE cluster (entry point + every dependency frame) FRESH into a clean
  zone (the cached region is forgotten so the allocator re-scans below everything),
  reusing nothing already on the board — not this entry point's own old frames, not
  another deploy's frames — only frames created earlier in THIS run (within-cluster
  sharing, gated by `cluster_root == root && id ∉ stale_ids`).
- Stamps each new frame with `AutoDeployedFrame.cluster_root = <entry point>`, so a
  later `--redeploy` finds the whole previous cluster.
- At the end, prints the previous cluster's still-live frame URLs for ONE-CLICK
  manual deletion in Miro's web UI (which DOES delete a frame with its contents),
  and drops those stale records.

Resumability (skip already-complete frames) was considered and deferred — it needs a
per-frame "done" checkpoint; the auditor was fine re-scanning.

---

## 9. What did NOT work (so we don't retry it)

- **Duplicating shared subtrees to a tree** — EXPLODES. A deep diamond hits 240+
  nodes and STILL has ~26 shared nodes; the mesh regenerates because copies re-call
  the same deep helpers. Bounded duplication just stops half-de-shared.
- **Distance-gated duplication BEFORE framing** — whack-a-mole (see §5).
- **Satellites (hoist small helpers outside the frame)** — only partial; the reuse is
  distributed at ALL subtree sizes, not concentrated in a few leaves, so removing 9
  leaves barely dented the mesh. Superseded by framing + localize.
- **Making a shared node's screenshot smaller** — does nothing for crossings; the
  arrow still spans the same distance. Only a LOCAL copy shortens the arrow.
- **Upload-dedup via the first upload's Miro image URL** — the URL is authenticated
  and valid only ~60s, so it's non-deterministic and could block a mid-deploy;
  deprioritized.
- **`--preview` that also deployed** — was a footgun; `--preview` is now local-only
  (composes one PNG, never touches the board) and is the fast way to iterate.

---

## 10. The AI-first / CLI split (design philosophy)

The tool is meant to be driven by an AI assistant, but the auditor's rule is: **the
CLI must produce a good diagram with ZERO AI intervention** (the default is
deterministic), because the AI may lose context or ignore docs. AI involvement is a
small, optional refinement, never required.

- **CLI decides deterministically:** layout, framing, localization, cluster
  placement, frame naming/reuse. Same input → same output.
- **AI, if it wants, overrides only by EXCEPTION, by name** (the intended surface is
  a couple of flags like `--inline <fn>` / `--externalize <fn>` reading a table the
  CLI prints) — never tuning thresholds or re-inventing the layout, which is what
  brings inconsistency.
- **Self-documenting output** beats separate docs the AI won't load: print the
  override hint in stdout.

The classification signal for "utility to externalize" vs "business logic to keep
inline" is computable by the CLI: fan-in, subtree size (leaf vs deep), `is_pure` /
`is_library` from the AST, and real `count_crossings` — NOT auditor labels.

---

## 10b. What can be a root, and which contract a name means

**Naming something is a decision already made.** The picker lists the project's own functions,
minus constructors and `lib/`, because nobody wants to scroll past dependency plumbing. But
`--entry-point` reaches anything: a constructor, `fallback`/`receive`, a contract under `lib/`.
The motivating case was `FLAMMProxy.constructor` — an empty body whose entire behaviour is the
`BeaconProxy` it inherits — which could not be drawn at all.

**A constructor is drawn with the base constructors it runs.** `constructor(...) BeaconProxy(beacon,
data) {}` parses the base invocation as a modifier, which resolved to nothing, so the diagram
stopped at the root. Base constructors are edges now: anchored on the header token when invoked
there, on the signature when implicit, descending through bases that have no constructor of their
own. Only what construction RUNS is drawn — not every inherited function.

**A `lib/` root implies `--include-external`.** Everything such a function calls is dependency code
too; without the flag the frame silently collapses to one screenshot, and nothing says why.

**A name resolves through imports, the way `solc` resolves it.** Contract names are not unique:
`lib/` vendors several copies of the same library, and lookups used to take the first match in
the project — sometimes a copy the audited code never imports. Every lookup now resolves the name
through the using file's import graph and `remappings.txt`, nearest first
(`EvmBatMetadata::contract_in_scope`, `evm/parser/import_graph.rs`). For `--entry-point`,
several matches narrow deterministically — own entry points, own functions, `lib/`; then the copy
the audited code reaches — and the deploy prints `from <file>` when the name is shared.

**The CLI decides what the code decides; it stops for what the code cannot** (§10). An ambiguity
that imports settle is never a question. Two in-scope contracts both defining `poke`, or a library
nothing imports, stop with each candidate listed as `path/To.sol:Contract.function` — the form that
selects it. That is different from `bat-cli resolve`: an interface's implementation is bound at
deploy time, which no amount of reading imports can reveal.

---

## 11. Board hygiene (Miro side)

- **Board picker lists only boards you OWN** (`?owner=<user id>`), so a big org
  returns a handful instead of hundreds (it was hanging on 243), and you can't edit
  someone else's board by accident.
- **Boards bat-cli creates are PRIVATE** — `policy.sharingPolicy` with `access`,
  `teamAccess`, `organizationAccess` all `private`; audit diagrams are sensitive and
  a Business/Enterprise org shares new boards team-wide by default.

---

## 12. Where the knobs live (quick reference)

All in `src/batbelt/evm/miro/auto_deploy.rs` unless noted:

| Constant | Value | Meaning |
|---|---|---|
| `FRAME_TARGET` | 15 | screenshots a frame is aimed at |
| `FRAME_MAX` | 20 | effective size above which a frame is split |
| `FRAME_MIN` | 6 | husk floor (piece AND residual) |
| `MAX_CUTS_PER_FRAME` | 10 | sets the initial cut budget; NOT a cap on passes (§13) |
| `DEPTH_FREE_LAYERS` / `DEPTH_PENALTY` | 5 / 0.15 | depth surcharge in `effective_size` |
| `CROSS_LAYERS` | 2 | caller columns-back that counts as a crossing |
| `MAX_CLOSURE` (localize) | `FRAME_MIN - 1` | biggest closure copied local (§14) |
| `MAX_COPIES_OF_ONE_CALLEE` | 3 | far callers that may each get a copy before the callee gets a frame (§14) |
| `LANE_PITCH` | 40 | x between two arrows' vertical lanes; narrows to fit (§13) |
| `ANCHOR_MARKER_SIZE` | 24 | invisible shape a connector endpoint anchors to |
| `REFERENCE_FONT` | 32 | the one font everything renders at; depth = scale |

Flags: `--dry-run`, `--preview <path>` (local PNG, no board), `--with-documentation`,
`--stroke-width`, `--allow-unresolved`, `--ignore-contract <name-or-path>` (§14),
`--inline-all` (one frame, measure size).

Removed in 0.26.11 (§15): `--redeploy`/`--fresh-frames` (every deploy is fresh now),
`--recycle`, `--refresh-links`, `--undeploy`, `--yes`, `--all`, `--max-depth`,
`--max-nodes`, `--include-external` (`lib/` is always drawn).

`--entry-point` forms: `function`, `Contract.function`, `path/To.sol:Contract.function` (§10b).

## 13. Readable arrows: call order, lanes, no stagger — ALL SHIPPED (0.26.11)

All three parts below shipped. What they replaced, and what they cost, is worth keeping because
each was a rule the code stated and then failed to enforce.

**Call order.** `count_crossings` keyed an edge by the integer slot of its caller, so two edges
leaving the SAME caller compared equal and could never count as crossing — the metric was blind to
the disorder a reader notices first, and the call-line signal (`from_line_fraction`) survived only
as a tie-break that any gain elsewhere overrode. It now keys by `slot + from_line_fraction`.
`sort_layer` also sent keyless nodes to the END of a layer, and in a downward sweep every leaf is
keyless, so a column came out as "callees that call something, then every leaf"; keyless nodes now
hold their slot. Measured on a real frame: the root's 18 callees in source order, inverted pairs
32 → 8, and those 8 are one callee genuinely called from two lines (one box cannot sit at two
heights).

**Lanes.** Miro routes a connector itself (§1), so every arrow leaving a caller turned on the same
x and the verticals stacked into what read as one thick line. A forward arrow is now drawn as three
straight legs between markers bat-cli places — out at the call line, down (or up) this arrow's OWN
lane in the gutter, in at the callee's signature line — so there is nothing left for Miro to route.
Lanes sit at `layer_right + margin + k · LANE_PITCH` and narrow automatically when a gutter cannot
hold them all. They are ordered by (start y, end y): of two arrows going the same way, the one
starting lower takes the outer lane, so each one's horizontal leg passes outside the other's
vertical leg. Two arrows whose start and end order disagree cross once, which is unavoidable with
one box per function. Cycles keep the single Miro-routed connector. Cost: roughly 1.6× calls in the
connector phase — bounded in practice by the client's 6 concurrent requests, not by the count.

A detail worth remembering: the shared stub (one arrow head per call line, into the edge marker) is
drawn ONLY when something still needs Miro to route. Drawing it as well as a lane put a second
horizontal on the same y, and the hook the reader saw at the caller's border was that duplicate
plus the elbow Miro added to join them.

**Stagger, gone.** The per-column x nudge existed only to separate elbows Miro chose; with lanes it
had no job, and it cost up to 300px of spread per column.


Three cosmetic defects survive in a wide fan-out, and all three force the reader to
rearrange the board by hand to follow the graph. Measured on a 33-node / 36-edge
frame (`--dry-run`), root fan-out of 18 edges:

- **Columns are not in call order.** 14 of 18 callees sat off their call-order slot,
  32 inverted `(call line, callee y)` pairs, plus the same defect in three deeper
  callers. Root cause is not the sweep count: `count_crossings` keys an edge by the
  integer *slot* of caller and callee, so two edges leaving the **same** caller have
  equal keys and can never count as crossing — the metric is blind to precisely this
  defect, and the call-order signal (`from_line_fraction`, used by the downward
  sweep) only ever survives as a tie-break. Compounding it, `sort_layer` sends
  keyless nodes to the END of the layer, which is what puts every leaf below every
  non-leaf.
- **Outgoing connectors collapse into one line.** Every group leaving a caller shares
  one vertical corridor x (the caller's right border, `+200` only when the call token
  itself reaches it), so with 18 edges the verticals fell inside 150px — 8.8px apart
  at 8px stroke.
- **Boxes' x forces crossings.** The per-column stagger (top box furthest right, ≤50px
  per rank / 300px total) exists only to un-stack Miro-chosen elbows; it spreads left
  edges by 300px and costs 700px of frame width, and the differing screenshot widths
  spread right edges by ~1500px, so arrows cross for no graph reason.

The design that fixes all three as one mechanism:

1. Make the crossing count honest — key each edge by `slot + from_line_fraction`, and
   let keyless nodes hold their slot instead of sinking. Then an out-of-call-order
   pair costs exactly 1, the same as any other crossing, so the layout trades it only
   when it genuinely saves crossings elsewhere.
2. **Lanes.** `layout.rs` fills the `routes` contract it already declares (nothing
   consumes it today) with real waypoints: each forward edge gets its own vertical
   lane in the gutter, `x_k = R_g + LANE_MARGIN + k · LANE_PITCH` (100 / 40 — five
   stroke widths apart, first turn clear of the red storage border), with the gutter
   widened to `max(gutter_x, 2·margin + (n-1)·pitch)`. Lanes are assigned by
   `(source y, target y)` so that non-inverted pairs provably never cross; inverted
   pairs cross once, which is topologically unavoidable. Upload draws a polyline of
   collinear marker-to-marker segments, so **Miro routes nothing** — the same
   invisible-marker trick already used for the call-token anchor, extended to the
   vertical leg. Layer-skipping edges reuse the gap already reserved by
   `insert_bend_points`, which today reserves a corridor and then drops it.
3. **Delete the stagger**, whose only job was separating elbows Miro no longer picks.

Simulated on that frame: inverted pairs 32 → 8 (the residue is one callee called from
two lines — one box cannot sit in two places), visual crossings 40 → 18, frame width
−160px net (the wider gutters cost less than the stagger did).

Why it is parked rather than shipped: it is a large change to the one pure, load-bearing
module for a purely aesthetic gain, and it carries real costs — ~1.6× API calls in the
connector phase (213 vs 135 here), a changed vertical order in deep layers so every
redeployed frame looks different from today's, an unverified minimum shape size for an
8px corner marker, and `refresh_links_surgical` keeping its one auto-routed connector
unless the registry also stores `callee_routes`. Nothing else moves: framing, localize,
pipeline order, the red/amber marks, `screenshot`, `undeploy`, `relink`, `resolve`.

A related idea, also parked: a `deploy_id` stamped inside the frame as content (not an
id), so a cut-and-pasted or re-deployed frame can be told apart from its twin. Children
already re-pair by content — every image carries `title` = node label, which is what
`rebuild_record` uses — so the gap is only the frame's own identity: today two frames
titled `auto: X` make `reanchor_frame` stop and ask for `--frame-url`, and the registry
holds one record per entry point. Keying the registry by a stamp instead would allow two
live frames of the same function, and would let a sweep repair inbound link cards, whose
`<a href=…moveToWidget=<frame_id>>` still points at the id the paste invalidated.

## 14. Density: what is NOT worth drawing

Three rules, all about the same thing — a diagram is for reading, and a box that teaches nothing
costs the reader more than it gives.

**An ignore list (`bat-cli ignore`).** A fixed-point maths library called from thirty places is
thirty boxes saying the same thing. On one real cluster `Math` was **205 of 747 drawn boxes, for 5
distinct functions**. The list takes a contract name or any part of a path, lives in
`BatMetadata.json` (preserved across a re-`sonar`, like `miro` and `resolutions`), and
`--ignore-contract` adds to it for one run; a deploy leaves out the union and prints what it
skipped. **It hides nothing about the audited code**: the calls stay in the callers' own
screenshots with their storage and boundary markings — only the callee's box is left out, and it
can always be deployed as an entry point of its own. Good candidates are utility maths and logging.
Bad ones are anything that writes storage or moves value (`SafeERC20`), which is what the diagram
exists for.

**Repetition and size are different costs.** Copying a callee next to its far caller is the only
thing that removes a crossing arrow (§1), and the decision to copy weighed only the callee's
closure. But the cost is paid once per caller: a helper the size of a getter was copied for every
caller that reached it, and one contract drew **203 boxes for 30 functions**. So two thresholds —
`MAX_CLOSURE` for how big one copy may be, `MAX_COPIES_OF_ONE_CALLEE` for how many callers may each
have one. Over either, the callee gets a frame of its own and a card beside each caller: one
drawing instead of seven, and the arrow is gone rather than shortened.

**A crossing is a reason to cut, and only the crossing call is cut.** Framing cuts for SPACE, which
left long arrows untouched however much room the frame had. `cut_crossing_shared` now replaces the
offending CALL — via `cut_edge`, not `cut_node` — so the caller sitting next to the callee keeps
reading it as a screenshot and only the caller that was flying over two columns gets a card. The
two bands meet at `FRAME_MIN`: under it a crossing callee is copied, at or over it it is carded, so
nothing falls between them and keeps crossing (the old `MAX_CLOSURE = 3` left exactly that gap).

**Colours are a graph colouring, not a ranking.** A colour exists to tell two arrows apart where a
reader compares them, which is three places: neighbouring lanes in one gutter, arrows leaving the
same screenshot, and arrows landing on boxes that neighbour each other in the next column. Every
rank-based rule collided somewhere — ranking by depth gave the same colour to the first callee of
one column and the first of the next, which after lanes are usually the two arrows side by side.
So the arrows are a conflict graph, walked in a fixed order (gutter, then lane) and given the first
colour no conflicting neighbour holds. Two arrows reaching the SAME function deliberately share a
colour, which is what makes a helper drawn in several places recognisable.

## 15. One deploy, always fresh (0.26.11)

`deploy` used to reuse the board: it recycled the entry point's existing frame (wiping its contents
and redrawing in place) and turned an already-framed callee into a link card pointing at whatever
frame existed. Both were dropped, for the same reason: **what it drew depended on which frames
happened to exist and where earlier deploys had left them**, so a deploy was not reproducible and
its output was scattered across the board — while the thing an auditor wants is a cluster they are
about to read, together, drawn by the rules in force today. Every deploy now draws the whole
cluster fresh in a clean region. Sharing happens only WITHIN a run (`ensure_target_frames`, keyed on
`cluster_root` + a non-stale id), so a helper two branches reach is still drawn once. The previous
cluster is not deleted — the API deletes one item at a time, slowly (§8) — its still-live frame
URLs are printed for one-click deletion in Miro.

That removed `--redeploy`/`--fresh-frames` (now the only behaviour) and `--recycle`. Removed with
them: `--refresh-links` and `--undeploy` (delete a frame in Miro yourself; the web UI takes its
contents with it), `--yes` (its prompt no longer existed — `assume_yes` was declared and never
read), `--all`, and `--max-depth`/`--max-nodes`, which truncated the graph and so hid code from an
audit. `--include-external` went too: if the code is in the repo it is part of what runs.

**Framing now cuts until the frame reads.** It gave up two ways in silence — a fixed ten passes,
and a cut budget computed once — so "nothing worth cutting AT THIS SIZE" was treated as "nothing
worth cutting" and the rest shipped as one wall: `FLAMMSwapLib.execute` went to the board as **250
screenshots and 641 connectors**. It now lowers the budget to the husk floor before giving up,
bounds itself by the node count (no run reaches it; every pass removes a screenshot), and SAYS so
when it genuinely cannot cut. Same function, same rules: 16 screenshots. Localization then checks
its own premise — copying a helper is cheap *because the frame is already small*, so on a frame
framing could not bring under `FRAME_MAX` it is skipped rather than adding 74 copies to a wall.

**A deploy's own temp dir is shared per project** (`$TMPDIR/bat-cli/<project>/`) and the rendered
PNG name carries no run id, so two deploys of the same project at once delete each other's
screenshots mid-upload. Known, not fixed: run one at a time.

## 16. Not implemented: a planning phase before drawing

`deploy_one` interleaves deciding and drawing: it builds the graph, frames it, creates the frame on
the board, and only THEN discovers its children — `ensure_target_frames` deploys each missing
target inside the parent's own deploy. Three consequences, all felt:

- **The size of a run is unknowable until it ends.** Nobody can say how many frames a cluster will
  have, so there is no total, no progress over frames, and `--dry-run` cannot tell you either: it
  expands the root frame and returns.
- **Frames are drawn strictly one at a time.** Uploads inside a frame already run concurrently
  (bounded by the client's 6 permits and the credit budget), but the network sits idle while the
  next frame renders and lays out. A 30-frame cluster pays that gap 30 times.
- **A frame cannot card a sibling that does not exist yet.** Reuse within a run is discovered as it
  goes, so a branch framed later is redrawn by a branch framed earlier. The current fix reads the
  registry for frames this run already wrote (§15) — real, but it only ever knows the past.

The shape of the fix: split `deploy_one` into `plan_frame` (graph → framing → localize → layout →
measured size; no client, all local and already `rayon`-parallel in the render) and `draw_frame`
(allocate, create, upload). Then `plan_cluster` recurses over the plans, deduplicating targets by
title, so the whole cluster is known before the first API call: the total is printable, the
allocator can place every frame in one pass, the unresolved-interface check can stop the run before
anything is drawn, and the frames can be created and filled concurrently.

Two things to get right when it is done: the rendered PNGs of the whole cluster now exist at once
(cleanup must move to the end of the run), and the shared temp dir means a per-run subdirectory is
needed anyway (§15).

## 17. A deployment is the unit, not a frame title

`deploy --entry-point X` produces ONE deployment: the frame for `X` plus a frame for every branch
cut out of it. The registry used to hold **one record per function name, board-wide**, which was
true while a callee already on the board was linked rather than redrawn (§15). Once every deploy
became fresh, a helper two entry points reach is drawn once per deployment — several frames on the
board carry the title `auto: FLAMMFlowLib.requireFlat`, different ids, by construction — and the
name-keyed registry made the second deployment's record displace the first's. Four sequential
deploys left three clusters drawn on the board and unreachable from the CLI.

So the key is **(deployment, frame)**, the deployment being `cluster_root`, which the record already
carried. Consequences, all deliberate:

- Re-deploying an entry point replaces **that deployment's** records and nobody else's. It asks
  first, because it also leaves the previous frames on the board; `--yes` answers it.
- Old frames are not deleted and their URLs are no longer printed: the API deletes one item at a
  time (§8), the frames are visible, and checking they were still there cost one call per frame.
- **Addressing is a deployment plus a name inside it**, never a title: `--frame` became
  `--deployment <entry point>` and `--dependency <function>`, with no `--dependency` meaning the
  deployment's own frame. Inside one deployment a name is unique, which is what makes it an address.
  A dependency drawn INSIDE the root frame has no frame of its own, and the error says so by listing
  what the deployment did draw.
- **Overloads carry their signature** — `MMRouterLib.read(uint256,address)` — since a name is not
  enough even inside a deployment. The discriminator used to be `@<line>`, which is unique and tells
  a reader nothing; a signature is what somebody reading the source would use. It also keeps the
  node id and the frame title differing only by `::` versus `.`, which is what lets `relink`
  re-pair a pasted frame by title.

## 18. Types are drawn too, and they are trees

A struct with no struct fields is one screenshot on the frame that needed it. A struct whose fields
are structs is a different object: `FLAMMSwapLib.Plan` holds a `SwapContext`, which holds a
`PoolContext`. Drawing only the outer one answers half the question; drawing all of them inline
buries the function the frame is about. So it becomes its own frame, laid out by the same rules a
call graph is, with an arrow from each field to the type it names, and the asking frame gets a
**purple card** — the shape a branch cut out to its own frame already leaves behind, in a colour
that says "type" rather than "call". The type frame joins the asking deployment, so `--dependency`
reaches it afterwards (§17).

- **Field types are read from the source**, because the scan does not record them: the first
  identifier on the line, with `T[]` and `mapping(K => V)` unwrapped to what they hold, resolved
  against the indexed structs and enums preferring one declared in the same file. Value types are
  skipped by name, which also stops a contract called `Address` from being mistaken for one.
- **Placement is beside the host frame, not at the allocator's cursor**, because the two are read
  together. Candidate rectangles are tested against every frame in the registry, spiralling out from
  the host. This is forced, not preferred: **Miro refuses to create a frame that overlaps another**
  and answers `500 Internal error` — found the hard way, and worth remembering for anything else
  that creates frames.
- **The arrows use `line_anchor`, shared with `deploy`.** They are the same problem — a connector
  endpoint is clipped to the item's border, so landing on a line needs an invisible marker at the
  point plus one on the border level with it, and a full-width line pushes the convergence outside
  or the stub has no length and no head is drawn. This was a second copy of that arithmetic for
  about an hour, and it had already drifted (the gap after the text was one character here,
  `ANCHOR_GAP_CHARS` there), which is why it is one function now.

## 19. Two rules about the board that are not about layout

**A frame says what it does, in its own fill.** Very pale red (`#fff0ef`) when something drawn in it
writes storage or reaches a write, pale amber (`#fff7ec`) when something in it only calls out past
the audited code, red winning when both — the precedence the per-node markings already have. The
per-node border answers that question once you are inside a frame; from the distance where a cluster
of thirty frames fits on screen, which is where the reader decides what to open, it is two pixels
wide. It is the frame's own fill because Miro's REST API sets no border on a frame, and a rectangle
at the frame's bounds would be one more item to create, record and clean up. The tints were chosen
by eye on a board, from a row of candidates, at that zoom: Miro's own "light red" (`#ffc6c6`) is
already too strong to read code through.

**Forgetting a record needs a definite answer.** `item_exists` returned `is_ok()`, so a rate limit,
a 500, a dropped connection or an expired token all came back as "not on the board" — and the caller
acts on that by DELETING the registry entry, on the reasoning that a record outlives a frame deleted
by hand. One unlucky request therefore dropped a whole deployment from the registry while its frames
sat untouched on the board. `item_status` now answers `Some(true)`, `Some(false)` only on a 404, or
`None` when the board could not be asked, and the record is kept in the last case. The general rule,
worth applying to anything else that prunes: **an error is not evidence of absence.**

**And one layout rule that came out of reading real frames:** the entry point sits at the TOP-LEFT
of its frame. `layout_tree` centred each parent on its children — the textbook tidy tree — so on a
tree-shaped graph the function you start from floated halfway down with empty space above it, while
everything done to a frame afterwards (reading it, dropping a declaration screenshot underneath)
wants that box near the corner and the room below. The layered path always top-aligned; the tree
path now agrees.

## 20. The cluster is an outline, not a shelf

Frames used to be shelf-packed: `ShelfAllocator::place` took the next slot on the current row and
wrapped at 60k, in whatever order the recursion happened to draw them. For one frame that is fine.
For a thirty-frame cluster it is a wall — the auditor opens a card, lands somewhere, and has no way
to tell what sent them there or what comes next except by reading titles.

Two things fixed it, and they only work together:

- **Deployment order is deterministic.** `card_reading_order` walks the graph depth-first from the
  root, taking each node's calls sorted by `(line_in_slice, column)` — source order, which is the
  order the auditor reads the function in. A card is listed the first time it is met; a screenshot
  is descended into. Unreachable cards are appended at the end rather than dropped.
- **Placement follows that order down the board.** `place_in_outline(w, h, depth, has_children)`
  starts a row at `depth * indent` (2 000 px) and fills it left to right. A frame whose cards lead
  somewhere closes its row, because its subtree is placed underneath and a neighbour on the same row
  would read as part of it. Leaf siblings share a row, which is what keeps "one function, six small
  helpers" from becoming a column of air.

The invariant that makes it navigable: **a frame's parent is the nearest frame above it with a
smaller indent.** Nothing is stored to make that true — it falls out of placing frames in the order
they are deployed, which is why the two halves are one change. Rows stay exclusive horizontal
bands, so frames still cannot overlap by construction (§19: Miro refuses overlapping frames with a
500).

`depth` is threaded as a parameter: `deploy_one(..., depth)` with 0 at the root, and
`ensure_target_frames(..., depth + 1)` for everything its cards lead to. `has_children` is
`!card_order.is_empty()`, which is why the card order is computed BEFORE the frame takes its slot
rather than where it is used. A frame whose cards all resolve to frames already drawn in this run
still closes its row — it reserves a band for a subtree it does not get. That costs vertical space,
never correctness, and knowing better requires the planning phase of §16.

**This is also what makes §16 worth doing.** The outline puts the reading order into the
coordinates instead of into the order of arrival, so once the cluster is planned before it is
drawn, frames can be created concurrently without losing it. Today they cannot: order is carried by
`for (target, _) in wanted { deploy_one(...).await }`, and parallelising that loop would need a
mutex on the allocator cursor, a lock on the read-modify-write of `BatMetadata.json`, an in-flight
map so two branches do not draw the same helper twice, and the per-run temp subdirectory §15
already wants.

## 21. How to do the §16 split, concretely

§16 says what the planning phase is for. This section is the recipe, written down so it does not
have to be re-derived. Budget: about a day, most of it in `deploy_one`.

**Half of it already exists.** `--dry-run` recurses over the whole cluster (the `options.dry_run`
branch of `deploy_one`, which walks `card_order` with `cluster.dry_seen` as its memory) and does
every local step: parse, render, frame, localize, layout, measure, allocate. Measured on
`FLAMM.swap`: **34 frames in 74.5 s, zero API calls**. What is missing is that it prints the result
and throws it away.

### Steps

1. **`struct FramePlan`** — everything `draw` needs and `plan` already computed: `title`,
   `contract`, `function`, `file`, `cluster_depth`, `frame_x/y/width/height`, `fill`, `nodes`,
   `edges`, `anchors`, `layout`, the rendered PNG paths (keyed as `render_and_measure` keys them),
   `card_order`, and the amber/red line sets.
2. **`plan_one` / `plan_cluster`** — lift the local half of `deploy_one` (everything up to and
   including `place_in_outline`) into a function returning `FramePlan`, and make `plan_cluster`
   the recursion, keeping `dry_seen` as the dedup. `--dry-run` becomes "plan, then print", which
   is also how the plan half gets its regression test.
3. **`draw_cluster(plans)`** — a flat loop over the plans **in plan order**. It recurses over
   nothing: every position is already decided, so the allocator is not touched during drawing.
4. **Two passes over the board, not one.** A parent needs its children's frame URLs for its link
   cards, so pass 1 creates all 34 frames (empty, from the plan's geometry) and collects
   `title → frame_id`; pass 2 fills each frame. Pass 2 is where concurrency lands: the frames are
   independent once their ids exist.
5. **Temp dir per run** (§15). The PNGs of the whole cluster now exist at once and `cleanup` moves
   to the end of the run, so the shared `$TMPDIR/bat-cli/<project>/` becomes a collision.

### The traps, in the order they will bite

- **Two walks that can disagree.** Do NOT let `draw` recurse "and also" read the plan: the plan's
  dedup (`dry_seen`) and the draw's dedup (the registry's `created_here`) WILL diverge on a helper
  reached twice, and the symptom is two frames placed at one position — which Miro rejects with a
  500 (§19). One walk, in `plan`. The draw consumes a list.
  **This is measured, not predicted.** On `FLAMM.swap` the dry run plans **34** frames and the real
  deploy draws **30**: the real one also reads `deployed_titles` (`auto_deploy.rs:930`, the frames
  this run has already drawn for this cluster), which lets `best_cut` card a callee at any size, so
  four branches the plan expected to draw became cards. Any split that keeps both walks starts four
  frames out of step.
- **The registry is written during the draw, and read during it too.** `created_here` exists
  because the draw discovers reuse as it goes. Once the plan decides reuse, that check is dead
  weight and a second source of truth — delete it rather than leave it agreeing by luck.
- **Back cards are per origin, not per frame** (`AutoDeployedFrame::back_cards`): the plan has to
  record every origin that cards a target, not just the first, or a frame reached from three
  callers comes back with one way out.
- **`--yes`/replacement semantics** run before the first frame: the old cluster's ids are collected
  into `ClusterCtx::stale_ids` up front, so that part is unaffected — do not move it.

### Verification

`cargo test` does not catch any of this; it shows up on the board. Verify in this order: a
`--dry-run` diff of the plan against today's output (same frames, same order, same positions), then
one real deploy of `FLAMM.swap` (34 frames) checking no overlap, no duplicate title within the
cluster, every card resolving, and every non-root frame carrying its way back.

### What the split is actually worth (measured 2026-10-05)

`FLAMM.swap`, 30 frames: **865.8 s wall, 232 s of CPU, and 74.5 s for the whole local half** (the
recursive dry run does the same parse/render/frame/layout work with zero API calls). So **~91 % of a
deploy is waiting on Miro**, one frame at a time, with no rate limiting at all (zero 429s, zero
retries, at 24 permits).

That settles which half to optimise: nothing local moves the needle, and the only change that does
is drawing frames concurrently — which needs the positions decided up front, which is the split.
