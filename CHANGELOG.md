---
title: "Changelog"
description: "All notable changes to the Vox project are documented here."
category: "ci"
status: "current"
last_updated: "2026-05-26"
training_eligible: true
training_rationale: "Maintains a history of project changes, useful for understanding evolution and bug fixes."
---
# Changelog

All notable changes to the Vox project are documented here.

## [Unreleased]

(Empty — v0.6.0 just tagged.  Next-release content lands here.)

## [0.6.0] - 2026-05-26

**Release theme:** Single-machine multi-agent with no data loss; `@endpoint` decorator surface retired; language hardening. All Phase 0 (foundations), Phase 1 (language spine), Phase 3 (VCS gossip), Hopper Hp-T1..T4 (unified task intake), and Telemetry unification Phases A–D deliverables from the mesh SSOT.

### Language surface changes

- **`@endpoint(kind: server|query|mutation)` retired** — the canonical bare-form decorators `@server` / `@query` / `@mutation` (introduced in Phase B, audit doc §11.2, 2026-05-23) are now the only endpoint declaration surface.  See [the v0.5 → v0.6 migration guide](docs/src/reference/migration-0.5-to-0.6.md).
- Other language surface changes (typed list subscript `Option[T]`, strict-Option `Json` API, `@json_as`, raw strings, intra-project imports) are detailed in the same migration guide.

### Removed

- **Lexer:** `AtEndpoint` token + `#[token("@endpoint")]` annotation deleted from `crates/vox-compiler/src/lexer/token.rs`.  Any remaining `@endpoint` text in user source now fails to lex.
- **Parser:** `parse_endpoint` function deleted from `crates/vox-compiler/src/parser/descent/decl/head.rs`; `Token::AtEndpoint` removed from the two decl-leading token sets and the dispatch arm in `descent/mod.rs`.
- **Detector severity escalation:** `retired/decorator-usage` raises `@endpoint(kind: …)` findings from `Severity::Warning` to `Severity::Error` per the CR-L6 retirement-guard parity gate (`vox-stdlib-gap-audit-2026-05-23.md §Phase H step 18`).

Deliberately not removed (still alive in production for the canonical bare-form decorators): `EndpointDecl` AST node, `EndpointKind` enum, `parse_query` / `parse_mutation` / `parse_server_endpoint`, all codegen handlers for `Decl::Endpoint`, and the `auth_endpoint` / `effect_net_decl` / `id_at_boundary` detectors.

### Added

- **Telemetry unification (Phases A–D):** `vox-telemetry` L1 facade crate with `TelemetryEvent` enum (`ResearchMetric`, `ModelCall`, `TaskRootSummary`, `BuildSummary`, `Error`), `TelemetryRecorder` trait, `record_event!` macro, `TRACE_CTX` task-local for distributed tracing, and `TaskTelemetryAggregator` for per-task token/cost rollup.
- **`VOX_TELEMETRY` master switch:** `off` / `0` / `false` silences all telemetry paths. Overrides all per-category env vars (`VOX_BENCHMARK_TELEMETRY`, `VOX_MCP_LLM_COST_EVENTS`). See [`env-vars.md`](docs/src/reference/env-vars.md).
- **`vox telemetry doctor`:** new subcommand that prints the effective `TelemetryConfig`, recorder registration status, and spool path. Exits 1 when master switch is off.
- **`ModelCallEvent`:** per-LLM-call telemetry with split prompt-cache tokens (`cache_read_input_tokens` / `cache_creation_input_tokens`), latency, cost, and trace context.
- **`TaskRootSummaryEvent`:** emitted on `complete_task`, `fail_task`, `doubt_task` with real token/cost totals populated by the `TaskTelemetryAggregator`.
- **`BuildSummaryEvent`:** emitted after `vox ci build-timings` with wall time and compiled-crate count.
- **`ErrorEvent`:** emitted at HTTP 429 (infer.rs), non-2xx HTTP (actor-runtime chat.rs), and circuit-breaker trips.
- **Trace propagation:** `RemoteTaskEnvelope` extended with `parent_task_id`, `caller_agent_id`, `trace_id`, `span_depth`; MCP dispatch wraps sub-agent calls in `TRACE_CTX::scope`; `SubAgentDispatchEvent` enriched with `parent_task_id` and `span_depth`.
- **Unified Task Hopper — Option A (Hp-T1..T4):** `InMemoryHopper` in `vox-orchestrator` implements the `HopperIntake` trait. Items progress through `Inbox → Assigned → Done / Overridden` states. HTTP intake endpoints surface the hopper at `/api/v2/hopper/{inbox,assigned,history,submit,reprioritize,assign,complete}`.
- **`PrioritySource` typed partial order (Hp-T3):** Canonical `PrioritySource` enum (`Developer(2) > Orchestrator(1) > LearningPolicy(0)`) in `vox-orchestrator-types`; dominance encoded via `Ord` so policy checks are a one-liner (`source.dominates(other)`). `ReprioritizationActor` is now a backward-compat type alias pointing at `PrioritySource`.
- **`DeveloperOverride` capability token (Hp-T4):** `reprioritize` requires a token minted by `DeveloperOverrideMint`. Each override is stamped with actor, reason, audit-id, and timestamp in `override_history`; after a developer override `priority_source` flips from `Orchestrator` to `Developer` and subsequent automated policy must not overwrite it without a new `Developer`-level cap.
- **Intra-project imports (Phase 1 — P1-T1):** `import "./foo.vox"` + `pub fn` resolution works under `--mode interp`. Typecheck and `--mode script` parity tracked in Phase 2.
- **JSON ergonomics (Phase 1 — P1-T2):** Strict-Option `Json` API — `get` / `at` / `pointer` + `as_str` / `as_int` / `has`; legacy `get_str` / `get_int` retired. Corpus pass rate 41/55.
- **Two-daemon lock-contention guard (Phase 3 — P3-T4):** Integration test `two_daemon_lock_contention` verifies that a second orchestrator daemon on the same repository correctly loses the exclusive lock and exits cleanly rather than corrupting shared state.
- **Effect-row enum extensions (P1-T6):** `GpuCompute` and `Mutate` variants added to the effect-row enum; supports MENS distributed-training capability tagging (Mn-T1/Mn-T2 prerequisite).
- **ACI envelope on by default — CR-L5:** `OrchestratorConfig::agentos_aci_envelope_enabled` defaults to `true` starting in v0.6. The `attach_aci_envelope()` path in `vox-orchestrator-mcp` adds the `aci` JSON block to tool responses (purely additive; gracefully skips non-JSON outputs). Feature flag `vox.orchestrator.agentos.aci_envelope.enabled` updated to match.
- **`vox audit aci-default` gate (CR-L5):** `CrlGate::L5AciDefault` in `vox-audit` reads the workspace version; passes when v0.5.x has `false` and v0.6+ has `true`. Prevents accidental rollback.

### Changed

- **Workspace version:** `0.5.0` → `0.6.0`. All published crates inherit the bumped version via `[workspace.package]`.
- **vox-db schema:** `BASELINE_VERSION = 67` (all P3-T1 schema migrations applied). Dashboard route convention (`/api/v2/<surface>` REST + `/v1/ws` topic-multiplex WS) in effect per SSOT §5.6.

## [0.5.0] - 2026-04-18

### Added
- **V0.5 Core SSOT**: Created `docs/src/architecture/v0.5-core-ssot.md` as the authoritative architecture summary for the v0.5 release.
- **Golden Path Validation**: Verified end-to-end build and dry-run deployment for `marquee_app` via Fly.io target.

### Fixed
- **CI/SSOT Stability**: 
    - Resolved 270+ documentation drift errors by archiving stale research into `docs/src/archive/research-2026-q1/`.
    - Bulk-fixed metadata (`training_eligible`, `archived_date`) for all archived research documents.
    - Cleaned up `contracts/documentation/canonical-map.v1.yaml` to match the current documentation tree.
    - Fixed orphan surface inventory check to handle missing files gracefully.
- **CLI/Constants**: Pruned stale document references from `crates/vox-cli/src/commands/ci/constants.rs`.

### Changed

- **vox-db / Mens training:** Removed `VoxDb::connect_default_with_training_fallback` and automatic `vox_training_telemetry.db` sidecar attach; training uses `connect_default` only. Legacy primary returns `LegacySchemaChain` until codex export/import cutover. `VOX_DB_TRAINING_TELEMETRY_SIDECAR` is unused (docs removed).
- **Operations SSOT / CI:** `contracts/operations/catalog.v1.yaml` is the only hand-edited source for first-party MCP + `vox-cli` + capability registry rows; `contracts/mcp/tool-registry.canonical.yaml`, `contracts/cli/command-registry.yaml` (vox-cli section), and `contracts/capability/capability-registry.yaml` are generated via `vox ci operations-sync --target {mcp,cli,capability,all} --write`. `vox ci operations-verify` now enforces strict file parity, `input_schemas.rs` coverage per tool, and `http-read-role-governance.yaml` vs catalog; the catalog carries a `capability:` block (runtime builtin maps + capability exemptions). Removed transitional `contracts/capability/curated-from-operations.generated.yaml`.
- **`vox-orchestrator` / daemon parity:** Populi mesh federation poll, **`VOX_ORCHESTRATOR_EVENT_LOG`**, and Codex clarification inbox drain live in **`mesh_federation_poll`**, **`orchestrator_event_log`**, and **`clarification_db_inbox_poll`**; **`vox-mcp`** delegates to them and **`vox-orchestrator-d`** runs the same background sidecars when config/DB apply. ADR 022 Phase B item 1 updated accordingly.
- **`vox-mcp` / daemon probe:** **`ServerState::probe_external_orchestrator_daemon_if_configured`** compares **`orch.ping`** **`repository_id`** to the MCP embed (**WARN** on mismatch; **`VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT`** → **ERROR**) and records **`orch_daemon_repo_id_aligned`** for optional RPC. **`VOX_MCP_ORCHESTRATOR_RPC_READS`** enables all aligned read pilots (or set per-tool **`TASK_STATUS` / `START` / `STATUS_TOOL`** flags). **`vox_orchestrator_start`** also calls **`orch.agent_ids`** (**`daemon_reported_agent_ids`**, mismatch note in **`honest_message`**). Shared **`orch_daemon_tcp_client_when_repo_aligned`**. **`OrchDaemonClient`** has **`orchestrator_status`**, **`task_status`**, **`spawn_agent_named`**, **`agent_ids`** helpers.
- **IPC-first write pilots:** `vox_protocol::orch_daemon_method` adds **`orch.submit_task`**, **`orch.complete_task`**, **`orch.fail_task`**, **`orch.cancel_task`**, **`orch.reorder_task`**, **`orch.drain_agent`**, **`orch.rebalance`**, **`orch.spawn_agent_ext`**, **`orch.retire_agent`**, **`orch.pause_agent`**, **`orch.resume_agent`**; daemon dispatch + client helpers implemented, MCP routes task/agent lifecycle through backend selectors with **`VOX_MCP_ORCHESTRATOR_RPC_WRITES`** (and per-slice overrides). Added contract schema [`contracts/orchestration/orch-daemon-rpc-methods.schema.json`](contracts/orchestration/orch-daemon-rpc-methods.schema.json) and protocol catalog entry `orchestrator-json-line-rpc`.
- **Workspace:** **`vox-dei`** is a normal workspace member (minimal **`lib.rs`**); **`vox-py`** remains the only root **`exclude`**. Docs and `vox-cli` comments updated; **`vox ci no-vox-dei-import`** unchanged.
- **`ServerState::new_test`**: uses **`OrchestratorConfig::for_testing()`** ( **`toestub_gate: false`**, tight limits) so MCP tests do not inherit production post-task behavior that can run nested **`cargo check --workspace`** for Rust manifests. The **`conflict_diff_success_payload_has_expected_keys`** contract test no longer calls **`complete_task`**; it builds an in-memory snapshot + **`record_conflict`** only (avoids multi-minute / LNK1104-prone paths). Production **`ServerState::new`** is unchanged.
- **Eval benchmark matrix**: `contracts/eval/benchmark-matrix.schema.json` now uses a fixed **`enum`** for each `benchmark_classes` entry (no arbitrary strings). `vox-cli` `eval_matrix` centralizes crate / feature / test-filter literals, runs **`ci command-compliance`** checks **in-process** (`command_compliance::run`) so `eval-matrix run` does not spawn `cargo run` (avoids Windows `vox.exe` replacement locks), and has **drift tests** (matrix JSON ∪ milestones ↔ Rust SSOT ↔ schema enum). GitLab **`vox-ci-guards`** runs **`ci line-endings`** and **`ci command-compliance`** to match the GitHub guard slice.
- **`vox fmt`**: wired to **`vox_compiler::fmt::try_format`** (parse → print → re-parse; atomic in-place write; **`--check`**). **Packaging Phase B:** **`vox install`** removed from the CLI (use **`vox add`** / **`vox lock`** / **`vox sync`** / **`vox pm`**); `command-registry.yaml` row dropped; **`vox ci command-compliance`** adds **`check_project_pm_commands_no_toolchain_lane`** (WP5) and **`check_operator_docs_no_legacy_vox_install_pm_nudge`** (WP4). **`cargo check -p vox-cli --all-features`:** stub **`serve/inference`** (Burn loader retired), **`workflow`** uses **`vox_db::LogExecutionParams`**, **`dei`** **`undo`/`redo`** use **`Orchestrator::init_db`** + **`sync_lock::rw_read(&orch.oplog)`**.
- **Examples**: `examples/actor.vox` and `examples/mcp_tool.vox` moved to **`examples/archive/legacy_syntax/`** (non-parseable on current grammar); added **`examples/STYLE.md`**, **`FEATURE_INDEX.md`**, **`PARSE_STATUS.md`**, archive READMEs.
- **`vox-compiler` `golden_examples_strict_parse`**: optional **`VOX_EXAMPLES_STRICT_PARSE=1`** requires every **`examples/golden/*.vox`** to parse (default CI remains golden-only).
- **`vox-cli`**: default features are **`mens-base` only** (no **`gpu`**). Native Mens train / probe / merge / eval-local require **`cargo build -p vox-cli --features gpu`**. **`vox-mens`** binary (prepends `mens` subcommand). **`vox-codex`** removed as a **`vox-cli`** dependency (`codex` / `stub-check` use **`vox-db`**); OS keyring helpers live in **`vox_db::secrets`** (**`vox-codex`** still re-exports for other crates). **`vox-corpus`** and **`vox-runtime`** are **always** linked so grammar / training JSONL paths work even with **`--no-default-features`** (`mens-base` remains the **command-surface** gate).
- **`vox-ludus`**: depends on **`vox-db`** instead of **`vox-codex`**.
- **Clippy (`-D warnings`)**: `cargo clippy --workspace --all-targets` is clean — auto-fixed **`collapsible_*`**, **`manual_clamp`**, **`iter_cloned_collect`**, **`lines_filter_map_ok`**, **`double_must_use`**, **`ptr_arg`**, **`manual_find`**, **`field_reassign_with_default`**, **`match_like_matches_macro`**, **`unnecessary_to_owned`**; targeted **`allow`** for intentional async tests (`await_holding_lock`), `Qwen35AttentionBlock` enum size, **`maybe_refresh_openrouter_models`** under `cfg(test)`, and serialized **`unsafe`** env updates in **`vox-integration-tests`**. **CI:** GitHub **`ci.yml`** and GitLab **`clippy`** job use **`--all-targets`** so integration / bench targets match local gates (**`workflow-enumeration.md`** updated).

### Added

- **Codex / data-plane CI:** `vox ci query-all-guard` and `vox ci turso-import-guard` (diff-scoped; `--all` for full tree), both run from `vox ci ssot-drift`; allowlists under `docs/agents/query-all-allowlist.txt` and `docs/agents/turso-import-allowlist.txt`. Ludus periodic reward SQL moved to `vox-db` `gamify_periodic_conditions` so `vox-ludus` no longer calls `query_all`.
- **Clavis cloudless vault env:** `VOX_CLAVIS_VAULT_PATH`, `VOX_CLAVIS_VAULT_URL`, `VOX_CLAVIS_VAULT_TOKEN` (precedence over deprecated `VOX_TURSO_*` / `TURSO_*` when compat aliases allowed); `vox clavis doctor` prints `cloudless_vault_store` diagnostics.
- **Telemetry (optional remote upload):** `vox telemetry status|export|enqueue|upload` with local JSON spool (`crates/vox-cli/src/telemetry_spool.rs`), Clavis `VoxTelemetryUploadUrl` / `VoxTelemetryUploadToken`, ADR [`docs/src/adr/023-optional-telemetry-remote-upload.md`](docs/src/adr/023-optional-telemetry-remote-upload.md), wire spec in `docs/src/archive/research-2026-q1/` (archived; do not ingest). Regenerate CLI/capability rows from `contracts/operations/catalog.v1.yaml` via `vox ci operations-sync --target cli --write` (and capability/MCP targets if you changed catalog tool rows). **Release discipline:** any change to telemetry contracts, upload behavior, or related env/Clavis IDs gets a bullet under **Telemetry** in this file’s `[Unreleased]` section.
- **Telemetry (Agent Budgeting):** Added `agent_exec_history` (S1) collection with 3-pillar tracking (time, compute tokens, vendor USD logic). Validated by `vox ci data-ssot-guards` with native 90-day retention aging.
- **Docs (research):** Terminal execution policy research (archived in `docs/src/archive/research-2026-q1/`; do not ingest) — PowerShell-first agent shells, Cursor/Gemini/Codex policy evidence, allowlist bypass lessons, alignment with operations SSOT.
- **Telemetry trust SSOT (docs):** architecture pages for trust boundaries, taxonomy (roadmap), retention/sensitivity (roadmap), client disclosure, implementation blueprint, and executable backlog; `VOX_BENCHMARK_TELEMETRY` / `VOX_SYNTAX_K_TELEMETRY` documented in `docs/src/reference/env-vars.md`. Entry points: `docs/src/architecture/telemetry-trust-ssot.md`, `AGENTS.md`, contributor hub.
- **LLM premature-completion SSOT:** `contracts/operations/completion-policy.v1.yaml` + telemetry JSON Schemas; `vox ci completion-audit` (writes `contracts/reports/completion-audit.v1.json`), `completion-gates` (Tier A + Tier B baseline `contracts/reports/completion-baseline.v1.json`), `completion-ingest` → VoxDB `ci_completion_*`; wired into `command-compliance`, `ssot-drift`, and GitHub `ci.yml`. Architecture: [`docs/src/architecture/completion-policy-ssot.md`](docs/src/architecture/completion-policy-ssot.md). Mens scorecard `summary.json` adds optional `completion_policy` crosswalk. **`--features completion-toestub`:** merges TOESTUB `victory-claim` into audit (Tier C per policy); CI uses it for `completion-audit`. **`completion-ingest`** fills snapshot `new_count` / `resolved_count` vs prior run fingerprints. **`contracts/reports/completion-task-ledger.v1.json`:** 768 explicit `T-WS###-NN` IDs. MCP chat system prompt adds anti-skeleton rider pointing at the policy. **`vox ci completion-audit --scan-extra <dir>`** (repeatable): audit generated trees under the repo (canonical paths must stay inside root).
- **`vox-orchestrator-d`** — TCP or **stdio** newline [`DispatchRequest`](crates/vox-protocol/src/lib.rs) (`orch.ping`, `orch.status`, `orch.task_status`, `orch.spawn_agent`, `orch.agent_ids`); shared **`VOX_MCP_AGENT_FLEET`** [`AgentFleet`](crates/vox-orchestrator/src/runtime.rs) spawn with MCP; MCP **`probe_external_orchestrator_daemon_if_configured`** skips stdio peer.
- **Docs / scripts:** [`docs/src/how-to/examples-corpus.md`](docs/src/how-to/examples-corpus.md), [`docs/src/how-to/first-full-stack-app.md`](docs/src/how-to/first-full-stack-app.md); 
- **Tests:** `budgets_json_loads_and_defines_all_timing_lanes`, `tool_registry_slice_tolerates_bracket_in_description` (LLM-audit hardening).
- **`vox ci build-timings`** — wall-clock **`cargo check`** for default `vox-cli`, GPU+stub lane, optional CUDA lane (`--json` supported). **`--crates`** adds `vox-cli --no-default-features`, `vox-db`, `vox-oratio`, `vox-mens --features train`, and **`vox-cli --features oratio`** lanes. Optional soft budgets: `docs/ci/build-timings/budgets.json` with **`VOX_BUILD_TIMINGS_BUDGET_WARN=1`** / **`VOX_BUILD_TIMINGS_BUDGET_FAIL=1`**. GitHub CI runs **`build-timings --crates`** in place of the standalone GPU/stub check step.
- **`oratio`** feature — Oratio / `vox-oratio` is no longer pulled by default **`mens-base`**; enable **`--features oratio`** for **`vox oratio`** (canonical speech CLI; alias **`vox speech`**). CI feature matrix includes an **`oratio`** compile lane.
- **Docs**: `docs/src/architecture/vox-cli-build-feature-inventory.md` (feature / compile-impact map); `docs/src/architecture/crate-topology-buckets.md` (workspace crate buckets); `docs/ci/build-timings/` (`budgets.json`, `snapshot-metadata.json`, optional `latest.jsonl`); migration matrix + deviation notes in `crate-build-lanes-migration.md`.
- **CLI / Mens**: `vox mens pipeline` — dogfood corpus → eval → optional native train (replaces PS1 orchestration); `scripts/run_mens_pipeline.ps1` is a thin delegate.
- **Scripts / builtins**: `std.process.run_capture` (stdout/stderr/exit record; non-zero exit still `Ok`); `std.fs.glob` (sorted paths); `vox-runtime` helpers `vox_process_run_capture`, `vox_fs_glob`.
- **Compilerd**: JSON `run` params accept optional **`mode`** (`auto` \| `app` \| `script`) aligned with `vox run --mode`.
- **CI**: `vox ci check-docs-ssot` scans `docs/src` and `.github/workflows` for retired Python inventory / `populi_release_gate.sh` references; `FEATURE_SETS` includes `script-execution` matrix rows; docs [command surface duals](docs/src/ci/command-surface-duals.md).
- **GitLab**: `vox-ci-guards` aligned with GitHub (`toestub-scoped`, `cuda-features`, `mens-gate --profile ci_full`); `NATIVE_TRAIN=false` rejected on `ml-train`.
- **CLI / CI**: `vox ci` guard commands (manifest, SSOT checks, doc-inventory, workflow allowlist, Mens `gates.yaml` runner, TOESTUB scoped, CUDA probes); crate `vox-doc-inventory`.
- **Scripts**: `ci_full` Mens profile; GitHub `ci.yml` cut over to `cargo run -p vox-cli -- ci …`; thin shell/ps1 delegates; `docs/agents/script-registry.json` + baseline metrics.
- **Run**: `vox run --mode {auto,app,script}`; `vox script` (feature `script-execution`); frontend bundle path uses **pnpm**; script builtins: `print`, `std.env.get`, `std.process.run` / `exit`, `std.fs.list_dir`, `std.args`.
- **Parser**: Trailing comma support in function parameter lists (A-072/A-100)
- **Parser**: Duplicate parameter name detection with clear error message (A-074/A-101)
- **Parser**: Error recovery test coverage (A-099)
- **Parser**: `filter_fields` support in `VectorIndexDecl` parsing
- **Typeck**: Lambda parameter type checking test (A-092)
- **Typeck**: Lambda outer scope capture test (A-093)
- **Typeck**: Match arm variable binding test (A-094)
- **Typeck**: Match exhaustiveness error test (A-095)
- **Store**: `CodeStore::dry_run_migration()` — report pending migrations without applying (B-059)
- **Store**: `CodeStore::health_check()` — `PRAGMA integrity_check` wrapper (B-060)
- **Store**: `CodeStore::batch_insert()` for bulk artifact insertion (B-062)
- **Store**: Pagination support (`LIMIT`/`OFFSET`) in `list_components` (B-063)
- **Store**: Relevance threshold filtering in `recall_memory` (B-064)
- **VoxDb**: `DbConfig::from_env()` for environment-based configuration (B-065)
- **VoxDb**: Retry logic (3× with backoff) in `VoxDb::connect` (B-066)
- **VoxDb**: `VoxDb::transaction()` wrapper for atomic operations (B-067)
- **VoxDb**: Integration test for in-memory connection (B-068)
- **AGENTS.md**: Phase 5 VoxPM roadmap merged from `PLAN.md` (B-076)
- **Docs**: `vox-runtime/README.md` — actor model architecture (B-112)
- **Docs**: `vox-pm/README.md` — CAS store architecture (B-113)
- **Docs**: mdBook search enabled with full-text indexing (A-136)
- **Docs**: Automated API reference pipeline `vox doc` (A-142)
- **Docs**: Decorator and Keyword manifests in JSON format (B-121/B-122)
- **Docs**: OpenGraph/SEO metadata and social sharing support (B-125)
- **Docs**: RSS/Atom feed generation for release notes (B-124)
- **CI**: Documentation build check and Rustdoc integration (B-117/B-118)
- **CI**: Dashboard API `dead_code` warnings suppressed (future integration)
- **OpenCode CLI**: `vox opencode` subcommand tree (install, setup, doctor, status, dashboard, spawn, review, config, sync, logs, share)
- **OpenCode CLI**: `vox opencode install` — downloads OpenCode AI and scaffolds config
- **OpenCode CLI**: `vox opencode doctor` — preflight check (binary, MCP, LSP, config, version)
- **OpenCode CLI**: `vox opencode dashboard` — launches embedded real-time agent dashboard
- **OpenCode CLI**: `vox completions <shell>` — generate shell completion scripts
- **OpenCode CLI**: `vox mcp-docs` — auto-generate MCP tool reference markdown table
- **OpenCode Integration**: `opencode.json` with version pinning (`opencode_version: >=0.2.0`)
- **OpenCode Integration**: Plugin API compatibility shim for OpenCode < 0.2.0
- **OpenCode Integration**: GitHub Actions workflow for `vox opencode doctor` in CI
- **MCP Server**: Protocol version negotiation (server echoes client's `protocolVersion`)
- **MCP Server**: 34 new tools (102 total): A2A messaging, VCS snapshots, JJ-inspired oplog/conflicts/workspaces, OpenCode bridge tools
- **MCP Tools**: `vox_map_agent_session`, `vox_record_cost`, `vox_heartbeat`, `vox_cost_history` (older docs/plugins may reference `vox_map_opencode_session` — use the canonical name)
- **MCP Tools**: `vox_a2a_send`, `vox_a2a_inbox`, `vox_a2a_ack`, `vox_a2a_broadcast`, `vox_a2a_history`
- **MCP Tools**: `vox_snapshot_*`, `vox_oplog`, `vox_undo`, `vox_redo`
- **MCP Tools**: `vox_workspace_*`, `vox_conflict_*`, `vox_change_*`, `vox_vcs_status`
- **Dashboard**: Redesigned with dark theme, glassmorphism, D3.js topology, SSE event log
- **Dashboard**: VCS panel, gamification panel, cost charts
- **Docs**: `docs/opencode-integration.md` — user-facing setup guide
- **Docs**: `docs/architecture/opencode-bridge.md` — technical deep-dive
- **Docs**: `docs/mcp-tool-reference.md` — auto-generated from 102 MCP tool schemas
- **Docs**: `docs/troubleshooting-faq.md` — common issues: port conflicts, MCP timeouts, LSP crashes
- **AGENTS.md**: Updated with 102 MCP tool list, OpenCode bridge section, new documentation links
- **CLI UX**: Colored output with actionable error suggestions in `vox opencode` commands


### Fixed
- **Codex / CI SSOT:** Restore `scripts/check_codex_ssot.sh` (bash delegate to `vox ci check-codex-ssot`); align `contracts/db/baseline-version-policy.yaml` with `BASELINE_VERSION` **45** and the current baseline SQL Keccak digest; refresh `contracts/capability/model-manifest.generated.json` so `vox ci ssot-drift` stays green.
- **Telemetry trust SSOT:** `docs/src/architecture/telemetry-trust-ssot.md` maps build timing / `build_*` observability, `VOX_BENCHMARK_TELEMETRY`, and MCP `vox_benchmark_list` `source` selectors.
- **Tests:** `vox-test-harness` `hir_fn` initializer matches `HirFn::is_mobile_native`; `vox-db` `local_tests` fixes `VOX_DATA_DIR` `set_var` delimiter typo.
- **Docs SSOT:** `orphan-surface-inventory.md` workspace crate list includes **`vox-dei`** so `vox ci check-docs-ssot` matches filesystem inventory.
- **`vox ci build-timings`:** Soft budgets load only from **`docs/ci/build-timings/budgets.json`** (no duplicate Rust const); `VOX_BUILD_TIMINGS_BUDGET_FAIL=1` works without `BUDGET_WARN`; `--json` serialization errors surface with context; `nvcc` probe uses **`CUDA_PATH`** / **`CUDA_HOME`** when `PATH` is stripped.
- **`vox ci command-compliance`:** `TOOL_REGISTRY` slice uses stable **anchor** before `pub fn tool_registry` so `]` inside description strings cannot break parsing.
- **`vox ci command-compliance`**: MCP wiring validates `TOOL_REGISTRY` vs canonical-only `handle_tool_call` arms and checks `crates/vox-mcp/src/tools/tool_aliases.rs` (`TOOL_WIRE_ALIASES` → canonical names).
- **`vox-cli`**: `benchmark_telemetry::record_opt_blocking` no longer mixes `std::io::Error` with Codex `StoreError` in a single `Result` chain.
- **Store**: Replaced `.unwrap()` on embedding `try_into()` with proper error handling (B-056)
- **Normalize**: All `AstNode` variants now have explicit cases (no wildcard fallthrough) (B-058)
- **LSP**: Removed unused imports in `main.rs`

### Removed
- `PLAN.md` — content merged into `AGENTS.md` §3 (B-076)
