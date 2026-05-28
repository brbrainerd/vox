---
title: "Codex vNext — schema domains"
description: "Official documentation for Codex vNext — schema domains for the Vox language. Detailed technical reference, architecture guides, and impl"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Codex vNext — schema domains

This document is the **design SSOT** for how relational tables are grouped after the greenfield cut. Implementation lives in [`crates/vox-db/src/schema/`](../../../crates/vox-db/src/schema/mod.rs) as ordered **domain fragments** concatenated into one baseline DDL; the database records a `schema_version` row equal to [`BASELINE_VERSION`](../../../crates/vox-db/src/schema/manifest.rs) (see [`contracts/db/baseline-version-policy.yaml`](../../../contracts/db/baseline-version-policy.yaml)). Historical docs referred to fragment labels `v1`…`v17`; the active layout is domain-scoped under `schema/domains/`. Notable areas: chat and search ingest, processing/audit, research sessions / conversation graph.

**Naming:** **Codex** = public platform DB. **Arca** = internal schema/CAS owner (`CodeStore`). Engine = **Turso** only.

## Baseline domains (in baseline / retained)

| Domain | Tables (representative) | Notes |
|--------|-------------------------|--------|
| **core_cas** | `objects`, `names`, `causal`, `metadata` | Content-addressed blobs and bindings |
| **packages** | `packages`, `package_deps` | Registry + yank flag (fragment `v4`) |
| **workflows** | `execution_log`, `scheduled`, `components` | Execution + scheduling hooks |
| **context_memory** | `memories`, `session_turns`, `builder_sessions`, `agent_sessions`, `agent_events`, `a2a_messages`, `cost_records`, `agent_metrics` | Agent/session/cost telemetry |
| **skills** | `skill_manifests` | Published skill rows + CAS-backed content |
| **docs_knowledge** | `knowledge_nodes`, `knowledge_edges`, `snippets` | Docs/RAG graph |
| **embeddings** | `embeddings` | Vector metadata |
| **ops_training** | `llm_interactions`, `llm_feedback`, `research_metrics`, `eval_runs`, `typed_stream_events`, `populi_reviews` | RLHF / eval / streams |
| **users_marketplace** | `users`, `user_preferences`, `behavior_events`, `learned_patterns`, `artifacts`, `artifact_reviews`, `agents` | User + marketplace (trim if product scope shrinks) |
| **user_chat** (fragment `v11`) | `conversations`, `conversation_messages` | Human-facing chat threads; optional `user_id` → `users`; complements `a2a_messages` |
| **tool_calls** (`v12`) | `conversation_tool_calls` | Tool invocations tied to assistant `conversation_messages` (`ordinal` per turn) |
| **usage_governance** (`v13`) | `usage_limit_definitions`, `usage_counter_snapshots` | Policy + counted usage per metric / scope / window |
| **topics** (`v14`) | `topics`, `conversation_topics`, `conversation_message_topics` | Thread + per-message tagging |
| **routing_calibration** (`v10`) | `agent_reliability` | Socrates-style routing scores (ADR 005) |
| **search_ingest** (`v15`) | `search_documents`, `search_document_chunks`, `search_indexing_jobs` | Corpus rows + chunk text + ingest job queue (retrieval fusion stays in `vox-db`) |
| **codex_reactivity** (`v8`) | `codex_schema_lineage`, `codex_change_log`, `codex_subscriptions`, `codex_query_snapshots`, `codex_projection_versions` | Convex-style hooks |
| **processing_audit** (`v16`) | `processing_runs`, `processing_run_steps`, `audit_log` | Durable run tracking + audit trail |
| **conversation_graph** (`v17`) | `research_sessions`, `conversation_versions`, `conversation_edges`, `topic_evolution_events` | Research session + lineage graph |

## Import / drop policy (fresh release)

| Area | Policy |
|------|--------|
| **Retain in vNext** | All domains needed for compiler PM, skills, workflows, context, Codex reactivity |
| **Import from legacy** | Rows mapped by explicit Rust importers in `vox_db::codex_legacy` (see crate docs) |
| **Defer / drop from default baseline** | Gamification (`gamify_*`) if no release owner; experimental builder-only tables without callers — re-add via migration when owned |

## Adding schema slices (baseline DDL)

- New DDL belongs in a **domain module** under `crates/vox-db/src/schema/domains/` and a matching entry in [`SCHEMA_FRAGMENTS`](../../../crates/vox-db/src/schema/manifest.rs) (append-only order). Bump **`BASELINE_VERSION`** only with a coordinated migration story (policy: `contracts/db/baseline-version-policy.yaml`).
- **Digest:** `vox_db::schema::schema_baseline_digest_hex` hashes the concatenated baseline SQL; HTTP `/ready` and operators compare **required tables** + digest (see `vox_db::codex_schema`, `vox-codex-api`).
- **v1–v7:** Historical slice layout; **v7** remains an empty fragment (no-op).
- **v8:** Codex reactivity + schema lineage (append-only).
- **v9+:** Domain-scoped changes; prefer small fragment files over monolithic SQL.
- **v11–v15:** Chat, tool calls, usage governance, topics, search ingest; search row counts on **`GET /api/search/status`** (`vox-codex-api`).
- **v16–v17:** Processing/audit and conversation-graph tables; accessors on `CodeStore` / `VoxDb` (`upsert_research_session`, `append_conversation_version`, …).

## Reactive layer (Convex-like, staged)

- **Tables:** `codex_change_log`, `codex_subscriptions`, `codex_query_snapshots`, `codex_projection_versions` (fragment `v8`).
- **Writes:** Mutations append to `codex_change_log` in the same transaction as domain rows (via `CodeStore::append_codex_change` / `VoxDb::append_codex_change`).
- **Delivery:** SSE or WebSocket endpoints (future `vox-codex-api` or generated app) poll or tail `codex_change_log` by `topic` and match `codex_subscriptions`.
- **Public HTTP sketch {** `GET /api/codex/subscribe/:topic`, `POST /api/codex/mutate/:name`, `GET /api/codex/query/:name` — implement behind one auth/tenant boundary.
- **Language IR hooks:** `.vox` query chains can now carry plan capabilities (`.live("topic")`, `.using("fts|vector|hybrid")`, `.sync()`, `.scope("populi|orchestrator")`) so compiler/codegen keep reactivity, retrieval, replica-sync, and orchestration hints together in one DB plan.

See [ADR 004: Codex over Arca over Turso](../adr/004-codex-arca-turso-ssot.md).


