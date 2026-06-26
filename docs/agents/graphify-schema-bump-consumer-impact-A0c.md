# Task A0c — Consumer-impact check for the `graph.json` schema bump

Verification-only (no code change). Confirms every consumer of `graph.json` tolerates
the A1+ schema bump: additive node kinds (`command|tool|surface`), the additive
`confidence` link field, and the resulting shift in every corpus's `graph_json_sha256`
(BLAKE3 digest).

## Step 1 — Consumers reviewed

All consumers read by key (`.get("...")`) and ignore unknown fields/kinds. None deserialize
into a `#[serde(deny_unknown_fields)]` struct; none pin an exact `graph.json` byte shape.

| Consumer | Location | Reads | Tolerant? |
|---|---|---|---|
| `GraphifyReader::from_value` | `vox-graphify-reader/src/lib.rs:75` | `nodes[].id/label/name/community`, `links\|edges[].source/target` by key | Yes — unknown fields/kinds skipped; new `cmd:`/`tool:`/`surface:` nodes are just more nodes; `confidence` ignored. |
| 5 MCP tools | `vox-orchestrator-mcp/src/graphify_tools.rs` | parse to `serde_json::Value`, then `from_value` / `.get()` (`load_graph_json` at :333) | Yes — value-based, no struct schema. |
| GUI `vox_graphify_status` | `vox-gui/src/commands/graphify.rs:34` → `assess_corpus_status` | reads the **manifest** only (sha/mtime/ttl freshness), never `graph.json` node/link shape | Yes — schema bump is invisible; digest shift is expected (freshness compares manifest sha to git HEAD, not a pinned value). |
| `graph_stats_from_json` | `vox-config/src/graphify.rs:225` | `nodes.len()`, `links\|edges.len()` | Yes — counts grow with new nodes; that is the intended effect. |
| `lexical_search_graph` | `vox-config/src/graphify.rs:287` | `nodes[].label/id/name` by key | Yes — new nodes become searchable; unknown fields ignored. |
| `project_graph_nodes_for_ingest` | `vox-config/src/graphify.rs:324` | `nodes[].label/id/name/type`, serializes whole node to `content` | Yes — `type` falls back to `"graph_node"`; whole-node serialization carries `confidence`/new kinds harmlessly. |
| `lens::collapse_to_modules` | `vox-graphify-reader/src/lens.rs:12` | `nodes[].id`, `links\|edges[].source/target`; ignores `kind` | Yes (reads by key) — but see Step 2: it would fold registry nodes into spurious "modules". |
| Golden tests | `vox-graphify-reader/tests/rebuild_tests.rs` | `graph_json_sha256` only length-checked (`>= 32`, :35); `node_count`/`edge_count`/`links.len()` asserted on tiny fixed inputs (:91,:144,:154-155) | No exact-byte/digest golden exists. The small `node_count`/`edge_count` asserts belong to A1/B1's own behavior changes (updated there), not a schema-tolerance problem. |
| `overlay_tests` / `reachability_tests` / `lens_tests` | `vox-graphify-reader/tests/*.rs` | construct their own `nodes`/`links` JSON; read by key | Yes — independent of the corpus schema. |

## Step 2 — Decisions

- **`module-anchor` is NOT introduced.** Import edges are dropped (Phase B note); no synthetic
  `module::*` anchor nodes are added.
- **`command|tool|surface` node kinds and the `confidence` link field are additive.** No consumer
  rejects them; all read by key.
- **A1 rewrites every corpus's `graph_json_sha256` (BLAKE3).** This is **expected, not a regression** —
  adding the `confidence` field to every link mutates `graph.json` bytes for every corpus, so each
  digest shifts. Freshness logic (`assess_corpus_status`, `lexical_lag` at `graphify.rs:389`) compares
  manifest digests to git HEAD / each other, not to any pinned constant, so the shift only marks
  corpora stale until the next rebuild — the intended behavior.
- **`lens::collapse_to_modules` SHOULD filter `cmd:`/`tool:`/`surface:` nodes** out of `modules`-mode
  output. `module_of(id)` splits on `::`; registry node ids (`cmd:do_it`, `tool:t`, `surface:...`) have
  no `::`, so each would become its own bogus single-node "module" and pollute the coarse view.
  **Recommended: yes** — add a `kind`-based filter in lens (or skip ids without `::`) in the
  Task-E-adjacent work **if a `modules`-mode corpus exists**. No `modules` corpus is registered today
  (only `repo-code-graph`), so this is a guard for when one is added, not a current bug.

No code changed in this task.
