---
title: "Orphan surface inventory"
description: "Official documentation for Orphan surface inventory for the Vox language. Detailed technical reference, architecture guides, and implemen"
category: "Language Reference"
last_updated: "2026-03-27"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Orphan surface inventory

Classification for code and docs that do not match the **minimal shipped `vox` CLI** or workspace membership. Goal { no ambiguous SSOT. See [forward migration charter](forward-migration-charter.md) (forward-only; no restore-based workflows).

## Policy buckets

| Bucket | Action |
|--------|--------|
| **keep** | Wired in default build; maintain |
| **port** | Needed for roadmap; rewire to `vox_db::VoxDb` / workspace members |
| **archive** | Historical value only; move to `docs/src/archive/` or mark “not built” in header |
| **delete** | Duplicate or superseded; remove when safe |

## Automation / CI SSOT

- Prefer **`vox ci …`** for registry-backed checks over one-off shell copies where a subcommand exists — [runner contract](../ci/runner-contract.md), [command compliance](../reference/command-compliance.md).
- **`VOX_*` / Turso env naming:** [Environment variables (SSOT)](../reference/env-vars.md).

## Inventory (surfaces)

| Surface | Location | Owner | Severity | Decision | Milestone | Validated | Evidence | Rationale |
|---------|----------|-------|----------|----------|-----------|-----------|----------|-----------|
| Minimal `vox` CLI | `crates/vox-cli/src/main.rs`, `commands/mod.rs` | Maintainers | low | keep | ongoing | 2026-03-20 | `ref-cli.md` | SSOT for shipped commands |
| Extended CLI subtree | `crates/vox-cli/src/commands/**` (beyond `commands/mod.rs`) | Maintainers | high | port | TBD | 2026-03-21 | `cli-scope-policy.md` | Unwired until explicitly added to minimal binary; `vox-skills` is a workspace member; `vox-cli` optional feature **`ars`** pulls the dep when OpenClaw/skill modules are reattached |
| Canonical `vox db` helpers | `crates/vox-cli/src/commands/db.rs`, `db_research_impl.rs` | Maintainers | medium | keep | ongoing | 2026-03-21 | `commands/db.rs` | `commands::ops` tree removed (unwired; duplicated `vox_orchestrator`); DB helpers live under `commands::db` |
| `vox scientia` CLI facade | `crates/vox-cli/src/commands/scientia.rs` | Maintainers | low | keep | ongoing | 2026-03-21 | `ref-cli.md`, `orchestration-unified.md` | Research / capability-map aliases over `commands::db_cli` (same DB + `repository_id` resolution as `vox db`) |
| Unwired `vox_orchestrator` CLI sources (removed) | _(deleted)_ `commands/chat/`, `commands/ops/`, `commands/quaero/`, `ai/{agent,dei,hud,learn}.rs` | Maintainers | low | delete | 2026-03-21 | `check_vox_cli_no_vox_orchestrator.sh` | Daemon-only DeI: use `crate::dei_daemon` + external `vox-dei-d` |
| `vox-runtime` DB helper | `crates/vox-runtime/src/db.rs` | Maintainers | low | keep | ongoing | 2026-03-25 | feature `database` | Uses `DbConfig::resolve_standalone` / `VOX_DB_*` (see crate rustdoc); parity with `vox-db` facade |
| `vox-mcp`, `vox-git` | workspace members | Maintainers | low | keep | ongoing | 2026-03-20 | `ci.yml` smoke | Core agent/tooling |
| Workspace excludes | root `Cargo.toml` `exclude` | Maintainers | medium | keep | ongoing | 2026-04-01 | `Cargo.toml` | **`vox-py`** remains excluded; **`vox-orchestrator`** is a normal workspace member (minimal `lib.rs` only). Do not add **`vox-orchestrator`** as a **`vox-cli`** dependency; orchestration SSOT is **`vox-orchestrator`** + **`build_repo_scoped_orchestrator`** (ADR 022). **`vox-dei-d`** stays the external DeI RPC process |
| Plans under `.cursor/plans/` | various | Maintainers | low | archive | ongoing | 2026-03-20 | — | May reference removed crates; not SSOT |
| Docs: full ecosystem | `how-to-cli-ecosystem.md` | Maintainers | medium | keep | ongoing | 2026-03-20 | `ref-cli.md` | Narrative may exceed minimal CLI |

## Deduplication wave classification (2026-03)

| Cluster | Primary locations | Classification | Canonical SSOT | Action |
|---------|-------------------|----------------|----------------|--------|
| bounded fs helper surface | `crates/**/bounded_fs.rs`, `crates/vox-bounded-fs/src/lib.rs` | merge | `vox-bounded-fs` | Remove per-crate wrappers where possible; direct crate usage |
| orchestrator construction path | `crates/vox-cli/src/commands/dei.rs`, `crates/vox-orchestrator/src/mcp_tools/server/lifecycle.rs` | merge | [`build_repo_scoped_orchestrator`](../../../crates/vox-orchestrator/src/bootstrap.rs) (ADR 022) | **Done:** shared factory + `bootstrap_build_parity` + `orchestrator_bootstrap_surface_parity`; **trust relax × grounding:** `trust_relax_allows_completion_under_grounding_enforce_when_agent_reliable`, `completion_grounding_enforce_requeues_when_trust_relax_disabled_even_if_reliable` (`orch_smoke` in `orchestrator/tests.rs`); keep new embedders on the factory only |
| compiler frontend entry path | `crates/vox-cli/src/commands/build.rs`, `crates/vox-cli/src/commands/check.rs`, `crates/vox-cli/src/pipeline.rs` | merge | `vox-cli` pipeline frontend | Route build/check/adjacent callers through one frontend pipeline |
| std/openclaw builtin mapping | `crates/vox-compiler/src/builtin_registry.rs`, `crates/vox-compiler/src/typeck/checker/expr_field.rs`, `crates/vox-compiler/src/codegen_rust/emit/stmt_expr.rs` | merge | data-driven builtin registry | Generate/derive type + codegen/runtime mapping from one table |
| rust interop support tiers | `contracts/rust/ecosystem-support.yaml`, `crates/vox-compiler/src/rust_interop_support.rs`, `docs/src/architecture/rust-ecosystem-support-ssot.md` | merge | contract YAML (+ generated Rust) | Keep contract machine-SSOT, generate classifier |
| db baseline vs legacy/cutover chain | `crates/vox-db/src/codex_legacy.rs`, `legacy_import_extras.rs`, `legacy/mod.rs`, `schema/manifest.rs` | legacy | baseline schema manifest/spec | Fence migration-only paths under explicit legacy namespace and age-out policy |
| mcp registry bootstrap inversion | `scripts/extract_mcp_tool_registry.py`, `contracts/mcp/tool-registry.canonical.yaml`, `crates/vox-mcp-registry/build.rs` | legacy | canonical YAML | Mark extract script as migration-only legacy pathway |
| duplicate non-normative mcp reference table | `docs/mcp-tool-reference.md` | delete/legacy | `docs/src/reference/mcp-tool-registry-contract.md` + canonical YAML | Replace with redirect to normative source |
| redirect stub docs (`ref/*`) | `docs/src/ref/*.md` | keep (alias) | `docs/src/reference/*` | Keep lightweight redirects; no duplicated normative content |

## Workspace crate index (CI guard)

`scripts/check_docs_ssot.sh` (or `scripts/check_docs_ssot.ps1` on Windows) requires every `crates/*/Cargo.toml` package name to appear **exactly once** between the markers below (one crate per line).
Note: `vox-ars` and `vox-gamify` are retired aliases/namespaces (now `vox-skills` and `vox-ludus`).

<!-- workspace-crates-start -->
vox-audio-ingress
vox-bootstrap
vox-bounded-fs
vox-browser
vox-build-meta
vox-capability-registry
vox-checksum-manifest
vox-clavis
vox-cli
vox-codex-api
vox-compiler
vox-config
vox-constrained-gen
vox-container
vox-corpus
vox-crypto
vox-db
vox-dei
vox-orchestrator
vox-doc-inventory
vox-doc-pipeline
vox-eval
vox-forge
vox-git
vox-grammar-export
vox-identity
vox-install-policy
vox-integration-tests
vox-jsonschema-util
vox-lsp
vox-ludus
vox-mcp-meta
vox-mcp-registry
vox-openai-sse
vox-openai-wire
vox-oratio
vox-pm
vox-populi
vox-primitives
vox-project-scaffold
vox-protocol
vox-publisher
vox-repository
vox-reqwest-defaults
vox-runtime
vox-scaling-policy
vox-schola
vox-scientia-api
vox-scientia-core
vox-scientia-ingest
vox-scientia-runtime
vox-search
vox-scientia-social
vox-skills
vox-socrates-policy
vox-ssg
vox-tensor
vox-test-harness
vox-toestub
vox-tools
vox-webhook
vox-workflow-runtime
workspace-hack
<!-- workspace-crates-end -->

## Review cadence

Re-run classification when adding a workspace member or a new `vox` subcommand.


