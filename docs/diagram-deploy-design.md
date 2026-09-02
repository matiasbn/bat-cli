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
| `MAX_CUTS_PER_FRAME` | 10 | max sub-frames carved from one frame |
| `DEPTH_FREE_LAYERS` / `DEPTH_PENALTY` | 5 / 0.15 | depth surcharge in `effective_size` |
| `CROSS_LAYERS` | 2 | caller columns-back that counts as a crossing |
| `MAX_CLOSURE` (localize) | 3 | only SMALL helpers are copied local |
| `REFERENCE_FONT` | 32 | the one font everything renders at; depth = scale |

Flags: `--inline-all` (one frame, measure size), `--preview <path>` (local PNG, no
board), `--redeploy` (fresh cluster + report old URLs), `--refresh-links` (surgical
swap of newly-framed callees), `--undeploy` (remove a frame + its items outright).
