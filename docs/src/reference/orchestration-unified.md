---
title: "Unified orchestration — SSOT"
description: "Official documentation for Unified orchestration — SSOT for the Vox language. Detailed technical reference, architecture guides, and impl"
category: "Language Reference"
last_updated: "2026-04-29"
training_eligible: true

schema_type: "TechArticle"
---

# Unified orchestration — SSOT

This document captures **compatibility rules** and **opt-in migration toggles** while MCP, CLI, and DeI share one orchestrator contract (`vox-orchestrator`).

## Workspace journey store (Codex)

Repo-backed **`vox-mcp`** and **`vox-orchestrator-d`** open the primary [`VoxDb`](../../../crates/vox-db/src/lib.rs) via [`connect_workspace_journey_optional`](../../../crates/vox-db/src/workspace_journey_store.rs) (default **`.vox/store.db`**). Env: **`VOX_WORKSPACE_JOURNEY_STORE`**, **`VOX_WORKSPACE_JOURNEY_FALLBACK_CANONICAL`** ([env SSOT](env-vars.md)). Daemon diagnostics: JSON-RPC method **`orch.workspace_journey`** (bind `repository_id` vs discovered repo).

**Bridge / routing policy:** Vox-first codegen remains the default MCP path (`vox_generate_code`, local inference server for `vox generate`); non-Vox edits stay bounded behind explicit tools and repository policy — see [completion policy SSOT](../archive/research-2026-q1/completion-policy-ssot.md).

**Journey envelope (v1):** [`contracts/orchestration/journey-envelope.v1.schema.json`](../../../contracts/orchestration/journey-envelope.v1.schema.json) is the machine SSOT for per-request metadata (`journey_id`, `session_id`, `thread_id`, trace/correlation ids, `repository_id`, `origin_surface`). MCP `vox_chat_message` embeds this shape in structured transcript payloads; CLI and daemon surfaces wire fields incrementally.

**Canonical MENS dev journey (Codex):** Tables `developer_journey_definitions` / `developer_journey_steps` (baseline fragment `developer_journeys`) seed `canonical_journey.v1.greenfield_vox_mens_devloop`. MCP **`vox_journey_canonical_steps`** returns ordered `step_json` rows when `VoxDb` is attached. Human-readable limitation ids for journey maturity live in [`contracts/journeys/limitations.v1.yaml`](../../../contracts/journeys/limitations.v1.yaml).

**DeI planning on the daemon:** JSON-line DeI methods `ai.plan.new`, `ai.plan.replan`, `ai.plan.status`, and `ai.plan.execute` are handled on the **`vox-orchestrator-d`** stdio surface (`orch_daemon::dei_dispatch`). Persistent plan rows require the same Codex `VoxDb` handle the orchestrator was built with.

## Ownership: who writes what

| Concern | Embedded MCP (`vox-mcp`) | `vox-orchestrator-d` (daemon) | VoxDb / Turso |
| --- | --- | --- | --- |
| Session chat transcript (RAM) | Orchestrator [`ContextStore`](../../../crates/vox-orchestrator/src/context/mod.rs) in-process | Same process model per ADR 022 until RPC parity | — |
| Structured chat turns | `chat_append_workspace_message` + journey envelope v1 | Future `orch.*` parity for remote clients | `conversation_messages`, `conversations` |
| Legacy `chat_transcripts` rows | MCP chat path (dual-write) | Not primary writer today | `chat_transcripts` |
| Workspace journey attach / diagnostics | `connect_workspace_journey_optional`, MCP tooling | JSON-RPC `orch.workspace_journey` | journey + repo bind rows |
| Routing decisions (`routing_decisions`) | MCP chat / codegen tools; **orchestrator `AiTaskProcessor`** when DB attached | Same table when daemon shares DB | local-first SQLite |
| Unified routing experiment flag | — | — | `VOX_UNIFIED_ROUTING` (telemetry reason shape in `vox-actor-runtime::routing_telemetry`) |

## HITL Doubt Flow

When agents detect ambiguity, they invoke the `vox_doubt_task` MCP tool. This transitions the task to `TaskStatus::Doubted` and emits a `TaskDoubted` event; the resolution agent inside `vox-orchestrator` (see [`crates/vox-orchestrator/src/orchestrator/agent/doubt.rs`](../../../crates/vox-orchestrator/src/orchestrator/agent/doubt.rs)) takes over to resolve the doubt with the user and submits an audit report that hooks into the gamification system (`vox-gamify`). For structural details, see the canonical [HITL & Doubt reference](hitl-and-doubt.md).

## Contract surfaces

- **Repo reconstruction campaigns:** JSON Schema `contracts/orchestration/repo-reconstruction.schema.json`; benchmark tiers and KPI guidance in [repo reconstruction benchmark ladder](repo-reconstruction-benchmark-ladder.md). Remote task envelopes may include optional `exec_lease_id` and `campaign_id` for mesh correlation (see ADR 017).
- **Types:** `vox_orchestrator::contract` — `TaskCapabilityHints`, `SessionContractEnvelope`, `OrchestrationMigrationFlags` (`orchestration_v2_enabled`, `legacy_orchestration_fallback`), MCP ↔ DeI plan tool alignment (`MCP_PLAN_TOOL_NAMES`, `DEI_PLAN_METHODS_NEW_REPLAN_STATUS`).
- **Runtime config:** `vox_orchestrator::OrchestratorConfig` — process-wide limits, Socrates gates, scaling knobs, and nested **`orchestration_migration`** (`OrchestrationMigrationFlags`). Loaded from `Vox.toml` `[orchestrator]` and **`VOX_ORCHESTRATOR_*`** env overrides via `OrchestratorConfig::merge_env_overrides` in `crates/vox-orchestrator/src/config/`.

### Agent queue capabilities (`TaskCapabilityHints`)

On **`Orchestrator::spawn_agent`**, each new [`AgentQueue`](../../../crates/vox-orchestrator/src/queue/mod.rs) gets capabilities from **`merge_agent_capabilities`** (`crates/vox-orchestrator/src/capability_probe.rs`):

1. Start from **`default_agent_capabilities`** in config / TOML.
2. Overlay **host probe** via **`probe_host_capabilities`**: `cpu_cores` (from `available_parallelism`), `arch` (`std::env::consts::ARCH`), `hostname` (`HOSTNAME` / `COMPUTERNAME`, or `sysinfo` when built with **`system-metrics`**).
3. **Labels:** config labels preserved first; probe-supplied labels appended without duplicates.
4. **GPU / NPU flags:** operator config wins if already `true`; otherwise probe may set `gpu_cuda` when **`VOX_MESH_ADVERTISE_GPU=1|true`** (legacy workstation advertisement), or `gpu_vulkan` / `gpu_webgpu` / `npu` from the matching **`VOX_MESH_ADVERTISE_*`** vars (not driver probes). Optional **`VOX_MESH_DEVICE_CLASS`** fills `device_class`. See [mobile / edge AI SSOT](mobile-edge-ai.md).
5. **`min_vram_mb` / `min_cpu_cores`:** filled from probe only when unset in config.

Routing reads **`capability_requirements`** on tasks and applies GPU / VRAM / **`min_cpu_cores`** / **`prefer_gpu_compute`** soft penalties in `crates/vox-orchestrator/src/services/routing.rs` (mens / Mens-style training hints).

When MCP polls **`GET /v1/populi/nodes`**, each row becomes a [`RemotePopuliRoutingHint`](../../../crates/vox-orchestrator/src/populi_federation.rs): if `last_seen_unix_ms` is older than orchestrator **`stale_threshold_ms`** at poll time, **`heartbeat_stale`** is set and experimental Populi routing signals skip that node (maintenance / quarantine were already excluded).

Optional **`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_RECONCILE`**: same poll tick may call **`GET /v1/populi/exec/leases`** and compare each **`holder_node_id`** to the fresh node list (tracing target **`vox.mcp.populi_reconcile`**; Codex event **`mesh_exec_lease_reconcile`** when **`VOX_MESH_CODEX_TELEMETRY`**). Opt-in **`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_AUTO_REVOKE`** performs **`POST /v1/populi/admin/exec-lease/revoke`** on mismatches (mesh/admin token; aggressive — see [env SSOT](env-vars.md)).

See also [mens SSOT](populi.md) for `VOX_MESH_*` and local registry.

## Mesh distribution vs single-process embedding

- **Embedding:** Each `vox-mcp` (or `vox dei` CLI) process constructs an in-memory [`Orchestrator`](../../../crates/vox-orchestrator/src/orchestrator.rs). That is “single-process gravity” for **RAM-local** queues and locks.
- **Distribution:** With **`VOX_MESH_ENABLED`**, durable coordination (locks, oplog mirror, A2A inboxes, heartbeats) is backed by **Turso** so **another** MCP or laptop can participate in the same logical mesh. Two nodes = two orchestrator **instances** sharing **one** cross-node SSOT via the DB and HTTP A2A relay — not one magic cluster master in RAM.
- **Bootstrap SSOT:** [`build_repo_scoped_orchestrator`](../../../crates/vox-orchestrator/src/bootstrap.rs) and [`build_repo_scoped_orchestrator_for_repository`](../../../crates/vox-orchestrator/src/bootstrap.rs) are the shared factory for MCP, CLI, and other embedders so repository id, affinity groups, and memory shard paths stay aligned.

For table-level detail and conflict rules, see [Mens coordination](populi-coordination.md).

## A2A delivery planes

**Naming:** **Vox internal A2A bus** refers to same-process and DB-backed orchestrator messaging (`local_ephemeral` / `local_durable` below). **LF A2A v1.0 Agent Card** (external Linux Foundation discovery) is planned separately — see stub [`contracts/orchestration/a2a-agent-card.v1.json`](../../../contracts/orchestration/a2a-agent-card.v1.json) and [communication-protocols.md](communication-protocols.md). Do not assume wire compatibility between the two.

The orchestrator intentionally uses more than one delivery plane; these are **not** interchangeable transports with hidden semantics.

| Canonical plane | Current wire token(s) | Guarantees | Use for |
| --- | --- | --- | --- |
| `local_ephemeral` | MCP `route=local` | in-process only, best-effort per-receiver FIFO, restart-volatile | low-latency same-node agent coordination |
| `local_durable` | MCP `route=db` | durable row storage, explicit durable ack/poll semantics | cross-process local inboxes and persistence-friendly retries |
| `remote_mesh` | MCP `route=mesh`, Populi HTTP A2A | HTTP relay with bearer/JWT auth, explicit inbox lease + ack, client-supplied idempotency | cross-node messaging and remote task envelopes |
| `broadcast` | local bus broadcast, bulletin/event fanout | receiver-local ordering only, no shared durable semantics | fanout notifications |
| `stream` | DeI JSON lines, `vox-orchestrator-d` `orch.*` JSON lines/TCP, MCP WS gateway, SSE, OpenClaw WS | ordered per connection/byte stream, reconnect semantics vary by transport | incremental output and live updates |

Machine-readable source of truth for these names lives in [`contracts/communication/protocol-catalog.yaml`](../../../contracts/communication/protocol-catalog.yaml). MCP A2A responses surface the canonical plane names in addition to legacy wire tokens so callers can migrate without breaking compatibility.

## Environment and config

### `OrchestratorConfig` — `VOX_ORCHESTRATOR_*`

Boolean fields use Rust `bool` parsing (`true` / `false` only). Invalid values log a warning and leave the current setting unchanged.

| Variable | Maps to |
| -------- | -------- |
| `VOX_ORCHESTRATOR_ENABLED` | `enabled` |
| `VOX_ORCHESTRATOR_MAX_AGENTS` | `max_agents` |
| `VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS` | `lock_timeout_ms` |
| `VOX_ORCHESTRATOR_TOESTUB_GATE` | `toestub_gate` |
| `VOX_ORCHESTRATOR_MAX_DEBUG_ITERATIONS` | `max_debug_iterations` |
| `VOX_ORCHESTRATOR_SOCRATES_GATE_SHADOW` | `socrates_gate_shadow` |
| `VOX_ORCHESTRATOR_SOCRATES_GATE_ENFORCE` | `socrates_gate_enforce` |
| `VOX_ORCHESTRATOR_SOCRATES_REPUTATION_ROUTING` | `socrates_reputation_routing` |
| `VOX_ORCHESTRATOR_SOCRATES_REPUTATION_WEIGHT` | `socrates_reputation_weight` |
| `VOX_ORCHESTRATOR_TRUST_GATE_RELAX_ENABLED` | `trust_gate_relax_enabled` — when `true` and Codex `agent_reliability` for the agent is ≥ [`trust_gate_relax_min_reliability`](../../../crates/vox-orchestrator/src/config/orchestrator_fields.rs), **Socrates enforce**, **completion grounding enforce**, and **strict scope** may skip completion requeue / enqueue denial (see [`PolicyTrustRelax`](../../../crates/vox-orchestrator/src/services/policy.rs)). |
| `VOX_ORCHESTRATOR_TRUST_GATE_RELAX_MIN_RELIABILITY` | `trust_gate_relax_min_reliability` — minimum reliability (default `0.85`, aligned with trust auto-approve floor). |
| `VOX_ORCHESTRATOR_ATTENTION_ENABLED` / `VOX_ORCHESTRATOR_ATTENTION_BUDGET_MS` / `VOX_ORCHESTRATOR_ATTENTION_ALERT_THRESHOLD` / `VOX_ORCHESTRATOR_ATTENTION_INTERRUPT_COST_MS` / `VOX_ORCHESTRATOR_ATTENTION_TRUST_ROUTING_WEIGHT` | Pilot attention budget + dynamic interruption gating (see [`information-theoretic-questioning.md`](information-theoretic-questioning.md), [`env-vars.md`](env-vars.md)). `Vox.toml` also supports `[orchestrator].interruption_calibration` for per-channel gain offsets and backlog/trust calibration. |
| `VOX_ORCHESTRATOR_LOG_LEVEL` | `log_level` (raw string) |
| `VOX_ORCHESTRATOR_FALLBACK_SINGLE` | `fallback_to_single_agent` |
| `VOX_ORCHESTRATOR_MIN_AGENTS` | `min_agents` |
| `VOX_ORCHESTRATOR_SCALING_THRESHOLD` | `scaling_threshold` |
| `VOX_ORCHESTRATOR_IDLE_RETIREMENT_MS` | `idle_retirement_ms` |
| `VOX_ORCHESTRATOR_SCALING_ENABLED` | `scaling_enabled` |
| `VOX_ORCHESTRATOR_COST_PREFERENCE` | `cost_preference` (`performance` \| `economy`) |
| `VOX_ORCHESTRATOR_SCALING_LOOKBACK` | `scaling_lookback_ticks` |
| `VOX_ORCHESTRATOR_RESOURCE_WEIGHT` | `resource_weight` |
| `VOX_ORCHESTRATOR_RESOURCE_CPU_MULT` | `resource_cpu_multiplier` |
| `VOX_ORCHESTRATOR_RESOURCE_MEM_MULT` | `resource_mem_multiplier` |
| `VOX_ORCHESTRATOR_RESOURCE_EXPONENT` | `resource_exponent` |
| `VOX_ORCHESTRATOR_SCALING_PROFILE` | `scaling_profile` (`conservative` \| `balanced` \| `aggressive`) |
| `VOX_ORCHESTRATOR_MAX_SPAWN_PER_TICK` | `max_spawn_per_tick` |
| `VOX_ORCHESTRATOR_SCALING_COOLDOWN_MS` | `scaling_cooldown_ms` |
| `VOX_ORCHESTRATOR_URGENT_REBALANCE_THRESHOLD` | `urgent_rebalance_threshold` |
| `VOX_ORCHESTRATOR_MIGRATION_V2_ENABLED` | `orchestration_migration.orchestration_v2_enabled` |
| `VOX_ORCHESTRATOR_MIGRATION_LEGACY_FALLBACK` | `orchestration_migration.legacy_orchestration_fallback` |
| `VOX_ORCHESTRATOR_MESH_CONTROL_URL` | `populi_control_url` — HTTP base for **`GET /v1/populi/nodes`** (read-only); MCP `vox_orchestrator_status` includes **`mesh_snapshot`** JSON when set. Uses **`VOX_MESH_TOKEN`** on the client when present. Does not change task routing. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_EXPERIMENTAL` | `populi_remote_execute_experimental` (TOML alias: `mesh_remote_execute_experimental`) — enables staged rollout for remote task-envelope dispatch over populi A2A relay (with local fallback). |
| `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATING_ENABLED` | `populi_remote_lease_gating_enabled` (TOML: `mesh_remote_lease_gating_enabled`) — when true with matching roles, relay is **awaited** before local enqueue; success puts the task in **remote-hold** (single owner, no local dequeue). Relay failure **deterministically** falls back to local queue only (no fire-and-forget duplicate relay). |
| `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATED_ROLES` | `populi_remote_lease_gated_roles` — comma-separated `planner`, `builder`, `verifier`, `reproducer`, `researcher` (case-insensitive). Empty list means no task matches gating. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_POLL_INTERVAL_SECS` | `populi_remote_result_poll_interval_secs` (TOML alias: `mesh_remote_result_poll_interval_secs`) — **`remote_task_result`** inbox poll interval in seconds; **`0`** disables. Implemented in `vox_orchestrator::a2a::spawn_populi_remote_result_poller` (MCP and other embedders pass a join slot). |
| `VOX_ORCHESTRATOR_MESH_REMOTE_WORKER_POLL_INTERVAL_SECS` | `populi_remote_worker_poll_interval_secs` (TOML alias: `mesh_remote_worker_poll_interval_secs`) — **`remote_task_envelope`** worker poll interval in seconds; **`0`** disables remote worker consumption while keeping result polling optional. Implemented in `vox_orchestrator::a2a::spawn_populi_remote_worker_poller`. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_MAX_MESSAGES_PER_POLL` | `populi_remote_result_max_messages_per_poll` — **per-page** size when draining the parent mesh inbox for `remote_task_result` rows (minimum 1; default 64). The poller walks **cursor pages** (`before_message_id`, newest-first) up to a fixed cap so deep inboxes do not hide older results behind unrelated A2A mail. |

Populi client helpers now expose typed HTTP status errors (`PopuliRegistryError::HttpStatus`) and non-claimer inbox cursor paging (`before_message_id`, plus `A2AInboxPager`), so orchestrator fallback logic can branch on status codes (`403/404/409`) without brittle string matching.

### Placement and lease observability

**Implemented:** `task_id`, `agent_id`, and `placement_reason` are emitted as structured fields on the terminal `tracing::info!` event in `task_dispatch/submit/task_submit.rs` for every task routing decision. `lease_id` is carried in `PopuliRemoteDelegate` on the task record and forwarded in the A2A envelope when a lease is active.

| Field / concept | Purpose |
| --- | --- |
| `task_id` | Correlate orchestrator task lifecycle across logs and traces. |
| `lease_id` | Carried in `PopuliRemoteDelegate.exec_lease_id` on lease-held tasks; propagated in `RemoteTaskEnvelope` and `ExecLeaseGrantResponse`. |
| `placement_reason` | Machine-readable code for the selected execution surface (local vs lease-remote); emitted on every routing decision. |
| `populi_node_id` / `claimer_node_id` | Mesh identity for inbox claims and execution attribution where applicable. |

Stable `placement_reason` codes:

- `local_queue_default`
- `populi_remote_lease_hold`
- `local_queue_fallback_after_remote_relay_error`
- `local_queue_fallback_insufficient_vram` — no registered node meets the task's `min_vram_mb` requirement

Rollout and kill switches: [Populi remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md). Work-type boundaries: [placement policy matrix](populi-work-type-placement-matrix.md).

### Other CLI / data plane

Canonical descriptions for **`VOX_BENCHMARK_TELEMETRY`** / **`VOX_SYNTAX_K_TELEMETRY`** (and related Codex row shapes) live in [env-vars.md](env-vars.md). Trust boundaries for optional telemetry: [telemetry-trust-ssot](../architecture/telemetry-trust-ssot.md).

| Variable | Purpose |
| -------- | ------- |
| `VOX_BENCHMARK_TELEMETRY` | When `1` / `true`, CLI benchmark entry points append `benchmark_event` rows via `VoxDb::record_benchmark_event`. |
| `VOX_SYNTAX_K_TELEMETRY` | When `1` / `true`, syntax-K benchmark classes append `syntax_k_event` rows via `VoxDb::record_syntax_k_event` (session `syntaxk:<repository_id>`). If unset, falls back to `VOX_BENCHMARK_TELEMETRY`. |
| `VOX_WORKFLOW_JOURNAL_CODEX_OFF` | When `1` / `true`, skip Codex append for interpreted workflow journal rows. By default, when DB config resolves after `vox workflow run` / `vox mens workflow run` ( **`workflow-runtime`** ), Vox appends versioned workflow journal rows via `VoxDb::record_workflow_journal_entry` (session `workflow:<repository_id>`, metric `workflow_journal_entry`). Rows can include lifecycle events, retry events (`ActivityAttemptRecovered`, `ActivityAttemptFailed`, `ActivityRetryScheduled`), replay events, and per-step payloads (for example `MeshActivity` / `MeshActivitySkipped`) keyed by durable **`run_id`** + **`activity_id`** semantics described in [durable execution](../explanation/expl-durable-execution.md). |
| `VOX_MESH_MAX_STALE_MS` | Client-side filter for mens node lists in MCP snapshots (see [mens SSOT](populi.md)). |
| `VOX_MESH_CODEX_TELEMETRY` | When `1` / `true`, append `populi_control_event` rows via `VoxDb::record_populi_control_event` (session `mens:<repository_id>`): after **`vox run`** local registry publish when the CLI was built with **`populi`** (includes `vox-populi`), after **`vox-mcp`** startup publish when mens is enabled, and after MCP **`vox_orchestrator_status`** mens HTTP snapshot when Codex is connected. Implementation: [`vox_db::populi_registry_telemetry`](../../../crates/vox-db/src/populi_registry_telemetry.rs). **Never** stores `VOX_MESH_TOKEN`. |
| `VOX_MCP_LLM_COST_EVENTS` | Optional override for MCP LLM [`CostIncurred`](../../../crates/vox-orchestrator/src/events.rs) bus events vs Codex-only accounting; see [Model Routing](../how-to/how-to-model-routing.md). |
| `VOX_REPOSITORY_ROOT` | Optional directory for `repository_id` discovery in benchmark telemetry (and other CLI paths that adopt the same pattern); align with MCP’s discovered repo root when subprocess CWD differs. |

**TOML:** under `[orchestrator]`, set `orchestration_migration = { orchestration_v2_enabled = true, … }` (field names match `OrchestrationMigrationFlags` in `crates/vox-orchestrator/src/contract.rs`). When v2 is enabled, MCP `vox_submit_task` success JSON may include **`orchestration_contract` { `"v2"`** as a client hint.

Optional **`[mens]`** in `Vox.toml` merges mens scope/URL/labels for CLI and MCP (see [mens SSOT](populi.md)); **env wins** per field when set.

Effective Socrates thresholds still merge from `vox-socrates-policy` with optional overrides in `OrchestratorConfig::socrates_policy` — no literal drift outside the policy crate + merge logic.

## Deprecation / compatibility matrix (current)

| Surface | Rule |
| ------- | ---- |
| MCP tool names | Add aliases before removing names; `vox_plan`, `vox_replan`, `vox_plan_status` stay stable. |
| DeI RPC ids | `ai.plan.*` method strings unchanged (`vox_cli::dei_daemon::method`). |
| Orchestrator daemon RPC ids | `orch.*` method strings are versioned in `vox_protocol::orch_daemon_method`; contract schema [`contracts/orchestration/orch-daemon-rpc-methods.schema.json`](../../../contracts/orchestration/orch-daemon-rpc-methods.schema.json). |
| File sessions + Codex | Both remain valid; MCP `SessionManager` uses `with_db` when Codex is attached. |
| `vox db` | Remains implementation SSOT; `vox scientia` is a documented facade only. |

## Related docs

- [ADR 017: Populi lease-based remote execution](../adr/017-populi-lease-remote-execution.md) — **Accepted (implemented)**; lease-based authoritative remote execution contract.
- [ADR 018: Populi GPU truth layering](../adr/018-populi-gpu-truth-layering.md) — verified inventory vs labels.
- [Populi work-type placement matrix](populi-work-type-placement-matrix.md) — local / LAN / overlay policy.
- [`external-repositories.md`](external-repositories.md) — `repository_id`, sessions, cache layout.
- [`socrates-protocol.md`](socrates-protocol.md) — Socrates telemetry and policy.
- [`mens-training.md`](mens-training.md) — training backends and env.

