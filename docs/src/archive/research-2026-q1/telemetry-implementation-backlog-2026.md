---
title: "Telemetry implementation backlog 2026"
description: "Executable, codebase-wide checklist for telemetry unification; grouped by phase and primary path."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-04-02"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Telemetry implementation backlog 2026

Use this as the **single execution checklist** for telemetry unification. Check items off in PRs; link PRs from commit messages or issue trackers as your team prefers.

**SSOT hierarchy:** [telemetry-trust-ssot](telemetry-trust-ssot.md) > this backlog > crate code.

---

## Phase 0 — SSOT and documentation convergence

### 0.A Contributor entry points

- [x] `AGENTS.md` — add bullet linking [telemetry-trust-ssot](telemetry-trust-ssot.md), [telemetry-implementation-blueprint-2026](telemetry-implementation-blueprint-2026.md), and research doc.
- [x] `docs/src/contributors/contributor-hub.md` — optional one-line pointer to telemetry SSOT if hub lists architecture SSOTs.
- [x] `docs/src/contributors/documentation-governance.md` — add telemetry doc family to maintenance table if required by project rules.

### 0.B Environment variables SSOT

- [x] `docs/src/reference/env-vars.md` — add `VOX_BENCHMARK_TELEMETRY` row (CLI → `research_metrics` benchmark_event).
- [x] `docs/src/reference/env-vars.md` — add `VOX_SYNTAX_K_TELEMETRY` row (fallback to benchmark flag per `benchmark_telemetry.rs`).
- [x] `docs/src/reference/env-vars.md` — cross-link [telemetry-metric-contract](../reference/telemetry-metric-contract.md) from new rows.
- [x] `docs/src/reference/env-vars.md` — verify `VOX_MESH_CODEX_TELEMETRY`, `VOX_MCP_LLM_COST_EVENTS`, context lifecycle vars cross-link [telemetry-trust-ssot](telemetry-trust-ssot.md).
- [x] `docs/src/reference/orchestration-unified.md` — dedupe or point to env-vars for benchmark/syntax-k if duplicated.
- [x] `docs/src/reference/mens-training.md` — ensure benchmark/syntax-k pointers remain consistent with env-vars.

### 0.C Core reference docs

- [x] `docs/src/reference/telemetry-metric-contract.md` — add “Related SSOT” block: trust-ssot, taxonomy, retention-sensitivity, client-disclosure.
- [x] `docs/src/reference/cli.md` — add pointer to telemetry-trust-ssot next to cost-event and mesh telemetry sections.
- [x] `docs/src/architecture/completion-policy-ssot.md` — add pointer to telemetry-retention-sensitivity-ssot for `ci_completion_*` classification.
- [x] `docs/src/architecture/voxdb-connect-policy.md` — note optional DB and impact on telemetry availability (no writes when DB absent).

### 0.D Book index and architecture map

- [x] `docs/src/SUMMARY.md` — link telemetry-trust-ssot, taxonomy, retention-sensitivity, client-disclosure, blueprint, backlog.
- [x] `docs/src/architecture/architecture-index.md` — list new SSOTs under **Current architecture and SSOT**.
- [x] `docs/src/architecture/research-index.md` — link blueprint + backlog under planning or research follow-ups.
- [x] `docs/src/architecture/telemetry-unification-research-findings-2026.md` — add “Implementation” see-also to new SSOT pages.

### 0.E VS Code packaging

- [x] `apps/editor/vox-vscode/README.md` — link [telemetry-client-disclosure-ssot](telemetry-client-disclosure-ssot.md) and trust-ssot.

archived_date: 2026-04-18
---

## Phase 1 — Taxonomy and contract registry

### 1.A contracts/index.yaml

- [x] Register each telemetry JSON Schema with stable `id` and `enforced_by` where applicable.
- [x] Add index entries for `contracts/telemetry/completion-*.v1.schema.json` if any row missing.
- [x] Add index entry for `contracts/orchestration/context-lifecycle-telemetry.schema.json` with description “orchestrator tracing fields”.
- [x] Add index pattern for future `contracts/telemetry/usage-event-*.schema.json` (placeholder row or ADR note).

### 1.B Taxonomy document parity

- [x] `docs/src/architecture/telemetry-taxonomy-contracts-ssot.md` — fill `owner_crate` column for each shipped `METRIC_TYPE_*`.
- [x] Map `contracts/eval/syntax-k-event.schema.json` to `syntax_k_event` in taxonomy table.
- [x] Map `contracts/communication/interruption-decision.schema.json` to attention/interruption plane.

### 1.C Schema drift CI

- [x] `crates/vox-cli/src/commands/ci/run_body_helpers/data_ssot_guards.rs` — extend guards so every `METRIC_TYPE_*` constant is mentioned in telemetry-metric-contract or taxonomy SSOT.
- [x] `crates/vox-cli/src/commands/ci/command_compliance/mod.rs` — ensure completion telemetry schemas stay verified when index changes.

---

## Phase 2 — Retention and sensitivity

### 2.A retention-policy.yaml

- [x] Add `ci_completion_run` with `kind`, `days`/`ms_days`, `time_column` (e.g. `finished_at`), rationale in YAML.
- [x] Add `ci_completion_finding` retention row if distinct TTL desired (or cascade via run FK).
- [x] Add `ci_completion_detector_snapshot` retention row if distinct TTL desired (or cascade via run FK).
- [x] Add `ci_completion_suppression` retention row (may be `keep_forever` or long TTL; document rationale).
- [x] Document conflict resolution if completion rows must be `manual` for compliance.

### 2.B Documentation

- [x] `docs/src/architecture/telemetry-retention-sensitivity-ssot.md` — replace “gap” language with actual TTLs once YAML updated.
- [x] `docs/src/reference/cli.md` — `vox db prune-plan` help text cross-link retention SSOT if not already.

### 2.C Tests

- [x] `crates/vox-cli` tests — prune-plan includes new tables (integration or unit on YAML parse).
- [x] `crates/vox-db` — verify prune SQL exists for new completion tables if added to policy.

archived_date: 2026-04-18
---

## Phase 3 — Producer audit and code alignment (`vox-db`)

- [x] `crates/vox-db/src/research_metrics_contract.rs` — document each `METRIC_TYPE_*` in module rustdoc with sensitivity class.
- [x] `crates/vox-db/src/benchmark_telemetry.rs` — ensure metadata size respects `RESEARCH_METRICS_METADATA_JSON_MAX_BYTES`.
- [x] `crates/vox-db/src/syntax_k_telemetry.rs` — align metadata with `contracts/eval/syntax-k-event.schema.json`.
- [x] `crates/vox-db/src/socrates_telemetry.rs` — classify `socrates_surface` vs `memory_hybrid_fusion` in comments.
- [x] `crates/vox-db/src/questioning_telemetry.rs` — classify questioning rows (S1/S2) in rustdoc.
- [x] `crates/vox-db/src/populi_control_telemetry.rs` — document mesh token is never stored in metadata.
- [x] `crates/vox-db/src/workflow_journal.rs` — classify workflow journal entries vs usage telemetry.
- [x] `crates/vox-db/src/store/ops_codex/codex_metrics_packages.rs` — document `append_research_metric` as canonical write path.
- [x] `crates/vox-db/src/store/ops_completion.rs` — add rustdoc: workspace-adjacent data class.
- [x] `crates/vox-db/src/schema/domains/ci_completion.rs` — column-level comments for path/fingerprint sensitivity.

---

## Phase 3 — Producer audit (`vox-cli`)

- [x] `crates/vox-cli/src/benchmark_telemetry.rs` — document env precedence in file header; link env-vars SSOT.
- [x] `crates/vox-cli/src/commands/ci/build_timings.rs` — confirm writes only when opt-in; document.
- [x] `crates/vox-cli/src/commands/ci/completion_quality.rs` — document ingest path and data class.
- [x] `crates/vox-cli/src/commands/mens/watch_telemetry.rs` — link `telemetry_schema.rs` keys to data-ssot-guards contract.
- [x] `crates/vox-cli/src/commands/db_research/reliability.rs` — operator UX: warn when dumping S2 fields.
- [x] `crates/vox-cli/src/commands/db_cli/core_subcommands.rs` — help text references trust-ssot for research_metrics.
- [x] `crates/vox-cli/src/codex_cmd.rs` — Socrates aggregate JSON: classify as operator diagnostic.

archived_date: 2026-04-18
---

## Phase 3 — Producer audit (`vox-mcp`)

- [x] `crates/vox-orchestrator/src/mcp_tools/llm_bridge/infer.rs` — document `VOX_MCP_LLM_COST_EVENTS` defaulting when DB absent.
- [x] `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs` — classify `record_attention_event` persistence path (not usage telemetry unless explicitly scoped).
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs` — context lifecycle policy side effects documented.
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/benchmark_tools.rs` — tool descriptions reference trust-ssot.
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/chat_socrates_meta.rs` — `record_socrates_surface_event` classification.
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/repo_catalog_tools.rs` — benchmark record path gated and documented.
- [x] `crates/vox-orchestrator/src/mcp_tools/dei_tools/orchestrator_snapshot.rs` — mesh snapshot telemetry classification.
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/questioning_tools.rs` — attention events vs questioning DB tables.
- [x] `crates/vox-orchestrator/src/mcp_tools/a2a.rs` — attention debit events documented.
- [x] `crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs` — ensure `prepare_mcp_tool_args_for_storage` applied on all persistence paths.
- [x] `crates/vox-mcp/tests/tool_dispatch_tests.rs` — add cases for any new redaction rules.

---

## Phase 3 — Producer audit (`vox-orchestrator`)

- [x] `crates/vox-orchestrator/src/context_lifecycle.rs` — link `context-lifecycle-telemetry.schema.json` in module docs.
- [x] `crates/vox-orchestrator/src/mesh_federation_poll.rs` — document `mesh_exec_lease_reconcile` telemetry gate.
- [x] `crates/vox-orchestrator/src/config/orchestrator_fields.rs` — env flags for lifecycle shadow/enforce cross-link env-vars.
- [x] `crates/vox-orchestrator/src/attention/interruption_policy.rs` — document serialization for interruption-decision contract.
- [x] `crates/vox-orchestrator/tests/context_lifecycle_telemetry_fixtures.rs` — keep fixtures synced with schema changes.

archived_date: 2026-04-18
---

## Phase 3 — Producer audit (`vox-populi` / Mens)

- [x] `crates/vox-populi/src/mens/tensor/telemetry_schema.rs` — each key documented with S0/S1.
- [x] `crates/vox-populi/src/mens/tensor/candle_qlora_train/db_thread.rs` — training events vs product telemetry.
- [x] `crates/vox-populi/src/transport/handlers.rs` — `privacy_class` behavior documented.

---

## Phase 3 — Producer audit (`vox-ludus`)

- [x] `crates/vox-ludus/src/mcp_privacy.rs` — reference generalized redaction policy when introduced.
- [x] `crates/vox-ludus/src/config_gate.rs` — `VOX_LUDUS_MCP_TOOL_ARGS` values documented in env-vars.

archived_date: 2026-04-18
---

## Phase 3 — Producer audit (`vox-compiler` / Syntax-K)

- [x] `crates/vox-compiler/src/syntax_k.rs` — telemetry hook calls documented; link syntax-k-event schema.

---

## Phase 3 — Producer audit (`vox-orchestrator` / other)

- [x] `crates/vox-dei/src/route_telemetry.rs` — classify metrics; link taxonomy SSOT.
- [x] `crates/vox-dei/src/lib.rs` — any exports documented.

archived_date: 2026-04-18
---

## Phase 3 — Content-bearing stores (classification only, no merge into usage telemetry)

- [x] `crates/vox-db/src/codex_chat.rs` — rustdoc: S3 content plane.
- [x] `crates/vox-db/src/store/ops_mcp_diagnostics.rs` — transcript inserts S3.
- [x] `crates/vox-db/src/schema/domains/agents.rs` — table groups: telemetry vs content (comment block).

---

## Phase 4 — Client disclosure and UX

- [x] `apps/editor/vox-vscode/webview-ui/src/index.tsx` — evaluate tab `id="telemetry"` rename vs display label-only change; document breaking change if any.
- [x] `apps/editor/vox-vscode/webview-ui/src/components/Dashboard.tsx` — user-visible strings reviewed against client-disclosure SSOT.
- [x] `apps/editor/vox-vscode/package.json` — contribution settings descriptions reference trust SSOT where debug flags exposed.
- [x] `docs/src/reference/vscode-mcp-compat.md` — cross-link telemetry-client-disclosure-ssot.

archived_date: 2026-04-18
---

## Phase 5 — Operations catalog and CLI registry

- [x] `contracts/operations/catalog.v1.yaml` — ensure every telemetry-related `vox ci` / `vox db` op used in guards is catalogued.
- [x] `contracts/cli/command-registry.yaml` — regenerate after any new CLI surface (`vox ci capability-sync --write` workflow per project rules).
- [x] `docs/src/architecture/operations-catalog-ssot.md` — pointer to telemetry backlog if present.

---

## Phase 6 — CI workflow

- [x] `.github/workflows/ci.yml` — confirm `data-ssot-guards` / `ssot-drift` runs on PRs; add step if missing.
- [x] Document in `docs/src/ci/command-compliance-ssot.md` any new mandatory gate.

archived_date: 2026-04-18
---

## Phase 7 — Optional central sink (future)

- [x] ADR: remote telemetry upload, data residency, opt-in UX — [ADR 023](../adr/023-optional-telemetry-remote-upload.md).
- [x] `crates/vox-clavis/src/lib.rs` — `SecretId` for upload URL + bearer token (`VoxTelemetryUploadUrl`, `VoxTelemetryUploadToken`); CLI uses `resolve_secret` only.
- [x] Queue module: `crates/vox-cli/src/telemetry_spool.rs` — local spool, export, enqueue, delete-after-ack on HTTP 2xx.
- [x] Rate limit and payload signer specification in SSOT — [telemetry-remote-sink-spec](telemetry-remote-sink-spec.md).
- [x] CLI: `vox telemetry status|export|enqueue|upload` (catalog + generated registries).

---

## Phase 8 — CHANGELOG and release discipline

- [x] `CHANGELOG.md` — process note: telemetry-affecting changes use the **Telemetry** subsection under [Unreleased].
- [x] Maintainer pointer: [command-compliance SSOT](../ci/command-compliance-ssot.md) — verify telemetry SSOT links when touching metric contracts or upload behavior.

archived_date: 2026-04-18
---

## Completion criteria (definition of done)

- [x] All Phase 0–4 items checked for minimal viable trust convergence.
- [x] Phase 5–6 complete before any default remote upload ships (no default upload in product; `vox telemetry upload` remains explicit).
- [x] Phase 7 technical guardrails documented in ADR 023; organization legal/security sign-off for production ingest remains operator responsibility (called out in ADR).


