---
title: "Crate Classification Audit (2026-05-08)"
description: "DEPRECATED tombstone — historical pre-reorg snapshot only. Do not use for contraction or architecture decisions; see where-things-live.md and layers.toml."
category: "Architecture SSOTs"
status: "deprecated"
superseded_by:
  - "2026-05-08-workspace-reorg-outcome.md"
  - "2026-05-08-crate-org-followup-design.md"
training_eligible: false
training_rationale: "Tombstoned 2026-06-10 — many DEAD labels are stale; excluded from agent training corpora."
---

# Crate Classification Audit (2026-05-08)

> **TOMBSTONE — do not use for contraction or architecture decisions (2026-06-10).** This page is a historical pre-reorg snapshot. Many crates listed as **DEAD** below are live today (`vox-mcp-registry`, `vox-test-harness`, `vox-grammar-export`, etc.). For current crate health use [`where-things-live.md`](where-things-live.md), [`layers.toml`](layers.toml), and `cargo run -p vox-arch-check`.

> **Snapshot note:** This is a point-in-time audit snapshot from 2026-05-08, used to inform the workspace reorg + crate-org-followup work. Crate classifications below reflect the *pre-fix* state — see `2026-05-08-workspace-reorg-outcome.md` and `2026-05-08-crate-org-followup-design.md` for what actually shipped.

This audit classifies every crate in `crates/` against five labels:

- **CORE** — compiled into the default `vox-cli` binary (no feature flags required)
- **PLUGIN** — heavy deps or hardware-specific; ships as a dylib plugin
- **SHARED** — used by both CORE crates and PLUGIN crates; must remain in the workspace
- **DEAD** — zero consumers in `Cargo.toml`; candidate for deletion after verification
- **MISPLACED** — compiled as CORE today but should be extracted to a plugin

> Methodology: `grep -r "<crate>" crates/ --include="Cargo.toml"` to count consumers, combined with review of `vox-cli/Cargo.toml` direct/transitive deps. Feature-gated-only deps (optional = true) are noted.

---

## Summary Table

| Crate | Classification | Consumers | Notes / Action |
|---|---|---|---|
| workspace-hack | SHARED | all | cargo-hakari generated; keep |
| vox-bounded-fs | CORE | 18 | async FS jailing used widely |
| vox-secrets | CORE | 21 | auth/keys, used in CLI, orchestrator, populi |
| vox-cli | CORE | 0 (it IS the binary) | default binary |
| vox-cli-core | CORE | 1 (vox-cli) | CLI utilities |
| vox-compiler | CORE | 7 | Vox language compiler |
| vox-config | CORE | 8 | config types |
| vox-container | CORE | 4 | sandboxed execution |
| vox-corpus | CORE | 3 | dataset pipeline |
| vox-db | CORE | 4 | SQLite/Turso store |
| vox-eval | CORE (optional) | 1 | behind `dep:vox-eval` in populi |
| vox-identity | CORE | 4 | auth identity |
| vox-crypto | CORE | 3 | crypto primitives |
| vox-jsonschema-util | CORE | 3 | JSON schema |
| vox-mesh-types | SHARED | 2 | mesh type definitions used by core + plugin |
| vox-openai-sse | SHARED | 2 | SSE wire used by oratio + orchestrator |
| vox-openai-wire | SHARED | 2 | OpenAI wire types |
| vox-orchestrator | CORE | 5 | task orchestrator |
| vox-orchestrator-types | SHARED | 4 | types shared between orchestrator + plugins |
| vox-plugin-api | SHARED | 14 | plugin ABI surface; MUST stay |
| vox-plugin-catalog | CORE | 2 | SSOT plugin catalog |
| vox-plugin-host | CORE | 4 | plugin loader/discovery |
| vox-package | CORE | 2 | project management |
| vox-populi | CORE | 3 | ML inference/training dispatch |
| vox-primitives | CORE | 4 | shared primitive types |
| vox-protocol | CORE | 3 | A2A protocol |
| vox-reqwest-defaults | CORE | 6 | HTTP client defaults |
| vox-repository | CORE | 2 | repo abstraction |
| vox-actor-runtime | CORE | 2 | runtime helpers |
| vox-scaling-policy | CORE | 2 | scaling decisions |
| vox-ssg | CORE (direct dep) | 1 (vox-cli) | static site gen; see MISPLACED note |
| vox-checksum-manifest | CORE | 1 | checksums |
| vox-install-policy | CORE | 1 | install policy |
| vox-capability-registry | CORE | 3 | capability registry |
| vox-constrained-gen | CORE | 3 | constrained generation |
| vox-project-scaffold | CORE | 1 | project scaffolding |
| vox-publisher | CORE (partial) | 1 | publishing; social adapters behind features |
| vox-exec-grammar | DEAD | 0 | no consumers in workspace |
| vox-grammar-export | DEAD | 0 | no consumers in workspace |
| vox-schola | DEAD | 0 | no consumers (merge_qlora uses inline types) |
| vox-scientia-core | DEAD | 0 | no consumers in workspace |
| vox-scientia-social | DEAD | 0 | no consumers in workspace |
| vox-scientia-ingest | DEAD | 0 | no consumers; ingestion pipeline unused |
| vox-search | CORE | many | agent retrieval execution (`execute_search_plan`, MCP); see [search-retrieval-ssot-2026.md](search-retrieval-ssot-2026.md) |
| vox-socrates-policy | DEAD | 0 | policy engine; no consumer |
| vox-spool | DEAD | 0 | spool subsystem; no consumer |
| vox-tools | DEAD | 0 | tools crate; re-exported via oratio (orphaned) |
| vox-webhook | DEAD | 0 | webhook handlers; no consumer |
| vox-workflow-runtime | DEAD (optional) | 0 direct / 1 optional | optional in vox-cli behind `workflow-runtime` feature |
| vox-mcp-meta | DEAD | 0 | MCP meta; no consumers |
| vox-mcp-registry | DEAD | 0 | MCP registry stub; orphaned |
| vox-doc-inventory | DEAD | 0 | doc inventory; no consumers |
| vox-integration-tests | DEAD | 0 | test crate with no callers; uses vox-lsp |
| vox-test-harness | DEAD | 0 | test harness; no consumers |
| vox-lsp | CORE (optional) | 1 (orchestrator behind feature) | behind `toestub-gate` feature |
| vox-openclaw-runtime | CORE (optional) | 2 (orchestrator + runtime, mandatory) | OpenClaw / ARS runtime facade; used at runtime |
| vox-audio-ingress | MISPLACED | 1 (self-referential via vox-oratio dep) | brings in vox-oratio; should be PLUGIN |
| vox-browser | DEAD | 0 | browser abstraction; no consumer; MISPLACED into vox-plugin-browser |
| vox-forge | CORE (optional) | 1 (vox-cli behind `coderabbit` feature) | GitHub integration (GitLab deprecated 2026-06-03) |
| vox-git | CORE (optional) | 1 (vox-cli behind `coderabbit` feature) | git integration |
| vox-gamify | CORE (optional) | 0 direct / 1 optional | gamification; optional in vox-cli |
| vox-code-audit | CORE (optional) | 1 optional | completion guard; optional |
| vox-doc-pipeline | CORE | 1 | doc pipeline runner |
| vox-dashboard | CORE (optional) | 1 (vox-cli behind `dashboard` feature) | VS Code extension dashboard |
| vox-skills | CORE (optional) | 1 (vox-cli behind `ars` feature) | skill registry ARS shim |
| vox-oratio | MISPLACED | 2 optional | Candle Whisper; should be fully in vox-plugin-oratio |
| vox-tensor | SHARED | 4 (all optional or plugin) | GPU tensor; needed by plugins; see note |
| vox-ml-cli | CORE | 1 (vox-cli) | `vox mens` subcommand |
| vox-populi | CORE | 3 | populi inference; hosts burn/candle gated code |
| — | — | — | — |
| **PLUGIN crates** | | | |
| vox-plugin-browser | PLUGIN | 0 runtime consumers | CDP browser plugin |
| vox-plugin-cloud | PLUGIN | 0 runtime consumers | cloud sync plugin |
| vox-plugin-mens-candle-cuda | PLUGIN | 0 runtime consumers | Candle CUDA ML backend |
| vox-plugin-noop-code | PLUGIN | 0 runtime | test fixture |
| vox-plugin-noop-code-bad-abi | PLUGIN | 0 runtime | ABI rejection test fixture |
| vox-plugin-noop-skill | PLUGIN | 0 runtime | skill discover test fixture |
| vox-plugin-nvml-probe | PLUGIN | 0 runtime | NVIDIA NVML hardware probe |
| vox-plugin-oratio | PLUGIN | 0 runtime | Whisper STT plugin |
| vox-plugin-oratio-mic | PLUGIN | 0 runtime | microphone capture plugin |
| vox-plugin-populi-mesh | PLUGIN | 0 runtime | mesh transport + skill |
| vox-plugin-script-execution | PLUGIN | 0 runtime | script execution plugin |
| vox-plugin-skill-* | PLUGIN | 0 runtime | agent skills (skill-only plugins) |
| vox-plugin-tensor-burn-wgpu | PLUGIN | 0 runtime | Burn + wgpu tensor backend plugin |

---

## DEAD crates — Deletion Candidates

The following crates have zero consumers in `Cargo.toml` files across the workspace. They are safe to remove after verifying they have no external users or build-script roles:

| Crate | Reason |
|---|---|
| `vox-exec-grammar` | No Cargo.toml consumer found |
| `vox-grammar-export` | No Cargo.toml consumer found |
| `vox-schola` | Inline serde types in `vox-ml-cli/merge_qlora.rs` replaced it; plugin owns merge |
| `vox-scientia-core` | No consumers; social/ingestion pipeline not wired |
| `vox-scientia-social` | No consumers |
| `vox-scientia-ingest` | No consumers |
| `vox-socrates-policy` | No consumers |
| `vox-spool` | No consumers |
| `vox-webhook` | No consumers |
| `vox-tools` | Orphaned; `vox-oratio` was extracted to a plugin |
| `vox-mcp-meta` | MCP meta stub; no consumers |
| `vox-mcp-registry` | Orphaned MCP registry |
| `vox-doc-inventory` | No consumers |
| `vox-integration-tests` | Self-contained test binary but no CI wiring found |
| `vox-test-harness` | No consumers |
| `vox-browser` | BrowserAutomation extracted to `vox-plugin-browser`; this crate is the old host-side abstraction with zero consumers left |

> **Action**: Before deleting each, verify `cargo check --workspace` still compiles. Run `cargo tree -p vox-cli | grep <crate>` for each — if empty, proceed to delete.

---

## MISPLACED crates — Extraction Candidates

| Crate | Current State | Recommended Action |
|---|---|---|
| `vox-oratio` | Used optionally in vox-ml-cli and vox-orchestrator; pulls in heavy Candle deps | Finish extraction to `vox-plugin-oratio`; all remaining uses should go through the plugin dispatch boundary |
| `vox-audio-ingress` | Only consumer is itself (via `vox-oratio` dep); orphaned | Delete or fold into vox-plugin-oratio-mic |
| `vox-ssg` | In vox-cli directly; SSG is a publishing concern, not core CLI | Consider extracting SSG commands to a skill plugin; low urgency |

---

## SHARED crates — Must Remain

These are used by both CORE and PLUGIN code paths and cannot be safely moved:

| Crate | Reason |
|---|---|
| `vox-plugin-api` | The stable ABI surface itself |
| `vox-plugin-host` | Host-side discovery + loader; called from vox-cli |
| `vox-plugin-catalog` | SSOT used by vox-cli CI gates |
| `vox-mesh-types` | Mesh types shared across core and populi-mesh plugin |
| `vox-orchestrator-types` | Task/budget types shared across orchestrator and plugin dispatch |
| `vox-tensor` | `data`, `grpo`, `replay`, `lora_config` modules used by both plugins and core; GPU-gated modules only used in plugins |

---

## `vox-tensor` — Deduplication Note

`vox-tensor` currently duplicates several types that also exist in `vox-populi/src/mens/tensor/`:

- `LoraConfig` / `lora_memory_estimate` — both `vox-tensor/src/lora_config.rs` and `vox-populi/src/mens/tensor/lora/` define them
- GPU-gated modules (`lora`, `optim`, `tensor`, `train`, `vox_nn`) — these should move to `vox-plugin-tensor-burn-wgpu`; the remaining always-compiled modules (`data`, `grpo`, `replay`, `lora_config`) are the actual shared surface

**Recommended**: Keep `vox-tensor` as a SHARED crate but shrink its scope to only the non-GPU modules (`data`, `grpo`, `replay`, `lora_config`). The GPU modules belong in `vox-plugin-tensor-burn-wgpu/src/`.

---

## Catalog Dead Entries

Two plugin IDs appear in `catalog.toml` but have no corresponding crate in `crates/`:

| Catalog ID | Status |
|---|---|
| `execution-api` | No `vox-plugin-execution-api` crate; referenced only in catalog with `bundled-in = []` |
| `stub-check` | No `vox-plugin-stub-check` crate; referenced only in catalog with `bundled-in = []` |

These are pre-declared future plugins. They are harmless (not in any bundle), but their `default-source` points to non-existent GitHub repos. They should either be removed from the catalog or have placeholder crates created.

---

## Recommendations (Priority Order)

1. **Delete DEAD crates** (P1, low risk): Start with `vox-exec-grammar`, `vox-grammar-export`, `vox-schola`. Run `cargo check` after each batch.
2. **Remove catalog ghost entries** (P2, trivial): Delete `execution-api` and `stub-check` entries from `catalog.toml` or add placeholder crates.
3. **Finish oratio extraction** (P2): Move remaining `vox-oratio` uses to `vox-plugin-oratio` dispatch.
4. **Shrink vox-tensor GPU surface** (P3): Move `lora`, `optim`, `tensor`, `train`, `vox_nn` into `vox-plugin-tensor-burn-wgpu`. Keep non-GPU modules.
5. **Delete `vox-browser`** (P3): Zero consumers; `vox-plugin-browser` is the canonical implementation.
