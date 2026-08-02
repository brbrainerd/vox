---
name: vox-graph
description: "Vox Graph structural discovery: call vox_search / vox_discover BEFORE grep. Use this whenever tracing how code connects — call sites, surface coverage, dead-end commands, or 'what relates to X'."
---

# Vox Graph — Graph-First Discovery

The Vox Graph engine indexes the codebase as a structural graph (fn, struct, surface, command, tool nodes + call/dependency edges). It is faster and more precise than text search for connection questions.

## When to use this skill

- **"Where is X called?"** — `vox_search_neighbors` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_search_neighbors` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_search_neighbors` with seed of Z's node id → returns neighbors + their coverage class.
- **"Which commands have no GUI surface?"** — query the `CliOnly` coverage class from the graph's coverage report.

## Rule

**ALWAYS call `vox_search_structural` or `vox_search_neighbors` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).

## Key MCP tools

| Tool | Purpose |
|---|---|
| `vox_search_status` | Read-only freshness report for the structural corpus |
| `vox_search_structural` | Lexical search over an on-disk corpus graph |
| `vox_search_neighbors` | BFS neighbor expansion from seed node IDs |
| `vox_search_callers` / `vox_search_callees` | Direct caller/callee expansion from a seed node |
| `vox_search_path` | Shortest path between two node IDs |
| `vox_search_compare` | Diff two corpus manifests (node/edge/community delta) |
| `vox_search_rebuild` | Rebuild the structural graph for a corpus |

## Graph verbs (CLI)

```
vox graph rebuild --corpus <id>      # rebuild the structural graph
vox graph status                     # freshness report
vox graph index <path> --id <id>     # index a new target as a corpus (path is positional, not a --path flag)
vox graph query <args>               # lexical search over the corpus (same lexical scorer as vox_search_structural)
vox graph coverage --corpus <id>     # coverage classification report
vox graph refresh --corpus <id>      # rebuild-or-ingest based on staleness
```

`graph` is the canonical subcommand name (`crates/vox-cli/src/lib.rs`); `graphify` and `search` are backward-compatible aliases only — do not use them in new documentation.

## Determinism firewall

The structural graph is deterministic and read-only. Semantic overlays (embedding, LLM relation labels) are query-time and provenance-labeled — never written into the graph itself. You can trust graph results as ground truth for the codebase structure at the time of the last rebuild.

## Naming note

This skill's tool names (`vox_search_*`) and CLI form (`vox graph <verb>`) match what's actually registered in `crates/vox-orchestrator-mcp/src/dispatch.rs`/`input_schemas.rs` and `crates/vox-cli/src/lib.rs` today. A prior version of this skill (and of the plan task that produced this fix) incorrectly claimed a `vox_graphify_*`/`vox graphify` naming was current and that a `vox_search_*` rename was still pending — that rename landed via #406 over a month before this correction; this file was simply out of date.
