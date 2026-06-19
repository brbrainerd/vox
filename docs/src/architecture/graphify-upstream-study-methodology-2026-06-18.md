---
title: "Graphify Upstream Source — Study & Port-Parity Methodology (2026-06-18)"
description: "How to use the graphify-upstream corpus to navigate Graphify's Python implementation and verify the native Rust port stage-by-stage."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Agents porting or auditing the native graphify builder need a stage map from upstream Python to native Rust."
---

# Graphify Upstream Source — Study & Port-Parity Methodology (2026-06-18)

## Acquire the corpus
```bash
vox run scripts/graphify-study-source.vox
```
Clones `https://github.com/safishamsi/graphify` into `.vox/cache/graphify-src/` and builds the
`graphify-upstream` corpus (`vox graphify index`, native Python extraction).

## Navigate it
- `vox graphify status --corpus graphify-upstream` — freshness + node/edge counts.
- Agents: `vox_graphify_search { corpus: "graphify-upstream", query: "<topic>" }` to find entry
  nodes, then `vox_graphify_query { corpus: "graphify-upstream", seeds: [...] }` to expand.

## Port-parity stage map
| Upstream stage (Python) | Native Rust equivalent | Status |
|---|---|---|
| `graphify.detect` (file classification) | `rebuild` WalkDir filter (`rs`/`ts`/`js`/`py`) | Native, coarser |
| `graphify.extract` (AST) | `ast::extract_ast_in_module` (`syn` + tree-sitter rust/ts/py) | Native |
| `graphify.llm` (semantic doc/media nodes) | Vox orchestrator LLM egress (hybrid lane) | Not in reader (by design) |
| `graphify.build` (NetworkX) | `rebuild` builds NetworkX-shaped `graph.json` | Native |
| `graphify.cluster` (Leiden) | `cluster::cluster_nodes` (`leiden-rs`) | Native |
| `graphify.analyze` (god-nodes/surprises) | `GraphifyReader::god_nodes`; surprises not ported | Partial |
| `graphify.report` / exporters | not ported (HTML/Obsidian/Neo4j stay upstream) | Out of scope |
| content-hash semantic cache | `cache::CacheManager` (BLAKE3 per file + extractor version) | Native |

## Parity-check procedure
For each native stage, query `graphify-upstream` for the equivalent upstream symbol (e.g.
`"god node degree"`, `"leiden partition"`, `"detect classify"`), read the upstream behavior, and
confirm the native module matches or record the intentional divergence here. Known divergences:
semantic/LLM extraction and HTML/Neo4j exporters are deliberately not ported (hybrid boundary —
see the capabilities-audit SSOT).
