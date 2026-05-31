---
title: "Telemetry and research_metrics contract"
description: "research_metrics row shape, write validation, session_id prefix conventions, key metric_type semantics, Mens telemetry.jsonl envelope and KPI tiers, deprecation notes, and related CI guards."
category: "Language Reference"

schema_type: "TechArticle"
---

# Telemetry & `research_metrics` contract

## Related SSOT

- [Telemetry trust boundary and SSOT map](../architecture/telemetry-trust-ssot.md)
- [Telemetry taxonomy and contracts SSOT](../archive/research-2026-q1/telemetry-taxonomy-contracts-ssot.md) (roadmap)
- [Telemetry retention and sensitivity SSOT](../archive/research-2026-q1/telemetry-retention-sensitivity-ssot.md) (roadmap)
- [Telemetry client disclosure SSOT](../archive/research-2026-q1/telemetry-client-disclosure-ssot.md)
- [Telemetry implementation blueprint 2026](../archive/research-2026-q1/telemetry-implementation-blueprint-2026.md) and [backlog](../archive/research-2026-q1/telemetry-implementation-backlog-2026.md)
- Optional **explicit** remote upload (local JSON spool, not `research_metrics`): [ADR 023](../adr/023-optional-telemetry-remote-upload.md), [Telemetry remote sink specification](../archive/research-2026-q1/telemetry-remote-sink-spec.md), CLI **`vox telemetry`**

Code enforcement for row validation: [`validate_research_metric_row`](../../../crates/vox-db/src/research_metrics_contract.rs) (called from `append_research_metric`). Repository-scoped producers should use [`TelemetryWriteOptions`](../../../crates/vox-db/src/research_metrics_contract.rs) plus the `METRIC_TYPE_*` / `SESSION_PREFIX_*` / `SESSION_ID_*` constants in [`vox_db::research_metrics_contract`](../../../crates/vox-db/src/research_metrics_contract.rs).

## Row shape

Table `research_metrics` columns: `session_id`, `metric_type`, `metric_value` (nullable `REAL`), `metadata_json`.

- **`metric_value`**: optional scalar. **SQL `NULL` means “no scalar”** — APIs must not coerce NULL to `0.0` (aggregations skip nulls; see `list_research_metrics_by_type`).
- **`metadata_json`**: structured payload; may include units and names that disambiguate mixed benchmarks.

## Validation limits (writes)

| Field | Rule |
|-------|------|
| `session_id` | Non-empty; max **512** UTF-8 characters. |
| `metric_type` | Non-empty; max **128** characters; characters must be ASCII alphanumeric or `_`, `.`, `-`, **`:`** (colon allows MCP-linked namespaces such as `foo:bar`). |
| `metadata_json` | Optional; if present, max **256 KiB** serialized length. |

## Session id namespaces (convention)

Producers should prefix `session_id` so rollups and dashboards can group without colliding:

| Prefix | Example | Typical producer |
|--------|---------|------------------|
| `bench:` | `bench:<repository_id>` | CLI / build timings |
| `syntaxk:` | `syntaxk:<repository_id>` | Syntax-K eval fixtures |
| `mcp:` | `mcp:<repository_id>` | MCP Socrates / surface telemetry |
| `mens:` | `mens:<repository_id>` | Populi control-plane audit (`populi_control_event`) |
| `workflow:` | `workflow:<repository_id>` | Interpreted workflow journal (`workflow_journal_entry`, versioned event payloads from the workflow durability contract) |
| `route:` | `route:<repository_id>` | Routing policy and capability-gate telemetry (`model_route_event`) |

**Fixed session (no repository in id):** hybrid memory fusion uses session `socrates:retrieval` and metric type `memory_hybrid_fusion` (see `SESSION_ID_MEMORY_HYBRID_FUSION` in the Rust module).

**Questioning / linked metrics:** MCP may use opaque `session_key` strings for `questioning_event` and `vox_db_research_metric_linked` (not forced through [`TelemetryWriteOptions`](../../../crates/vox-db/src/research_metrics_contract.rs)); those rows still must satisfy validation caps above.

## Metric types (non-exhaustive)

| `metric_type` | Session prefix | Scalar semantics | Notes |
|---------------|----------------|------------------|-------|
| `benchmark_event` | `bench:<repository_id>` | Optional; unit in metadata `metric_value_unit` | CLI build timings use **`seconds`** for wall time. |
| `syntax_k_event` | `syntaxk:<repository_id>` | Optional ratio / timing | Fixture id in metadata; optional `support_metrics` (representability / LLM surface / runtime projection summaries per `contracts/eval/syntax-k-event.schema.json`). |
| `socrates_surface` | `mcp:<repository_id>` | Hallucination-risk proxy | Prefer metadata for interpretability; eval summaries inject explicit denominators (below). |
| `agent_exec_time` | `mcp:<repository_id>` | Optional | Tool execution duration/budget events used for execution-time calibration. |
| `model_route_event` | `route:<repository_id>` | Optional | Runtime/orchestrator route decisions. `metadata_json` must include `trace_id` and `route_policy_profile`. |
| `model_call_event` | `route:` / `mcp:` / producer-defined | Optional | Per-call LLM / provider invocation audit row (latency, provider id, tokens when present). |
| `task.root_summary` | producer-defined | Optional | High-level task / conversation rollup for dashboards. |
| `build.summary` | `bench:` | Optional | Build or CI lane rollup (`METRIC_TYPE_BUILD_SUMMARY_EVENT`). |
| `telemetry.error` | producer-defined | Optional | Structured error / fault envelope (`METRIC_TYPE_ERROR_EVENT`); use for HTTP failures, rate limits, and similar fault paths. |
| `fixture.model.intent_resolved` | producer-defined | Optional | AI-first `@ai` fixture: intent routing metadata (task category, strengths) (`METRIC_TYPE_FIXTURE_MODEL_INTENT`). |
| `fixture.prompt.dispatch` | producer-defined | Optional | AI-first `@prompt` fixture: cascade dispatch observation (`METRIC_TYPE_FIXTURE_PROMPT_DISPATCH`). |
| `fixture.search.dispatch` | producer-defined | Optional | AI-first `@search` fixture: retrieval dispatch observation (`METRIC_TYPE_FIXTURE_SEARCH_DISPATCH`). |
| `fixture.hole.observed` | producer-defined | Optional | AI-first `@hole` fixture: compile-time hole observed (`METRIC_TYPE_FIXTURE_HOLE_OBSERVED`). |
| `orch.circuit_breaker.trip` | `route:<repository_id>` or orchestrator session key | Optional counter / trip marker | Emitted when the orchestrator doom-loop circuit breaker transitions to **Open** (`METRIC_TYPE_CIRCUIT_BREAKER_TRIP`). |
| `orch.socrates.fusion` | orchestrator / MCP session | Optional | Socrates eval fusion rollup (`METRIC_TYPE_SOCRATES_FUSION`). |
| `orch.routing.tier` | `route:<repository_id>` | Optional | Tier routing decision telemetry (`METRIC_TYPE_MODEL_TIER_ROUTE`). |
| `orch.plan.mode_decision` | orchestrator session | Optional | Plan-mode selection (`METRIC_TYPE_PLAN_MODE_DECISION`). |
| `orch.hitl.interrupt` | orchestrator session | Optional | Human-in-the-loop interrupt signal (`METRIC_TYPE_HITL_INTERRUPT`). |
| `orch.risk.score` | orchestrator session | Optional | Orchestrator risk scoring (`METRIC_TYPE_RISK_SCORE`). |
| `orch.privacy.route_decision` | orchestrator session | Optional | Privacy-aware routing outcome (`METRIC_TYPE_PRIVACY_ROUTE_DECISION`). |
| `orch.cache.hit_prediction` | orchestrator session | Optional | Cache hit / predictor telemetry (`METRIC_TYPE_CACHE_HIT_PREDICTION`). |
| `orch.budget.decision` | orchestrator session | Optional | Budget gate decisions (`METRIC_TYPE_BUDGET_DECISION`). |
| `orch.calibration.run` | orchestrator session | Optional | Calibration job lifecycle (`METRIC_TYPE_CALIBRATION_RUN`). |
| `orch.calibration.drift_alert` | orchestrator session | Optional | Model / signal drift alert (`METRIC_TYPE_DRIFT_ALERT`). |
| `orch.calibration.bandit_update` | orchestrator session | Optional | Bandit / Thompson-style policy update (`METRIC_TYPE_BANDIT_UPDATE`). |
| `orch.subagent.dispatch` | orchestrator session | Optional | Sub-agent dispatch (`METRIC_TYPE_SUBAGENT_DISPATCH`). |
| `orch.subagent.chain_depth_alert` | orchestrator session | Optional | Deep sub-agent chain warning (`METRIC_TYPE_CHAIN_DEPTH_ALERT`). |
| `orch.agentos.guardrail_deny` | orchestrator session | Optional | AgentOS guardrail denial (`METRIC_TYPE_AGENTOS_GUARDRAIL_DENY`). |
| `vox.lint.finding` | `lint:<repository_id>` | Optional | CR-L8 corpus-feedback: one lint rule firing at a specific location (`METRIC_TYPE_LINT_FINDING`). |
| `vox.lint.autofix_applied` | `lint:<repository_id>` | Optional | CR-L8 corpus-feedback: an autofix was accepted and applied (`METRIC_TYPE_LINT_AUTOFIX_APPLIED`). |
| `vox.lint.autofix_rejected` | `lint:<repository_id>` | Optional | CR-L8 corpus-feedback: an autofix was rejected by the user (`METRIC_TYPE_LINT_AUTOFIX_REJECTED`). |
| `vox.repair.attempt` | `repair:<session_id>` | Optional | CR-L8 corpus-feedback: one LLM repair attempt (`METRIC_TYPE_REPAIR_ATTEMPT`). |
| `vox.repair.outcome` | `repair:<session_id>` | Optional | CR-L8 corpus-feedback: terminal outcome of a `vox repair` session (`METRIC_TYPE_REPAIR_OUTCOME`). |
| `vox.audit.run` | `audit:<thing>` | Optional | Per-`vox audit <thing>` run observation (corpus hash, duration, outcome) (`METRIC_TYPE_AUDIT_RUN`). |
| `vox.model.selection_decision` | `model_autonomic:<id>` | Optional | Model-autonomic L0: one `select(intent, registry)` decision (`METRIC_TYPE_SELECTION_DECISION`). |
| `vox.model.discovery` | `model_autonomic:<id>` | Optional | Model-autonomic L1: new model id discovered in upstream catalog (`METRIC_TYPE_MODEL_DISCOVERY`). |
| `vox.model.classification` | `model_autonomic:<id>` | Optional | Model-autonomic L2: classifier tagged a model with tier/strengths/confidence (`METRIC_TYPE_MODEL_CLASSIFICATION`). |
| `vox.model.confidence_promotion` | `model_autonomic:<id>` | Optional | Model-autonomic L2/L3: model crossed a confidence-state boundary (`METRIC_TYPE_CONFIDENCE_PROMOTION`). |
| `plugin.load_failure` | `plugin:<plugin_id>` | Optional | Plugin failed to load (ABI mismatch, missing entrypoint, or init error) (`METRIC_TYPE_PLUGIN_LOAD_FAILURE`). |
| `sandbox.timeout_kill` | activity session | Optional | Activity sandbox killed after exceeding its configured timeout — distinct from `agent_exec_time` which only tracks completed runs (`METRIC_TYPE_SANDBOX_TIMEOUT_KILL`). |
| `orch.cache.miss` | orchestrator session | Optional | LLM provider prompt-cache lookup missed; enables online miss-rate computation alongside `cache_read_input_tokens` on `ModelCallEvent` (`METRIC_TYPE_ORCH_CACHE_MISS`). |
| `orch.task.cancelled` | orchestrator session | Optional | Task was explicitly cancelled mid-execution (user interrupt, parent timeout, or lease loss) (`METRIC_TYPE_ORCH_TASK_CANCELLED`). |
| `vox.doctor.project_check` | `doctor:<path>` | Optional | CR-L7: per-`vox doctor --project <path>` health-check run — structured outcome alongside human-readable output (`METRIC_TYPE_DOCTOR_PROJECT_CHECK`). |
| `audit.effort.run.started` | `audit:<run_id>` | Optional | `vox audit effort` run lifecycle: scan started (`METRIC_TYPE_AUDIT_EFFORT_RUN_STARTED`). |
| `audit.effort.run.completed` | `audit:<run_id>` | Optional; duration in metadata | `vox audit effort` run lifecycle: scan completed successfully (`METRIC_TYPE_AUDIT_EFFORT_RUN_COMPLETED`). |
| `audit.effort.commit.judged` | `audit:<run_id>` | Optional | `vox audit effort`: one commit scored/classified during a run (`METRIC_TYPE_AUDIT_EFFORT_COMMIT_JUDGED`). |
| `audit.route.run.started` | `audit:<run_id>` | Optional | `vox audit effort-route` run lifecycle: routing started (`METRIC_TYPE_AUDIT_ROUTE_RUN_STARTED`). |
| `audit.route.run.completed` | `audit:<run_id>` | Optional; duration in metadata | `vox audit effort-route` run lifecycle: routing completed successfully (`METRIC_TYPE_AUDIT_ROUTE_RUN_COMPLETED`). |
| `audit.route.run.failed` | `audit:<run_id>` | Optional | `vox audit effort-route` run lifecycle: routing failed before completion (`METRIC_TYPE_AUDIT_ROUTE_RUN_FAILED`). |
| `audit.route.cluster.decided` | `audit:<run_id>` | Optional | `vox audit effort-route`: one cluster-and-route decision recorded (`METRIC_TYPE_AUDIT_ROUTE_CLUSTER_DECIDED`). |
| `audit.effort.run.failed` | `audit:<run_id>` | Optional | `vox audit effort` run lifecycle: scan failed before completion (`METRIC_TYPE_AUDIT_EFFORT_RUN_FAILED`). |

### `socrates_surface` aggregate metadata (`record_socrates_eval_summary`)

Rollups written to `eval_runs` include JSON with both raw counts and **explicit denominators** so downstream tools do not misread rates when some rows lack a scalar or parseable metadata:

- `rate_denominator`: literal `"parsed_metadata_rows"` — rates (`answer_rate`, `abstain_rate`) use this count.
- `abstain_rate_denominator_n` / `answer_rate_denominator_n`: same as `parsed_metadata_rows`.
- `mean_proxy_denominator_n`: `rows_with_metric_value` — mean hallucination-risk proxy uses only rows where `metric_value` was non-NULL.
- `rows_total_n`: `sample_size` — all `socrates_surface` rows scanned.

**Quality** in `eval_runs` uses the mean proxy **only** when `rows_with_metric_value > 0`; otherwise quality is **0.0** (avoids implying a perfect score with no scalar signal).

### `benchmark_event` metadata (`BenchmarkEventMeta`)

- `name`: logical benchmark id (`cargo_build_metrics`, …).
- `metric_value_unit`: when `metric_value` is set, unit SSOT (`seconds`, `milliseconds`, `ratio`, …).
- `details`: free-form JSON (per-crate timings, pass/fail flags).

### Build timing producers (current)

- `vox ci build-timings` (shallow lanes) writes `benchmark_event` name `ci_build_timings` with:
  - `metric_value`: total wall time in `seconds`,
  - `metric_value_unit`: `seconds`,
  - `details`: lane rows (`lane`, `ok`, `ms`) plus `total_ms`.
- `vox ci build-timings --deep` writes structured rows to `build_run` / `build_crate_sample` /
  `build_warning`; on structured-write fallback it writes `benchmark_event` name
  `cargo_build_metrics` with `metric_value_unit = seconds`.
- `VOX_BENCHMARK_TELEMETRY=1` controls `benchmark_event` writes; structured `build_*` writes follow
  command persistence settings and VoxDB availability.

For cross-repo querying via MCP, `benchmark_event` may use `name = "cross_repo_query"` with `metric_value_unit = "milliseconds"` and `details` such as:

- `query_kind`
- `trace_id`
- `correlation_id`
- `conversation_id`
- `workspace_repository_id`
- `target_repository_ids`
- `source_plane`
- `query_backend`
- `result_count`
- `skipped_count`

## Training JSONL (`telemetry.jsonl`)

Envelope per line: `{ "ts_ms", "event", "payload" }`. Payload keys are defined in `crates/vox-populi/src/mens/tensor/telemetry_schema.rs` (e.g. `eta_seconds_remaining`, `steps_per_sec_ema`). The CLI viewer `vox mens watch-telemetry` must track this schema (guarded by `vox ci data-ssot-guards`).

### Mens training KPI ownership (decision-driving)

- **Tier 1 (gate-driving)**:
  - `tokens_per_sec` (with `tokens_per_sec_is_proxy` when derived),
  - `valid_tokens`,
  - `theoretical_tokens`,
  - `supervised_ratio_pct`.
- **Tier 2 (diagnostic)**:
  - `steps_per_sec_ema`,
  - `eta_seconds_remaining`,
  - skip counters (`skip_no_supervised_positions`, `skip_short_seq`, ...).

### Deprecation / compatibility window

- Consumers should prefer canonical fields above.
- Legacy aliases are still read with warnings (status / eval-gate paths), then normalized at read time.
- `steps_per_sec_ema` as a throughput surrogate is considered deprecated for gates when `tokens_per_sec` is present.

## CI

- `vox ci data-ssot-guards` — asserts watch-telemetry references schema keys and `research_metrics` list API avoids `COALESCE(metric_value, 0.0)`.
- Web IR structural gate: workflow sets `VOX_WEBIR_VALIDATE=1` and runs `cargo nextest run -p vox-compiler --test web_ir_lower_emit_test --run-ignored ignored-only` (see `.github/workflows/ci.yml`).
