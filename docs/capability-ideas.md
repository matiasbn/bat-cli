# What to build next — capability ideas (2026-10-06)

A proposal, not a decision. Written after a review of the codebase against the question:
*what would help an auditor review code and understand a system's architecture, that bat-cli
is unusually well placed to build?* The two seeds were relationships between contracts, and
storage analysis.

Nothing here has been started.

## Three things worth building, in order

### 1. Storage map — "who writes what, and under which guard"

One frame per contract, or per namespaced storage struct (ERC-7201's `$`): the state-variable
block as a screenshot, and beside it one card per function that writes it, with the arrow
landing on the exact declaration line and a badge for the guard — a modifier, a `msg.sender`
check, or nothing. Readers on the other side.

The question it answers in one look: *every writer of `totalDebt` that is unguarded.* Today
that is `jq` plus grep.

Why this tool and not Slither: `storage_write_sites` already carries file lines, the `$.field`
pointer path is already tracked, writes reached through a runtime-bound interface are already
followed via `resolutions`, and `line_anchor` already lands an arrow on an exact line and
column. Slither's `vars-and-auth` gives the table, not the lines, not the interface hops, and
nothing for namespaced storage.

Reuses: `storage_write_sites`, `EvmModifierDef.storage_writes`, `local_types` (to resolve the
`$` root to its struct type), `file_items`, `screenshot::render`, `line_anchor`,
`create_connector`, `create_link_card`.

Needs first: a reads collector (the same walk in `analyze_body`, collecting the roots of
rvalues), keying pointer writes by `StructName.field` instead of `$`, a callers index, and
real access-control detection.

About 4–5 days.

### 2. Contract architecture map

One board-level frame: a node per in-scope contract, edges typed and coloured — calls
(aggregated from `function_dependencies`, weight = distinct call sites), inheritance
(`base_contracts`), interface bindings (`interfaces` + `resolutions`, dashed when unresolved),
`using X for`, `new X`, and "wired at" (`UnresolvedCall.assigned_in`). Each node links to that
contract's already-deployed frames, so the map becomes the board's index.

The edge over Slither's `inheritance-graph`/`call-graph`: `contract_in_scope`. In a repo where
`lib/` vendors three copies of `BeaconProxy`, a graph keyed by name alone is wrong, and ours is
not.

Reuses: `ContractMetadata`, `InterfaceMetadata`, `resolutions`, `using_libraries`,
`function_dependencies`, `import_graph.rs`, `layout_graph`, `create_struct_card`,
`create_connector`, `place_in_outline`.

Needs first: a contract-level node render, aggregation from function edges to contract edges,
and a `--lens calls|inheritance|bindings` to keep the density readable (§14).

About 4–6 days.

### 3. Entry-point effects summary

`bat-cli effects Vault.deposit` prints — and optionally cards onto the root frame — the
transitive effect set: storage written per contract, token transfers and `call{value}`,
external boundaries left, events emitted, and at what depth each happens, in call order.

This is the **same reachability walk that already paints the red and amber marks**, printed
instead of painted. That is why it is two to three days and the best value per day on this
list. It is also the input a use-case or threat-model write-up wants.

Reuses: the marking reachability in `auto_deploy.rs`, `card_reading_order`,
`unknown_external_calls`, `assembly_state_calls`.

Needs first: `emit` collection, and factoring the reachability out of `plan_one` into a pure
function.

## Groundwork the three share

- **An access-control detector that reads bodies**, not modifier names. `detect_access_control`
  is a name heuristic: `RequireMsgSender` is a variant that exists and is never produced, and
  `onlyRole` always yields `DEFAULT_ADMIN_ROLE` whatever the argument. 1–2 days. On its own it
  duplicates Slither; as the badge on #1 and the colour on #2 it is what makes them readable.
- **A callers index.** `function_dependencies` is forward-only, so "who calls this" cannot be
  asked. It also unlocks `bat-cli callers <fn>`, `who-writes <var>`, `reaches <fn> <var>` —
  about a day once the index exists, and it multiplies what the assistant driving the tool can
  do, which the guide currently answers with `jq` recipes.

## Traps — deliberately not building

- **Computing storage slots.** Packing, dynamic types, inherited layout, ERC-7201 base-slot
  hashing: over a week and wrong at the edges. `forge inspect <C> storage-layout --json` is in
  every Foundry repo already — consume it and draw it.
- **Vulnerability detectors, taint or data-flow analysis.** Slither's ground. This tool's edge
  is exact-line visualisation and runtime-bound resolution.
- **A standalone inheritance viewer.** Keep it as an edge type in #2.
- **Every function of every contract on the architecture map.** §14 measured what density does.
  Contracts are the nodes; functions stay in their deployments.
- **A role/permission graph.** Without deploy-time wiring it cannot say who holds a role, so it
  is a badge, not a graph.

## Gaps, stated plainly

- **Verified:** `EntryPointMetadata.storage_reads`, `external_calls`, `events_emitted` and
  `dependencies` are hard-coded `vec![]` (`bat_metadata.rs:931`). The schema promises what the
  scan never fills.
- Struct field types are parsed into `EvmStruct.fields` but not persisted on
  `ContractMetadata`; `struct_frame.rs` re-reads them from source. Persist them before #1.
- `MetadataId`s are random per scan, so **nothing can be compared between two scans**. Any
  fix-review diff needs a stable key first (`file:Contract.function(sig)`).
- Deploy-time wiring — who owns whom, who holds which role, which proxy points where — is not
  knowable from `src/` alone. It lives in `script/` and `broadcast/`, which are not scanned.
  That is a separate decision, not a detail.
