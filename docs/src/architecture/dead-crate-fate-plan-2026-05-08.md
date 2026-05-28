---
title: "Dead Crate Fate Plan (2026-05-08)"
description: "Per-crate recommendation for the 17 DEAD and 3 MISPLACED workspace crates: delete, wire up, extract to plugin, or freeze."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Plan for resolving each abandoned crate either by removal or by wiring it into the plugin-first architecture; useful reference for future workspace hygiene."
---

# Dead Crate Fate Plan (2026-05-08)

Companion to `crate-classification-2026-05-08.md`. This doc makes a concrete decision for every DEAD and MISPLACED crate and flags three classification errors found during investigation.

---

## 1. Executive Summary

| Fate | Count | Crates |
|---|---|---|
| **DELETE** | 10 | vox-schola, vox-scientia-core, vox-scientia-social, vox-scientia-ingest†, vox-spool, vox-socrates-policy, vox-tools, vox-mcp-meta, vox-browser, vox-audio-ingress |
| **WIRE-UP-AS-IS** | 4 | vox-exec-grammar, vox-search†, vox-doc-inventory†, vox-mcp-registry |
| **EXTRACT-TO-PLUGIN** | 2 | vox-webhook, vox-grammar-export |
| **KEEP-FROZEN** | 3 | vox-workflow-runtime, vox-integration-tests, vox-test-harness |
| **REWRITE-AS-PLUGIN** | 1 | vox-ssg (MISPLACED) |
| **Finish extraction** | 1 | vox-oratio (MISPLACED) — extraction already in progress |
| **Catalog ghosts** | 2 | execution-api, stub-check — already cleaned up per catalog.toml comment |

† Classification correction: these three crates are NOT dead. See §2.

**Total estimated effort for all DELETE actions:** XS–S each, ~4h combined.
**Highest-leverage actions:**
1. Delete the 10 DELETE crates — reduces workspace compile surface and removes dead dep trees (~8k LOC).
2. Wire up `vox-exec-grammar` to the exec-policy enforcement path in `vox-container` (already designed, just not called).
3. Finish `vox-oratio` extraction — eliminates the last direct Candle dependency bleed from CORE.

---

## 2. Classification Corrections

The original audit missed three crates that are **mandatory (non-optional) direct deps** of `vox-cli`:

| Crate | Claim in audit | Reality |
|---|---|---|
| `vox-search` | DEAD, 0 consumers | `vox-cli` + `vox-orchestrator` depend on it unconditionally |
| `vox-scientia-ingest` | DEAD, 0 consumers | `vox-cli` depends on it unconditionally (line 135 of vox-cli/Cargo.toml) |
| `vox-doc-inventory` | DEAD, 0 consumers | `vox-cli` depends on it unconditionally (line 138 of vox-cli/Cargo.toml) |

These should be reclassified **CORE** in the audit table. Their fates are addressed below.

---

## 3. Per-Crate Analysis

### vox-exec-grammar
**Purpose**: Pure-Rust AST parser and risk classifier for shell/Vox command invocations; backs `exec-policy.v1.yaml` enforcement without requiring PowerShell.
**Why dead**: Created for ADR-026 exec-policy enforcement but the caller in `vox-container` was never wired up. The crate itself is functional (tokeniser + risk classifier complete per its own status comment).
**LOC / size**: 956 lines, 4 files.
**Concept fit today**: Yes — `vox-container` enforces a sandbox policy; having a pure-Rust fallback parser that classifies shell command risk is exactly the right thing for hosts without PowerShell. The concept is sound and the impl is done.
**Recommendation**: **WIRE-UP-AS-IS** — add a dep from `vox-container` (or `vox-cli-core`) and call `risk::classify` before exec dispatch.
**How**: Add `vox-exec-grammar = { workspace = true }` to `vox-container/Cargo.toml`; call `vox_exec_grammar::risk::classify` inside the exec-policy gate that already exists. The ADR-026 contract file (`contracts/terminal/exec-policy.v1.yaml`) already describes the interface.
**Effort**: S (1h)
**Risk**: Low. The crate has no unsafe code and only `serde` / `thiserror` deps. Verify the `ExecPolicy` struct maps cleanly to whatever `vox-container` already reads from the YAML contract.

---

### vox-grammar-export
**Purpose**: Multi-format grammar exporter (EBNF, GBNF, JSON Schema, Lark, XGrammar-2, tree-sitter) for the Vox language grammar — feeds constrained inference backends and Python bridges.
**Why dead**: Built for the constrained-generation pipeline (`vox-constrained-gen`) but the caller was never added. Likely shelved when constrained-gen's scope was narrowed.
**LOC / size**: 1,579 lines, 11 files — substantial, non-trivial implementation.
**Concept fit today**: Yes — `vox-constrained-gen` is CORE and active; GBNF and XGrammar-2 formats are directly relevant for structured output enforcement. The Lark format supports the Python bridge described in the interop plan.
**Recommendation**: **EXTRACT-TO-PLUGIN** — move into a `vox-plugin-grammar-export` plugin so grammar export is available on demand without bloating the default CLI compile. The formats (GBNF, XGrammar-2) are only needed when running constrained inference.
**How**: Create `crates/vox-plugin-grammar-export/`; implement `GrammarExportPlugin` behind the plugin ABI; dispatch from `vox-constrained-gen` via plugin host when available.
**Effort**: M (½ day)
**Risk**: Medium. The existing 11-file implementation is self-contained (no unsafe, no heavy deps). Risk is ABI design for the export request/response types. Check whether `GrammarFormat` enum needs to be in a shared types crate accessible across the plugin boundary.

---

### vox-schola
**Purpose**: Standalone ML training/serving binary (`vox-schola`) — a thin CLI wrapper around QLoRA training.
**Why dead**: The QLoRA training path was absorbed into `vox-ml-cli` (the `merge_qlora.rs` command) and `vox-plugin-mens-candle-cuda`. The binary is 79 lines and just re-dispatches to `vox-cli-core`. Nothing calls it; the `vox mens` subcommand replaced it.
**LOC / size**: 79 lines, 1 file.
**Concept fit today**: No — the training surface now lives in `vox-ml-cli` + `vox-populi` + plugins.
**Recommendation**: **DELETE**
**How**: Remove `crates/vox-schola/`; remove from `Cargo.toml` workspace members.
**Effort**: XS (15min)
**Risk**: None. Verify `cargo tree -p vox-cli | grep vox-schola` is empty. The binary name `vox-schola` was a separate binary; confirm no release workflow ships it.

---

### vox-scientia-core, vox-scientia-social (grouped)
**Purpose**: Façade crates re-exporting `vox-publisher`'s scientia modules (`scientia_contracts`, `scientia_discovery`, etc.) and distribution planning helpers.
**Why dead**: Both are pure pass-through facades over `vox-publisher`. They have zero consumers — `vox-publisher` is consumed directly. The façade split was planned but never adopted downstream. `vox-scientia-social` is 14 lines.
**LOC / size**: 37 lines (core), 14 lines (social).
**Concept fit today**: No — adding a façade crate with zero consumers that re-exports another crate is pure overhead. If the scientia surface needs a stable API contract, that belongs in `vox-publisher`'s own public module organization.
**Recommendation**: **DELETE** both.
**How**: Remove `crates/vox-scientia-core/` and `crates/vox-scientia-social/`; remove from workspace members. The actual scientia logic stays in `vox-publisher`.
**Effort**: XS (15min each)
**Risk**: Low. Verify nothing imports `vox_scientia_core::*` or `vox_scientia_social::*` anywhere in the workspace (`grep -rn "vox_scientia" crates/ src/`).

---

### vox-scientia-ingest
**Purpose**: RSS feed crawler and deduplicator for ingesting research items into `vox-search` and `vox-db`.
**Why dead (correction)**: The original audit called this DEAD but `vox-cli/Cargo.toml` line 135 has it as a mandatory dep. It is compiled into the CLI binary unconditionally.
**LOC / size**: 142 lines, 3 files.
**Concept fit today**: The ingest concept belongs in a plugin (it brings `feed-rs`, `fnv`, `reqwest` into the default CLI compile for a feature most users never use). However, since it is currently wired into `vox-cli`, an immediate DELETE would break the build.
**Recommendation**: **DELETE** — but only after removing the `vox-cli` dependency first. The two-step is: (1) audit what `vox-cli` actually calls from this crate, (2) either move that call to `vox-publisher` or gate it behind a feature flag, then (3) delete the crate.
**How**: Search for `vox_scientia_ingest::` usage in `crates/vox-cli/src/`; if the surface is minimal, inline it into `vox-publisher`'s scholarly-external-jobs feature. Then remove the dep and the crate.
**Effort**: S (1h — need to audit actual usage in CLI before removing)
**Risk**: Medium. Do not delete until the `vox-cli` dep is removed; breaking the mandatory dep would break the build.

---

### vox-search
**Purpose**: Local-first retrieval execution — memory hybrid, repo inventory, Codex chunks, policy, and optional lexical (Tantivy) and vector (Qdrant) backends.
**Why dead (correction)**: NOT dead. `vox-cli` and `vox-orchestrator` depend on it unconditionally. It has 23 source files and 3,513 LOC — a substantial, actively-structured crate.
**LOC / size**: 3,513 lines, 23 files.
**Concept fit today**: Fully relevant. Search is a core capability for the orchestrator and CLI. The optional Tantivy/Qdrant/Tavily backends are appropriately feature-gated.
**Recommendation**: **WIRE-UP-AS-IS** — it is already wired. Reclassify as CORE in the audit. No action needed beyond correcting the classification.
**How**: Update `crate-classification-2026-05-08.md` to move `vox-search` from DEAD to CORE with consumers = 2.
**Effort**: XS (doc correction only)
**Risk**: None for the crate itself. The optional `qdrant-vector` and `tavily` features bring external network deps into the default compile; consider whether the `default` feature enabling them is intentional.

---

### vox-socrates-policy
**Purpose**: Shared Socrates anti-hallucination confidence thresholds and risk classification — numeric constants for orchestrator, MCP, and TOESTUB review gates.
**Why dead**: Built to be consumed by orchestrator, MCP, and toestub but none of them added the dep. `vox-capability-registry` handles the policy surface those callers actually use.
**LOC / size**: 752 lines, 6 files.
**Concept fit today**: Partial. The Socrates protocol is referenced in `docs/src/architecture/socrates-protocol-ssot.md` and the constants are meaningful. However, the orchestrator already has its own confidence thresholds. Maintaining a separate crate for numeric constants that nothing reads creates drift risk.
**Recommendation**: **DELETE** — consolidate the meaningful constants (`ConfidencePolicy`, `RiskBand`, etc.) into `vox-capability-registry` or `vox-orchestrator-types` where they will actually be used. The 752 LOC is mostly well-structured types that can be merged.
**How**: Copy the types worth keeping (`ConfidencePolicy`, `ComplexityBand`, `RiskBand`) into `vox-orchestrator-types/src/socrates.rs`. Delete `crates/vox-socrates-policy/`.
**Effort**: S (1h — type migration + verification)
**Risk**: Low. No consumers to update. Risk is only ensuring the migrated constants match whatever the orchestrator currently hard-codes.

---

### vox-spool
**Purpose**: JSON-L backed job queue and spool abstraction for async task dispatch.
**Why dead**: No consumer ever added. Likely pre-built for the orchestrator's task queue but the orchestrator implemented its own queuing inline.
**LOC / size**: 270 lines, 3 files (queue.rs, jsonl.rs, lib.rs).
**Concept fit today**: The orchestrator and workflow-runtime both have async job needs, but a 270-line JSONL queue crate is too thin to be a plugin and too redundant to be CORE alongside the orchestrator's own queue.
**Recommendation**: **DELETE**
**How**: Remove `crates/vox-spool/`. If the JSONL helper is needed elsewhere, it can be inlined.
**Effort**: XS (15min)
**Risk**: None. Zero consumers. Verify with `grep -rn "vox.spool\|vox_spool" crates/`.

---

### vox-webhook
**Purpose**: HTTP webhook gateway — inbound receiver, outbound delivery with retry/signing, Discord/Slack channel adapters, HMAC signing, Axum router wiring.
**Why dead**: Built completely (1,460 LOC, 7 modules) but no caller added it. The orchestrator handles external events through its own inbox, not through this router.
**LOC / size**: 1,460 lines, 7 files.
**Concept fit today**: Yes — webhook ingress for GitHub events, Discord, Slack is a legitimate integration surface. The implementation is solid (signing, retry, bridge to orchestrator inbox). It's the right concept in the wrong place (CORE crate that should be a plugin).
**Recommendation**: **EXTRACT-TO-PLUGIN** — move into `vox-plugin-webhook`. The `WebhookOrchestratorBridge` is the right integration point. The plugin dispatches received webhook events into the orchestrator inbox via the plugin ABI.
**How**: Create `crates/vox-plugin-webhook/`; implement plugin ABI entry; move all 7 modules; wire `OrchestratorInboxItem` dispatch through `vox-orchestrator-types`. The `vox-orchestrator` dep in current Cargo.toml would become a dispatch-via-plugin call.
**Effort**: M (½ day — mostly moving code + ABI plumbing)
**Risk**: Medium. `vox-webhook` depends on `vox-orchestrator` directly; the plugin must reverse this to a dispatch-out pattern. The signing and delivery logic is self-contained and low-risk.

---

### vox-tools
**Purpose**: In-process MCP tool executor for Mens chat — OpenAI-compatible tool definitions and direct execution without MCP transport.
**Why dead**: After `vox-oratio` was extracted to `vox-plugin-oratio`, the transcription path in this crate was refactored to dispatch through the plugin (commit `f8f4cb0f9`). But the crate itself was never deleted or adopted by `vox-ml-cli`.
**LOC / size**: 580 lines, 3 files (lib.rs, mens_chat.rs, capability_registry/).
**Concept fit today**: The in-process tool execution concept was superseded by the plugin dispatch pattern. `vox-plugin-host` now provides the canonical discovery + dispatch surface. `vox-capability-registry` covers the registry aspect.
**Recommendation**: **DELETE** — the two components it wraps (`vox-capability-registry` + `vox-plugin-host`) already exist as first-class crates. The `mens_chat` tool list can live in `vox-ml-cli` directly.
**How**: Check what `vox-ml-cli` currently calls from `vox-tools` (likely nothing, given zero consumers). Remove `crates/vox-tools/`.
**Effort**: XS (15min)
**Risk**: Low. The audit says zero consumers. The transcription dispatch refactor already bypassed this crate. Confirm with `grep -rn "vox.tools\|vox_tools" crates/`.

---

### vox-mcp-registry
**Purpose**: Compile-time MCP tool name/description registry built from `contracts/mcp/tool-registry.canonical.yaml` via a build script.
**Why dead**: Built as a codegen step from the canonical YAML SSOT, but no consumer was ever added after `vox-mcp-meta` (its only downstream) also lost consumers.
**LOC / size**: 15 lines (lib.rs only; actual content generated at build time).
**Concept fit today**: The concept — a compile-time-generated SSOT registry from YAML — is sound and avoids duplication. It's currently the only crate with a build script that reads `contracts/mcp/tool-registry.canonical.yaml`.
**Recommendation**: **WIRE-UP-AS-IS** — wire it into `vox-orchestrator` or `vox-plugin-host` where MCP tool dispatch happens. Delete `vox-mcp-meta` (the redundant wrapper); let `vox-orchestrator` depend directly on `vox-mcp-registry`.
**How**: Add `vox-mcp-registry = { workspace = true }` to `vox-orchestrator/Cargo.toml`; use `TOOL_REGISTRY` to validate/enumerate tool names in the orchestrator's MCP dispatch. Delete `crates/vox-mcp-meta/`.
**Effort**: S (1h)
**Risk**: Low. The build script reads a YAML file from `contracts/`; verify the path is correct relative to the workspace root at build time.

---

### vox-mcp-meta
**Purpose**: Thin wrapper over `vox-mcp-registry` that re-exports `TOOL_REGISTRY` and adds taxonomy constants (A2A message types, etc.).
**Why dead**: Orphaned when its consumers migrated away; depends on `vox-mcp-registry` which is itself dead.
**LOC / size**: 62 lines (lib.rs only).
**Concept fit today**: No standalone role — it's a pass-through with extra constants that belong in `vox-orchestrator-types`.
**Recommendation**: **DELETE** — migrate the `A2A_MESSAGE_TYPES` and similar constants into `vox-orchestrator-types`; wire `vox-mcp-registry` directly (see above).
**How**: Move the constants array into `vox-orchestrator-types/src/a2a.rs`. Remove `crates/vox-mcp-meta/`.
**Effort**: XS (15min)
**Risk**: None. Zero consumers.

---

### vox-doc-inventory
**Purpose**: Generate and verify `docs/agents/doc-inventory.json` (schema v3) — replaces retired Python inventory scripts; provides `run_generate_inventory_cli` and verification utilities.
**Why dead (correction)**: NOT dead. `vox-cli/Cargo.toml` line 138 depends on it unconditionally. It has two binary targets (`vox-doc-inventory-generate`, `doc-inventory-generate`).
**LOC / size**: 775 lines, 12 files.
**Concept fit today**: Fully relevant — doc inventory is an active CI artifact.
**Recommendation**: **WIRE-UP-AS-IS** — already wired into `vox-cli`. Reclassify as CORE. No action needed.
**How**: Update the classification audit doc.
**Effort**: XS (doc correction)
**Risk**: None.

---

### vox-integration-tests
**Purpose**: Integration test crate — test binaries live in `tests/` (37+ test files covering A2A, LSP, orchestrator, compiler, CLI, speech, workflows, etc.).
**Why dead**: The `src/lib.rs` is 4 lines (no public API). The test files themselves are substantial and cover real scenarios. The crate has no consumers because it IS the consumer — it imports workspace crates as dev-dependencies.
**LOC / size**: 4 lines lib.rs; 37+ test files in `tests/` (substantial, not measured separately).
**Concept fit today**: Yes — the CI workflow `ci.yml` references it. Most test files are marked `*` (likely skipped in fast CI) but the harness exists and is correct.
**Recommendation**: **KEEP-FROZEN** — do not delete; this is real test infrastructure. It's "dead" only in the sense that nothing depends on it as a library. Its value is the test binaries. The `*`-marked tests should be audited for which ones are gated in CI vs. fully skipped.
**How**: No code change. Document in `README.md` which tests are CI-gated vs. local-only. Consider adding a `cargo test -p vox-integration-tests` step to `ci.yml` for the non-starred tests.
**Effort**: S (1h to audit and document which tests run in CI)
**Risk**: Low. Running a subset of integration tests in CI may surface failures currently hidden.

---

### vox-test-harness
**Purpose**: Shared test infrastructure for compiler and tooling pipelines — spans, HIR builders, assertions, pipeline helpers, port picker.
**Why dead**: Classification says zero consumers but `vox-integration-tests/Cargo.toml` lists it as a dev-dependency (`vox-test-harness = { path = "../vox-test-harness" }`). It is a test-only dep.
**LOC / size**: 421 lines, 8 files.
**Concept fit today**: Yes — shared test helpers reduce duplication across compiler tests.
**Recommendation**: **KEEP-FROZEN** — it is consumed by `vox-integration-tests` as a dev-dep. The audit's "zero consumers" count only production deps. Leave it; it's doing its job.
**How**: No change. Optionally add it as a dev-dep in crates that define their own `dummy_span()` or `minimal_module()` helpers.
**Effort**: XS
**Risk**: None.

---

### vox-browser
**Purpose**: Narrow Chromium CDP automation backend via `chromiumoxide` — session API, global engine, auto-detection of Chrome binary.
**Why dead**: `vox-plugin-browser` is the canonical implementation and the plugin-host provides CDP access at runtime. This crate is the pre-extraction host-side abstraction left behind.
**LOC / size**: 417 lines, 2 files.
**Concept fit today**: No — `vox-plugin-browser` covers the same surface and is the correct plugin-first home. Having both is confusing and wastes a `chromiumoxide` dep at workspace-check time.
**Recommendation**: **DELETE**
**How**: Remove `crates/vox-browser/`. Confirm `vox-plugin-browser` has equivalent coverage for any functionality referenced in ADRs.
**Effort**: XS (15min)
**Risk**: Low. Zero consumers per audit. Verify `vox-plugin-browser` has the `BrowserEngine`/session API covered before deleting.

---

### vox-workflow-runtime
**Purpose**: Interpreted workflow runner — walks HIR workflow bodies, executes activity steps as no-ops or mesh hooks, journals runs to `vox-db`, backs `vox mens workflow run`.
**Why dead**: Optional dep in `vox-cli` behind the `workflow-runtime` feature flag. Zero unconditional consumers; the feature is not enabled in default builds.
**LOC / size**: 1,864 lines, db_tracker + workflow/ subdirectory — a real, non-trivial implementation.
**Concept fit today**: Yes — workflow execution is a planned feature. The implementation is the MVP for `vox mens workflow run`. It's "dead" only because the feature flag is not in the default build.
**Recommendation**: **KEEP-FROZEN** — the concept is valid and the implementation is non-trivial. Do not delete. Do not invest further until the workflow feature is prioritized. Document as intentionally inactive (MVP parked behind feature flag).
**How**: Add a `# STATUS: frozen — behind workflow-runtime feature flag` comment to the crate's `Cargo.toml`. No code changes.
**Effort**: XS (comment only)
**Risk**: None to freeze. Risk of activation: the crate depends on `vox-populi/transport` (optional) and `vox-compiler`; verify these still compile together before enabling the feature.

---

## MISPLACED Crates

### vox-oratio (MISPLACED)
**Purpose**: Vox STT — Candle Whisper STT and transcript refinement. Large crate (7,224 LOC, 30+ files).
**Current state**: Extraction is actively in progress. The `stt-candle` feature is marked DEPRECATED in the Cargo.toml comment; the plugin (`vox-plugin-oratio`) now owns the Candle Whisper backend. `vox-ml-cli` and `vox-orchestrator` still take optional deps on `vox-oratio` for the `compiler-rerank` feature.
**Recommendation**: **Finish extraction** — the plan is correct and underway. The remaining `vox-oratio` uses are the `compiler-rerank` feature (AST mapper / transcript reranking), which is lightweight and has no ML deps. Either: (a) move `compiler-rerank` logic into `vox-compiler` directly, or (b) keep `vox-oratio` as a slim non-ML reranking crate and delete all ML modules. Option (b) is lower risk.
**How**: Delete `src/backends/`, `src/acoustic_preprocess.rs`, `src/vad/`, `src/tiering.rs` and all other ML-only files once the `stt-candle` feature removal is complete. Keep `refine/`, `ast_mapper.rs`, `routing.rs`, `speech_normalize.rs` as the non-ML residue.
**Effort**: M (½ day)
**Risk**: Medium. Need to audit exactly which symbols `vox-ml-cli/oratio` feature and `vox-orchestrator/oratio-rerank` import before deleting modules.

---

### vox-audio-ingress (MISPLACED)
**Purpose**: Minimal HTTP ingress binary for Oratio — `/api/audio/status` and `/api/audio/transcribe` endpoints, multipart upload, WebSocket. 549-line `main.rs`.
**Current state**: Transcription dispatch was refactored (commit `260744ba9`) to go through `vox-plugin-oratio`. The binary still exists but the heavy STT dep is gone. Classification audit called it self-referential via `vox-oratio` dep — but after the refactor, that dep may now be minimal.
**Recommendation**: **DELETE** — fold this into `vox-plugin-oratio-mic` or remove entirely. A standalone HTTP ingress binary for audio is an operational concern that belongs in a plugin or a compose/deployment artifact, not a workspace crate. `vox-plugin-oratio-mic` is the canonical microphone capture surface.
**How**: Check if the HTTP ingress endpoint (`/api/audio/transcribe`) is used by any client code or deployment config. If yes, move the axum router into `vox-plugin-oratio-mic`. If no, delete outright.
**Effort**: S (1h — check deployment usage first)
**Risk**: Low-medium. If the HTTP endpoint is used by the Coolify deploy or a frontend, removing it breaks that integration. Verify before deleting.

---

### vox-ssg (MISPLACED)
**Purpose**: Static site generator — converts Vox modules with `routes:` declarations into static HTML shells for Vite SSR pre-rendering. 183 LOC, 1 file, only dep is `vox-compiler`.
**Current state**: Direct mandatory dep of `vox-cli`. Simple and self-contained.
**Recommendation**: **REWRITE-AS-PLUGIN** — SSG is a publishing/build-time concern, not a runtime CLI concern. With the interop plan (Phase 5 bidirectional React interop), SSG output becomes more important but should be plugin-dispatched. The 183-line implementation is the right scope for a plugin; move it to `vox-plugin-ssg`.
**How**: Create `crates/vox-plugin-ssg/`; implement plugin ABI; move `generate_static_site` behind dispatch. Remove the mandatory dep from `vox-cli` and replace with a plugin-host call.
**Effort**: S (1h — mostly ABI plumbing; the logic is tiny)
**Risk**: Low. The current implementation is 183 lines with no unsafe code. Risk is that `vox-cli` currently calls it unconditionally; need to find the callsite and replace with plugin dispatch.

---

## 4. Catalog Ghost Entries

`execution-api` and `stub-check` were pre-declared plugin IDs in `catalog.toml` with no corresponding crates. Per the comment now in `catalog.toml`, these were already removed as of 2026-05-08. No further action needed.

---

## 5. Recommended Execution Order

### Wave 1 — Quick Wins (XS deletions, ~2h total)
These have zero risk and remove dead weight immediately:

1. Delete `vox-schola` — 79 lines, standalone binary, fully replaced
2. Delete `vox-scientia-core` + `vox-scientia-social` — 51 lines combined, pure facades with no consumers
3. Delete `vox-mcp-meta` — 62 lines, pass-through wrapper
4. Delete `vox-spool` — 270 lines, no consumer, replaced by orchestrator internals
5. Delete `vox-tools` — 580 lines, superseded by plugin-host + capability-registry
6. Delete `vox-browser` — 417 lines, superseded by vox-plugin-browser
7. Freeze `vox-workflow-runtime` — add status comment to Cargo.toml

**After wave 1:** run `cargo check --workspace` to confirm clean compile.

### Wave 2 — Wiring and small migrations (S tasks, ~4h total)
8. Wire `vox-exec-grammar` into `vox-container` — concept is complete, just needs a caller
9. Wire `vox-mcp-registry` into `vox-orchestrator` — replace any hard-coded tool name tables
10. Migrate `vox-socrates-policy` constants into `vox-orchestrator-types`, then delete the crate
11. Audit `vox-scientia-ingest` usage in `vox-cli`; gate behind feature or move to `vox-publisher`, then delete

### Wave 3 — Extractions (M tasks, 1–2 days total)
12. Extract `vox-webhook` → `vox-plugin-webhook`
13. Extract `vox-grammar-export` → `vox-plugin-grammar-export`
14. Finish `vox-oratio` extraction (slim to non-ML residue)
15. Delete `vox-audio-ingress` after confirming no active HTTP clients
16. Rewrite `vox-ssg` → `vox-plugin-ssg`

### Wave 4 — Classification corrections (XS doc edits)
17. Update `crate-classification-2026-05-08.md`: reclassify `vox-search`, `vox-scientia-ingest`, `vox-doc-inventory` from DEAD → CORE.

---

## 6. Open Questions for the User

1. **vox-scientia-ingest in vox-cli**: What does `vox-cli` actually call from `vox-scientia-ingest`? This should be a quick grep, but the answer determines whether the dep can be feature-gated or must be kept. If it's used for a `vox ci` command, it may warrant a dedicated `scholarly` feature flag rather than unconditional compile.

2. **vox-workflow-runtime activation timeline**: Is the `workflow-runtime` feature expected to be enabled in the next release cycle? If yes, the KEEP-FROZEN recommendation should be upgraded to WIRE-UP (enable the feature in `vox-cli`'s default feature set and add CI coverage). If no near-term plan, freeze is correct.

3. **vox-audio-ingress deployment**: Is the `/api/audio/transcribe` HTTP endpoint used by any frontend, Coolify deployment, or external client? If yes, the DELETE recommendation becomes a MIGRATE-TO-PLUGIN. If no external caller exists, delete is safe.

4. **vox-scientia resurrection**: The scientia social adapters (Reddit, YouTube, Twitter, etc.) are feature-flagged in `vox-publisher` but the higher-level scientia crates are facades nobody uses. Is scientia a concept the user wants to keep investing in (→ proper plugin), or is it retired (→ remove the feature flags from `vox-publisher` too)?

5. **vox-search default features**: `vox-search`'s `default` feature enables `tantivy-lexical`, `qdrant-vector`, and `tavily` — three heavy/network deps — for all CLI builds. Was this intentional, or should the default be `[]` with opt-in features?
