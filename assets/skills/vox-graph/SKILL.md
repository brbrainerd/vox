---
name: vox-graph
description: "Vox Graph structural discovery: call vox_search / vox_discover BEFORE grep. Use this whenever tracing how code connects — call sites, surface coverage, dead-end commands, or 'what relates to X'."
---

# Vox Graph — Graph-First Discovery

The Vox Graph engine indexes the codebase as a structural graph (fn, struct, surface, command, tool nodes + call/dependency edges). It is faster and more precise than text search for connection questions.

## When to use this skill

- **"Where is X called?"** — `vox_search` over the `repo-code-graph` corpus (BFS from the fn node) instead of grep.
- **"What surfaces expose command Y?"** — `vox_discover` with seed `cmd:Y` → follows edges to `surface:` nodes.
- **"What is related to Z?"** — `vox_discover` with seed of Z's node id → returns neighbors + their coverage class.
- **"Which commands have no GUI surface?"** — query the `CliOnly` coverage class from the graph's coverage report.

## Rule

**ALWAYS call `vox_search` or `vox_discover` first for connection/discovery questions.** Fall back to grep only when the graph gives no result (e.g., the corpus is stale or the file is not yet indexed).

## Key MCP tools

| Tool | Purpose |
|---|---|
| `vox_search_query` | Text-or-id search across corpora |
| `vox_search_path` | Shortest path between two node ids |
| `vox_discover` | Graph-augmented retrieval: seed → expand → composite rank |
| `vox_search_status` | Corpus freshness + coverage summary |

## Graph verbs (CLI)

```
vox search rebuild --corpus <id>   # rebuild the structural graph
vox search status                  # freshness report
vox search index                   # re-index after code change
```

*(There is no `graph` infix — `vox search <verb>` IS the graph subgroup, per vs1. See G6.)*

## Determinism firewall

The structural graph is deterministic and read-only. Semantic overlays (embedding, LLM relation labels) are query-time and provenance-labeled — never written into the graph itself. You can trust graph results as ground truth for the codebase structure at the time of the last rebuild.
