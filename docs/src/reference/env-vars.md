---
title: "Environment variables (SSOT)"
description: "Official documentation for Environment variables (SSOT) for the Vox language. Detailed technical reference, architecture guides, and impl"
category: "Language Reference"
last_updated: "2026-04-11"
training_eligible: true

schema_type: "TechArticle"
---

# Environment variables (SSOT)

Canonical names and precedence for tooling that spans CLI, MCP, orchestrator, and Codex. **Implementations** live in the crates cited below; update this page when adding or renaming variables.

## Codex / Turso (`vox-db`, `vox-package`)

| Variable | Role |
|----------|------|
| `VOX_DB_URL` | Remote libSQL / Turso URL (with `VOX_DB_TOKEN`). |
| `VOX_DB_TOKEN` | Auth token for `VOX_DB_URL`. |
| `VOX_DB_PATH` | Local database file path (`local` / replication features). |
| `VOX_SECRETS_HARD_CUT` | When truthy, disables `VOX_TURSO_*` / `TURSO_*` compatibility alias fallback in DB config resolution. |
| `VOX_SECRETS_PROFILE` | Secrets resolution strictness profile: `dev` (default), `ci`, `prod`, or `hard_cut`. Strict profiles reject deprecated aliases and source-policy violations. |
| `VOX_SECRETS_BACKEND` | Secrets backend selector: `auto` (default), `env_only`, `infisical`, `vault`, `vox_cloud`. |
| `VOX_SECRETS_AUTO_PREFER_VAULT` | When `1`/`true`/`yes`, forces `BackendMode::Auto` to select the `vox_cloud` cloudless vault backend even if explicit vault URLs/commands are absent. |
| `VOX_SECRETS_AUTO_VAULT` | Explicit hint to enable the `vox_cloud` vault backend in `Auto` mode; lighter than `PREFER_VAULT` (it just signals presence, doesn't force precedence over explicit backends). |
| `VOX_SECRETS_CUTOVER_PHASE` | Cloudless rollout choreography: `shadow` -> `canary` -> `enforce` -> `decommission`. `shadow` allows legacy sources, `canary` blocks legacy sources in strict profiles, `enforce` blocks legacy sources for all profiles, `decommission` also forces `vox_cloud` backend resolution. |
| `VOX_SECRETS_MIGRATION_PHASE` | Compatibility alias for `VOX_SECRETS_CUTOVER_PHASE`; same values and semantics. |
| `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | > [!WARNING] DEPRECATED<br>**Compatibility** aliases read after canonical `VOX_DB_*` fails in [`DbConfig::resolve_standalone`](../../../crates/vox-db/src/config.rs). In Cloudless hard-cut strict profiles, these aliases are scheduled for rejection by source policy. |
| `TURSO_URL` / `TURSO_AUTH_TOKEN` | > [!WARNING] DEPRECATED<br>**Legacy** Turso env names; same compatibility tier as `VOX_TURSO_*`. In Cloudless hard-cut strict profiles, these legacy aliases are scheduled for rejection by source policy. |
| `VOX_EMBEDDING_SEARCH_CANDIDATE_MULT` | Integer ≥ 1: multiplier for brute-force embedding search window (`limit * mult`, capped). See [`capabilities`](../../../crates/vox-db/src/capabilities.rs). |
| `VOX_WORKSPACE_JOURNEY_STORE` | Repo-backed **interactive** surfaces (`vox-mcp`, `vox-orchestrator-d`): `project` (default) uses `.vox/store.db` under the discovered repo root; `canonical` uses user-global / `VOX_DB_URL` Codex. See [`workspace_journey_store`](../../../crates/vox-db/src/workspace_journey_store.rs). |
| `VOX_WORKSPACE_JOURNEY_FALLBACK_CANONICAL` | When `project` open fails, allow fallback to [`connect_canonical_optional`](../../../crates/vox-db/src/connect_policy.rs) (default **on**); set `0`/`false` to stay strictly local. Applies to MCP, `vox-orchestrator-d`, and repo-scoped CLI (`vox agent`, `vox snippet`, `vox share`, … via [`workspace_db::connect_cli_workspace_voxdb`](../../../crates/vox-cli/src/workspace_db.rs)). |
| `vox-db` / **`replication`** feature | Cargo feature enabling Turso embedded-replica connect paths (`vox-package` exposes `replication = ["vox-db/replication"]`). Pair with [`VoxDb::sync`](../../../crates/vox-db/src/store/open.rs) / [`ReadConsistency::ReplicaLatest`](../../../crates/vox-db/src/lib.rs) before reads that need fresher remote state. |
| `VOX_DB_MVCC` | Codex MVCC transaction mode override for VoxDb read environments. |

**Precedence (remote):** `VOX_DB_URL`+`VOX_DB_TOKEN` → `VOX_TURSO_*` → `TURSO_*`. **Project VoxDb** (operational store + snippets/share) uses [`DbConfig::resolve_project_code_store_config`](../../../crates/vox-db/src/config.rs): empty env maps to the project-relative default store path, not the user-data default.

See [ADR 004: Codex / Arca / Turso](../adr/004-codex-arca-turso-ssot.md).

### Secrets cloudless vault vs Codex (two SQL surfaces)

| Plane | Purpose | Canonical env |
|-------|---------|---------------|
| **Codex** (`vox-db`) | Product relational data: sessions, memory tables, telemetry rows, gamification, etc. | `VOX_DB_URL` + `VOX_DB_TOKEN`, or `VOX_DB_PATH`, plus workspace journey vars above. |
| **Secrets vault** (`vox-secrets` cloudless backend) | Encrypted secret material at rest in a **separate** SQLite / libSQL database. | See vault vars below. |

**Vault URL / file (precedence):** `VOX_SECRETS_VAULT_PATH` (local path → `file:` URL) → `VOX_SECRETS_VAULT_URL` → `VOX_SECRETS_AUTO_VAULT` / `VOX_SECRETS_AUTO_PREFER_VAULT` → when compat aliases allowed (`VOX_SECRETS_HARD_CUT` off and cutover phase not `enforce`/`decommission`): `VOX_TURSO_URL` → `TURSO_URL` → default `file:.vox/secrets_vault.db`.

**Vault remote token (precedence):** `VOX_SECRETS_VAULT_TOKEN` → compat `VOX_TURSO_TOKEN` → `TURSO_AUTH_TOKEN` (same gating as URL aliases).

| Variable | Role |
|----------|------|
| `VOX_SECRETS_VAULT_PATH` | Local vault SQLite path; opened as `file:` (preferred for repo-local vaults). |
| `VOX_SECRETS_VAULT_URL` | Explicit vault URL (`file:…` or `libsql://…`). |
| `VOX_SECRETS_VAULT_TOKEN` | Auth token when `VOX_SECRETS_VAULT_URL` is remote. |
| `VOX_TURSO_URL` / `VOX_TURSO_TOKEN` | > [!WARNING] DEPRECATED for vault<br>Read only when compat aliases allowed; migrate to `VOX_SECRETS_VAULT_*`. |
| `TURSO_URL` / `TURSO_AUTH_TOKEN` | > [!WARNING] DEPRECATED<br>Same compatibility tier as `VOX_TURSO_*` for the vault plane. |

Do not point Codex and the vault at the same file unless you have an explicit ops reason. Codex compatibility shims live in [`DbConfig`](../../../crates/vox-db/src/config.rs); vault resolution lives in [`vox_vault`](../../../crates/vox-secrets/src/backend/vox_vault.rs). Run `vox secrets doctor` to print `cloudless_vault_store` diagnostics (redacted).

## Ludus (`vox-gamify`, `vox ludus`)

| Variable | Role |
|----------|------|
| `VOX_LUDUS_EMERGENCY_OFF` | When `1`/`true`/`yes`, hard-disables all Ludus side effects (rewards, teaching DB writes, overlays). See [`config_gate`](../../../crates/vox-gamify/src/config_gate.rs). |
| `VOX_LUDUS_SESSION_ENABLED` | Session-only override: `true` / `false` toggles `gamify_enabled` without touching on-disk config. |
| `VOX_LUDUS_SESSION_MODE` | `balanced` \| `serious` \| `learning` \| `off` (`off` disables for the session). |
| `VOX_LUDUS_VERBOSITY` | `quiet` \| `normal` \| `rich` — CLI celebration / overlay verbosity. See [`output_policy`](../../../crates/vox-gamify/src/output_policy.rs). |
| `VOX_LUDUS_MAX_MESSAGES_PER_HOUR` | Cap on bursty Ludus CLI messages per rolling hour (default `12`). |
| `VOX_LUDUS_CHANNEL` | UX channel override: `off` \| `serious` \| `balanced` \| `digest-priority` (also `digest` / `digest_priority`). When unset, derived from [`GamifyMode`](../../../crates/vox-config/). `digest-priority` suppresses inline CLI celebrations; use `vox ludus digest-weekly` for summaries. |
| `VOX_LUDUS_EXPERIMENT` | When non-empty: appended to `gamify_policy_snapshots.mode_label`, and scales teaching hint frequency (deterministic A/B multiplier from the string). |
| `VOX_LUDUS_MCP_TOOL_ARGS` | How MCP tool call `args` are stored in routed Ludus events: `full` (default) \| `hash` \| `omit` (see [`mcp_privacy`](../../../crates/vox-gamify/src/mcp_privacy.rs), [`config_gate`](../../../crates/vox-gamify/src/config_gate.rs)). |
| `VOX_LUDUS_EXPERIMENT_REWARD_MULT` | When set to a finite positive number (e.g. `1.1`), multiplies policy XP/crystal rewards in addition to mode + streak (Ludus experiment branch); unset keeps prior behavior. |
| `VOX_LSP_LUDUS_EVENTS` | When `0`/`false`/`off`, disables Ludus `diagnostics_clean` emission from `vox-lsp` (project Codex must still open successfully). |
| `VOX_LUDUS_ROUTE_LOG_SAMPLE` | Optional integer **N** ≥ 1: log roughly **1/N** `route_event` calls at `INFO` (`target = vox_gamify::route_event`) using a deterministic hash (user id + event type). |

## Repository root (`vox-repository`, `vox ci`)

| Variable | Role |
|----------|------|
| `VOX_REPO_ROOT` | Absolute or normalized path to the logical repo root for **`vox ci`**, doc-inventory, **`vox upgrade --source repo`** (when **`--repo-root`** is omitted), and other tools that must not depend on cwd alone. |
| `VOX_REPOSITORY_ROOT` | Compatibility alias read **before** `VOX_REPO_ROOT` in some tools ([`lineage`](../../../crates/vox-orchestrator/src/lineage.rs), TOESTUB/MCP/repo-id probes). Prefer `VOX_REPO_ROOT`; set both only if tooling disagrees. |

## User data directory (`vox-config`)

| Variable | Role |
|----------|------|
| `VOX_DATA_DIR` | Absolute path overriding the platform default Vox **data directory** (configs, canonical local store parent, etc.). See [`resolve_vox_data_dir`](../../../crates/vox-config/src/paths.rs). |

## Toolchain self-update (`vox upgrade`)

| Variable | Role |
|----------|------|
| `VOX_UPGRADE_PROVIDER` | `github` (default) or `http` — override release backend when not passing **`--provider`**. (`gitlab` is **deprecated/unsupported** as of 2026-06-03.) |
| `VOX_UPGRADE_REPO` | `owner/repo` (GitHub). Default upstream: **`vox-foundation/vox`**. |
| `VOX_UPGRADE_BASE_URL` | For **`http`**: base URL such as `https://github.com/org/repo/releases` (requires **`--version`** or **`VOX_UPGRADE_VERSION`**). |
| `VOX_UPGRADE_VERSION` | Pinned tag for **`http`** mirror when omitted on the CLI. |
| `VOX_UPGRADE_GITLAB_HOST` | **DEPRECATED (2026-06-03, unsupported):** GitLab API root (default `https://gitlab.com`) for the retired GitLab release backend. |
| `VOX_UPGRADE_GITHUB_API_URL` | GitHub API base (Enterprise), e.g. `https://github.example.com/api/v3`. |
| `GITHUB_TOKEN` / `GH_TOKEN` / `VOX_GITHUB_TOKEN` | Optional; raises GitHub API rate limits and enables **private** release assets. |
| `GITLAB_TOKEN` / `VOX_GITLAB_TOKEN` | **DEPRECATED (2026-06-03, unsupported):** GitLab **private-token** style access for the retired GitLab release/forge backend. |
| `CARGO` | Optional: path to the **`cargo`** executable for **`vox upgrade --source repo --apply`** (defaults to **`cargo`** on `PATH`). |

## Orchestrator (`vox-orchestrator`)

| Variable | Role |
|----------|------|
| `VOX_ORCHESTRATOR_DAEMON_SOCKET` | **Dual role (different processes):** (1) **`vox-orchestrator-d`** — TCP **bind** (`127.0.0.1:9745`, optional `tcp://` prefix) or **`stdio`** / **`-`** / **`stdin`** for newline JSON-RPC on stdin/stdout. (2) **`vox-mcp`** — optional **TCP peer** for `orch.ping` at startup (stdio transport skipped); compares `repository_id` from ping with the MCP embed’s repo id (**WARN** on mismatch, **ERROR** if **`VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT`** is truthy). MCP still embeds `Orchestrator` until ADR 022 Phase B IPC-first parity. |
| `VOX_ORCHESTRATOR_ENABLED` | Enable/disable orchestrator. |
| `VOX_ORCHESTRATOR_MAX_AGENTS` | Cap on concurrent agents. |
| `VOX_ORCHESTRATOR_LOCK_TIMEOUT_MS` | File lock TTL. |
| `VOX_ORCHESTRATOR_TOESTUB_GATE` | TOESTUB post-task gate. |
| `VOX_ORCHESTRATOR_MAX_DEBUG_ITERATIONS` | Re-route cap on validation failures. |
| `VOX_ORCHESTRATOR_SOCRATES_GATE_SHADOW` | Log Socrates decisions without blocking. |
| `VOX_ORCHESTRATOR_SOCRATES_GATE_ENFORCE` | Requeue on risky Socrates outcome. |
| `VOX_ORCHESTRATOR_SOCRATES_REPUTATION_ROUTING` | Blend Arca `agent_reliability` into routing. |
| `VOX_ORCHESTRATOR_SOCRATES_REPUTATION_WEIGHT` | Weight for reliability blend (default in config: `1.0`). |
| `VOX_ORCHESTRATOR_TRUST_GATE_RELAX_ENABLED` | When `true`, high **`agent_reliability`** relaxes **Socrates enforce**, **completion grounding enforce**, and **strict scope** (threshold: next row). |
| `VOX_ORCHESTRATOR_TRUST_GATE_RELAX_MIN_RELIABILITY` | Minimum reliability in `[0,1]` for the relax path (default **`0.85`** in config). |
| `VOX_ORCHESTRATOR_LOG_LEVEL` | Tracing/log level string. |
| `VOX_ORCHESTRATOR_FALLBACK_SINGLE` | Ambiguous routing → single agent. |
| `VOX_ORCHESTRATOR_MESH_CONTROL_URL` | Base URL of the mens HTTP control plane for **read-only** node snapshots in MCP/orchestrator (e.g. `http://mens-ctrl:9847`). See [mens SSOT](populi.md), [deployment compose SSOT](deployment-compose.md). |
| `VOX_ORCHESTRATOR_MESH_POLL_INTERVAL_SECS` | Poll interval for mens HTTP client (see [`OrchestratorConfig::merge_env_overrides`](../../../crates/vox-orchestrator/src/config/mod.rs)). |
| `VOX_A2A_CONSUMER_ID` | Override the **claim owner** string for [`VoxDb::poll_a2a_inbox`](../../../crates/vox-db/src/store/ops_a2a.rs) (default `pid:<process_id>`). |
| `VOX_ORCH_LINEAGE_OFF` | When `1` / `true` / `yes`, skips append-only `orchestration_lineage_events` writes from the orchestrator (rollback toggle). |
| `VOX_ORCH_CAMPAIGN_ID` | Optional opaque string (trimmed) stored in select lineage payloads (`plan_session_created`, workflow handoff, replan, etc.) -> group runs across `plan_session_id` values. |
| `VOX_WORKFLOW_JOURNAL_CODEX_OFF` | When `1` / `true` / `yes`, skips Codex persistence for interpreted workflow journals after `vox mens workflow run` (see [`workflow_journal_codex`](../../../crates/vox-cli/src/workflow_journal_codex.rs)). |
| `VOX_DB_CIRCUIT_BREAKER` | When enabled in [`DbCircuitBreaker::from_env`](../../../crates/vox-db/src/circuit_breaker.rs), gates selected Turso writes (locks, heartbeats, lineage, CAS, sessions, LLM logs, `agent_events`, Codex skills + **`chat_*`** user chat / usage / topics, generic `actor_state`, registry preference wipe, research ingest + capability map, `populi_training_run`, legacy JSONL data rows + `legacy_import_extras`, TOESTUB persistence, schemaless `Collection` document writes, agent memory/knowledge/search/embeddings, publication + scholarly/external jobs + planning + news + mens cloud + questioning, Ludus `gamify_*` / A2A / oplog / Ludus `actor_state`, learning + workflow journal + retention deletes + MCP chat transcripts, build observability + `components` — see `circuit_breaker.rs`). |
| `VOX_DB_SYNC_INTEGRATION` | Set to `1` with remote URL+token to enable the opt-in [`sync_for(ReplicaLatest)`](../../../crates/vox-db/src/store/open.rs) integration test (`vox-db` `sync_remote_integration.rs`). |
| `VOX_DB_EMBEDDED_REPLICA_INTEGRATION` | Set to `1` with URL+token to run the opt-in embedded-replica test (`cargo test -p vox-db --features replication sync_embedded_replica_smoke`). |
| `VOX_ORCHESTRATOR_MESH_HTTP_TIMEOUT_MS` | HTTP timeout for mens control-plane requests. |
| `VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL` | Experimental routing hooks (see [mens SSOT](populi.md)). |
| `VOX_ORCHESTRATOR_MESH_REBALANCE_ON_REMOTE_SCHEDULABLE_DROP` | When `1` / `true` **and** experimental routing is on, if the embedder refresh reports **fewer** federation-schedulable remote nodes than the previous snapshot, the orchestrator runs **[`Orchestrator::rebalance`](../../../crates/vox-orchestrator/src/orchestrator/scaling.rs)** once (local queue work-steering only; does **not** replay full routing for each queued task). Traces: `decision = populi_remote_schedulable_decreased`, `populi_remote_drop_load_rebalance` / `populi_remote_drop_load_rebalance_noop` (`target: vox.orchestrator.routing`). |
| `VOX_ORCHESTRATOR_MESH_REPLAY_QUEUED_ROUTES_ON_REMOTE_SCHEDULABLE_DROP` | When `1` / `true` **and** **`VOX_ORCHESTRATOR_MESH_ROUTING_EXPERIMENTAL`** is on, if federation-schedulable remote **count drops**, re-runs **[`Orchestrator::resolve_route`](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/batch.rs)** for each **queued** task (skips in-progress and Populi-delegated tasks) and moves tasks when the chosen agent changes. Runs after optional rebalance when that flag is also set. Traces: `decision = populi_remote_drop_queued_route_replay` (`target: vox.orchestrator.routing`), `queued_route_replay_move` (`target: vox.orchestrator.placement`). |
| `VOX_ORCHESTRATOR_MESH_EXEC_LEASE_RECONCILE` | When `1` / `true`, each successful mens node poll ([`VOX_ORCHESTRATOR_MESH_POLL_INTERVAL_SECS`], [`mesh_federation_poll`](../../../crates/vox-orchestrator/src/mesh_federation_poll.rs) in **`vox-mcp`** and **`vox-orchestrator-d`**) also calls **`GET /v1/populi/exec/leases`** and logs **warn**/**debug** (`target: vox.mcp.populi_reconcile`) when a lease holder is missing, heartbeat-stale (vs orchestrator **`stale_threshold_ms`**), in effective maintenance, quarantined, or (GPU-capable node) **`gpu_readiness_ok=false`**. With **`VOX_MESH_CODEX_TELEMETRY`**, emits **`mesh_exec_lease_reconcile`** via Codex (`record_populi_control_event`; details include **`auto_revoke_attempted`** / **`auto_revoke_ok`** when **`VOX_ORCHESTRATOR_MESH_EXEC_LEASE_AUTO_REVOKE`** is set (next row). |
| `VOX_ORCHESTRATOR_MESH_EXEC_LEASE_AUTO_REVOKE` | When `1` / `true` **and** reconcile is enabled, after each bad-holder diagnosis MCP calls **`POST /v1/populi/admin/exec-lease/revoke`** for that **`lease_id`** (requires mesh/admin bearer on the HTTP client — same token path as lease list). **Dangerous** when holders are only briefly stale or in cooperative maintenance; prefer manual revoke unless you accept freeing **`scope_key`** aggressively. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_WORKER_POLL_INTERVAL_SECS` | Poll interval for consuming `remote_task_envelope` rows in remote worker mode (`0` disables). |
| `VOX_ORCHESTRATOR_MESH_TRAINING_ROUTING_EXPERIMENTAL` | Enables training-task-specific scoring boosts/penalties in local routing. |
| `VOX_ORCHESTRATOR_MESH_TRAINING_BUDGET_PRESSURE` | Soft scalar (`0.0-1.0`) -> reduce expensive training placements under budget pressure. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_EXPERIMENTAL` | When `1`/`true`, enables [`RemoteTaskEnvelope`](../../../crates/vox-orchestrator/src/a2a/envelope.rs) relay over populi A2A. Without lease gating, relay runs **after** local enqueue (local execution can still run in parallel — legacy path). |
| `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATING_ENABLED` | When `1`/`true` with **`VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATED_ROLES`**, matching tasks use **single-owner** semantics: awaited relay, then **remote-hold** (no local dequeue) or **local-only** fallback if relay fails. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_LEASE_GATED_ROLES` | Comma-separated execution roles: `planner`, `builder`, `verifier`, `reproducer`, `researcher`. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_RECEIVER_AGENT` | Destination **numeric** A2A agent id (string form) for experimental remote relay. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_EXECUTE_SENDER_AGENT` | Originator agent id for relay (defaults to `1` when unset/invalid). |
| `VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_POLL_INTERVAL_SECS` | When experimental remote execute is on, polls populi A2A inbox for **`remote_task_result`** on this interval (default **5**). **`0`** disables. Uses `vox_orchestrator::a2a::spawn_populi_remote_result_poller` (not MCP-only). Independent of **`VOX_ORCHESTRATOR_MESH_POLL_INTERVAL_SECS`**. |
| `VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_MAX_MESSAGES_PER_POLL` | **Per-page** row cap when draining the parent mesh inbox for `remote_task_result` (default **64**, minimum **1**). The drain walks cursor pages (`before_message_id`) so deep inboxes do not hide older results. Maps to `OrchestratorConfig::populi_remote_result_max_messages_per_poll`. |
| `VOX_PLAN_SESSION_ID` / `VOX_PLAN_NODE_ID` / `VOX_PLAN_VERSION` | Optional planning-context correlation fields for interpreted workflow runners (`vox mens workflow run`); when set, durable `workflow_run_log` rows attach orchestrator plan provenance. |
| `VOX_ORCHESTRATOR_MIN_AGENTS` / `SCALING_*` / `COST_PREFERENCE` / `RESOURCE_*` | Scaling and economy knobs — see [`OrchestratorConfig::merge_env_overrides`](../../../crates/vox-orchestrator/src/config/mod.rs). |

**Populi placement / lease observability (roadmap):** stable **`task_id`**, **`lease_id`**, and **`placement_reason`**-style fields are specified as a documentation contract in [unified orchestration — placement observability](orchestration-unified.md#placement-and-lease-observability). Rollout kill switches: [Populi remote execution rollout checklist](../operations/populi-remote-execution-rollout-checklist.md).
| `VOX_ORCHESTRATOR_ATTENTION_ENABLED` / `VOX_ORCHESTRATOR_ATTENTION_BUDGET_MS` / `VOX_ORCHESTRATOR_ATTENTION_ALERT_THRESHOLD` / `VOX_ORCHESTRATOR_ATTENTION_INTERRUPT_COST_MS` / `VOX_ORCHESTRATOR_ATTENTION_TRUST_ROUTING_WEIGHT` | Attention-budget controls for orchestrator routing, **dynamic clarification deferral** (MCP questioning path when enabled), MCP **LLM infer** pre-check (orchestrator budget snapshot), `vox_submit_task`/`vox_a2a_send` policy gating, and planning-surface deferral when budget pressure is high. Implementation: [`evaluate_interruption`](../../../crates/vox-orchestrator/src/attention/interruption_policy.rs), [`BudgetGate::check_attention_snapshot`](../../../crates/vox-orchestrator/src/gate.rs). |
| `VOX_ORCHESTRATOR_CHATML_STRICT` | Enables stricter ChatML guardrails in orchestrator request shaping. |
| `VOX_ORCHESTRATOR_MAX_TOESTUB_DEBUG_ITERATIONS` / `VOX_ORCHESTRATOR_MAX_SOCRATES_DEBUG_ITERATIONS` | Specialized retry/debug iteration caps for TOESTUB and Socrates re-routing flows. |
| `VOX_ORCHESTRATOR_SCALING_THRESHOLD` / `VOX_ORCHESTRATOR_SCALING_ENABLED` / `VOX_ORCHESTRATOR_SCALING_LOOKBACK` / `VOX_ORCHESTRATOR_SCALING_PROFILE` / `VOX_ORCHESTRATOR_SCALING_COOLDOWN_MS` / `VOX_ORCHESTRATOR_MAX_SPAWN_PER_TICK` / `VOX_ORCHESTRATOR_URGENT_REBALANCE_THRESHOLD` | Scaling-control set used by adaptive fleet sizing and rebalancing. |
| `VOX_ORCHESTRATOR_IDLE_RETIREMENT_MS` | Idle retirement timeout for agent lifecycle contraction. |
| `VOX_ORCHESTRATOR_COST_PREFERENCE` / `VOX_ORCHESTRATOR_RESOURCE_WEIGHT` / `VOX_ORCHESTRATOR_RESOURCE_CPU_MULT` / `VOX_ORCHESTRATOR_RESOURCE_MEM_MULT` / `VOX_ORCHESTRATOR_RESOURCE_EXPONENT` | Cost-vs-performance and resource-bias routing parameters. |
| `VOX_ORCHESTRATOR_PLANNING_ENABLED` / `VOX_ORCHESTRATOR_PLANNING_ROUTER_ENABLED` / `VOX_ORCHESTRATOR_PLANNING_REPLAN_ENABLED` / `VOX_ORCHESTRATOR_PLAN_LLM_SYNTHESIS` / `VOX_ORCHESTRATOR_PLANNING_WORKFLOW_HANDOFF_ENABLED` / `VOX_ORCHESTRATOR_PLANNING_SHADOW_MODE` / `VOX_ORCHESTRATOR_PLANNING_AUTO_MODE_ENABLED` / `VOX_ORCHESTRATOR_PLANNING_ROLLOUT_PERCENT` / `VOX_ORCHESTRATOR_PLAN_ADEQUACY_SHADOW` / `VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE` | Planning-mode rollout and behavior controls; `VOX_ORCHESTRATOR_PLAN_ADEQUACY_SHADOW` (default on) keeps native plan adequacy as lineage/telemetry only; `VOX_ORCHESTRATOR_PLAN_ADEQUACY_ENFORCE` rejects native enqueue and MCP `vox_plan` success when the plan stays thin after refinement. See [plan adequacy](../archive/research-2026-q1/plan-adequacy.md). |
| `VOX_ORCHESTRATOR_RESEARCH_MODEL_ENABLED` | Enables the research-model branch in orchestrator planning env merges ([`OrchestratorConfig::merge_env_overrides`](../../../crates/vox-orchestrator/src/config/impl_env.rs)). |
| `VOX_ORCHESTRATOR_CONTEXT_LIFECYCLE_SHADOW` / `VOX_ORCHESTRATOR_CONTEXT_LIFECYCLE_ENFORCE` | Context envelope lifecycle policy for cross-surface `ContextEnvelope` JSON ingress (MCP `vox_submit_task` / `context_envelope_json`, gamify handoff, orchestrator session attach). Defaults off. **Shadow** logs validation violations without blocking and, on successful validation, emits structured tracing `event=context.capture` (ingest: source, envelope ids, merge strategy, trace/correlation ids; target `vox_orchestrator::context_lifecycle`). Session merges log `event=context.select` with merge `outcome` when shadow is on. Collector field shapes: [`contracts/orchestration/context-lifecycle-telemetry.schema.json`](../../../contracts/orchestration/context-lifecycle-telemetry.schema.json). **Enforce** rejects invalid envelopes, expired/stale payloads, repository/session mismatches, and merge failures (for example `ManualReview` when a session envelope already exists). Trust SSOT: [telemetry-trust-ssot](../architecture/telemetry-trust-ssot.md). |
| `VOX_ORCHESTRATOR_COMPLETION_GROUNDING_SHADOW` / `VOX_ORCHESTRATOR_COMPLETION_GROUNDING_ENFORCE` | Completion citation grounding: `vox_complete_task` may include `evidence_citations` and/or `[[voxcite:REF]]` markers in `completion_summary`. **Shadow** logs when declared refs are missing from the session context envelope. **Enforce** requeues the task (same retry budget as the Socrates gate) until citations match envelope text. Matching declarations raise the effective Socrates `evidence_count` used by the gate. |
| `VOX_ORCHESTRATOR_COMPLETION_MARKDOWN_LINK_AUDIT_ENABLED` | When truthy, task completion runs `vox ci check-links` for each `.md` path in the task write manifest (maps to [`OrchestratorConfig::completion_markdown_link_audit_enabled`](../../../crates/vox-orchestrator/src/config/orchestrator_fields.rs); disabled in [`OrchestratorConfig::for_testing`](../../../crates/vox-orchestrator/src/config/impl_env.rs)). |
| `VOX_ORCHESTRATOR_STRESS_DEBUG` | **Stress integration tests only** — not read by [`OrchestratorConfig::merge_env_overrides`](../../../crates/vox-orchestrator/src/config/impl_env.rs). Resolved via Secrets/env; when truthy (`1`, `true`, `yes`, `y`, `on`, case-insensitive), [`stress_test.rs`](../../../crates/vox-orchestrator/tests/stress_test.rs) prints periodic drain progress to stderr for hang diagnosis. |
| `VOX_ORCHESTRATOR_MIGRATION_V2_ENABLED` / `VOX_ORCHESTRATOR_MIGRATION_LEGACY_FALLBACK` | Migration controls for orchestrator V2 rollout and fallback behavior. |
| `VOX_ORCHESTRATOR_TRUST_EWMA_ALPHA` / `VOX_ORCHESTRATOR_TRUST_PROVISIONAL_THRESHOLD` / `VOX_ORCHESTRATOR_TRUST_TRUSTED_THRESHOLD` / `VOX_ORCHESTRATOR_TRUST_AUTO_APPROVE_MIN` | Trust-score smoothing and threshold controls used by trust-aware routing/autonomy. |
| `VOX_ORCHESTRATOR_REPO_SHARD_SPECIALIZATION_WEIGHT` / `VOX_ORCHESTRATOR_REPO_SHARD_VALIDATION_FAILURE_PENALTY` / `VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_PENALTY` / `VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_MS` | Repo-sharding specialization/penalty weights and conflict-cooldown knobs. |
| `POPULI_MODEL` | Default **Ollama** model id when routing uses local inference ([`usage`](../../../crates/vox-orchestrator/src/usage.rs), [`spec`](../../../crates/vox-orchestrator/src/models/spec.rs)). |
| `VOX_ORCHESTRATOR_POPULI_INFERENCE_BASE_URL` | Overrides `Vox.toml` **`[mesh].inference_base_url`** (Schola or Ollama-shaped HTTP base). An empty value clears the TOML entry. Processes that call Ludus still read **`POPULI_URL`**; keep them aligned per [mens serving SSOT](mens-serving-ssot.md). Impl: [`merge_env_overrides`](../../../crates/vox-orchestrator/src/config/impl_env.rs). |
| `POPULI_API_KEY` | Read via Secrets for authenticated remote mens inference. |
| `POPULI_TEMPERATURE` / `POPULI_MAX_TOKENS` | Generation configuration overrides for mens inference. |
| `VOX_ACCOUNT_ID` | Account identifier for orchestrator multi-tenant boundaries. |
| `VOX_SECRETS_CLOUDLESS_DB_PATH` | Path to Cloudless DB for Secrets backend. |
| `VOX_ORCHESTRATOR_EXEC_TIME_BUDGET_ENABLED` / `VOX_ORCHESTRATOR_EXEC_TIME_SAFETY_MULTIPLIER` / `VOX_ORCHESTRATOR_EXEC_TIME_TIMEOUT_RATE_ALERT` / `VOX_ORCHESTRATOR_EXEC_TIME_DEFAULT_BUDGET_MS` / `VOX_ORCHESTRATOR_EXEC_TIME_HISTORY_WINDOW_DAYS` | Execution time budgeting controls for autonomous agent tool invocation (Phase 17). |
| `VOX_ORCHESTRATOR_INTERRUPTION_CAL_A2A_GAIN` | Gain multiplier for A2A interruptions. |
| `VOX_ORCHESTRATOR_INTERRUPTION_CAL_BACKLOG_PENALTY` | Penalty offset for queue backlog in interruption math. |
| `VOX_ORCHESTRATOR_INTERRUPTION_CAL_PLAN_GAIN` | Gain multiplier for plan-related interruptions. |
| `VOX_ORCHESTRATOR_TIER_GATE_ENTROPY_THRESHOLD` / `VOX_ORCHESTRATOR_TIER_GATE_MIN_OBSERVATIONS` | Calibration vars for dynamic tier gating based on query entropy. |
| `VOX_ORCHESTRATOR_TLX_FRUSTRATION` / `VOX_ORCHESTRATOR_TLX_MENTAL` / `VOX_ORCHESTRATOR_TLX_TEMPORAL` / `VOX_ORCHESTRATOR_TLX_TRUST_DISCOUNT` | NASA-TLX cognitive load analogues for orchestrator agent scheduling pressure. |
| `GROQ_API_KEY` / `CEREBRAS_API_KEY` / `MISTRAL_API_KEY` / `DEEPSEEK_API_KEY` / `SAMBANOVA_API_KEY` / `CUSTOM_OPENAI_API_KEY` | Bare provider keys read for optional **key presence** checks in [`usage`](../../../crates/vox-orchestrator/src/usage.rs). Prefer **Secrets** / `VOX_*` secret resolution for real credential storage (see [`AGENTS.md`](../../../AGENTS.md)). |
| `VOX_NEWS_PUBLISH_ARMED` | When `1`/`true`, satisfies the **armed** gate for live news/scientia syndication (in addition to two DB approvers). See [news syndication security](../archive/research-2026-q1/news_syndication_security.md). |
| `VOX_SCHOLARLY_ADAPTER` | Scholarly submit adapter { `local_ledger` (default), `echo_ledger`, `zenodo`, `openreview`, etc. Unknown values error. See [`scholarly::flags`](../../../crates/vox-publisher/src/scholarly/flags.rs). |
| `VOX_SCHOLARLY_DISABLE` | When truthy (`1`, `true`, `yes`, `y`, `on`), blocks all scholarly submit/status paths. |
| `VOX_SCHOLARLY_DISABLE_LIVE` | When truthy, blocks **live** adapters (Zenodo/OpenReview); local/echo ledgers still allowed. |
| `VOX_SCHOLARLY_DISABLE_ZENODO` | Per-adapter kill-switch for Zenodo when truthy. |
| `VOX_SCHOLARLY_DISABLE_OPENREVIEW` | Per-adapter kill-switch for OpenReview when truthy. |
| `VOX_OPENREVIEW_API_BASE` / `OPENREVIEW_API_BASE` | Optional override for the OpenReview API v2 base URL (default `https://api2.openreview.net`). Used for mocks and self-hosted stacks; see [`api_base`](../../../crates/vox-publisher/src/scholarly/openreview.rs). |
| `VOX_ZENODO_SANDBOX` | When truthy, Zenodo REST uses sandbox API host instead of production. |
| `VOX_ZENODO_API_BASE` | Optional override for the Zenodo REST API root (e.g. `https://zenodo.org/api` or `https://sandbox.zenodo.org/api`). Used for mocks and non-standard endpoints; when unset, production vs sandbox follows `VOX_ZENODO_SANDBOX`. See [`ZenodoHttpClient::new`](../../../crates/vox-publisher/src/scholarly/zenodo.rs). |
| `VOX_ZENODO_HTTP_MAX_ATTEMPTS` | Max attempts per Zenodo HTTP call (deposit create, get, bucket `PUT`, `publish`) for retryable errors (5xx, 429, timeouts). Integer **1–10**, default **3**. |
| `VOX_ZENODO_ATTACH_MANIFEST_BODY` | When truthy, after creating a draft deposition, uploads `manifest.body_markdown` as `body.md` to `links.bucket` (Zenodo files API). |
| `VOX_ZENODO_PUBLISH_DEPOSITION` | When truthy, calls deposit `publish` after file attach. Requires **`VOX_ZENODO_ATTACH_MANIFEST_BODY`** or files from **`VOX_ZENODO_STAGING_DIR`** (Zenodo rejects publish with zero files). |
| `VOX_ZENODO_DRAFT_ONLY` | When truthy, never calls `publish` (overrides **`VOX_ZENODO_PUBLISH_DEPOSITION`** and **`VOX_ZENODO_PUBLISH_NOW`**). |
| `VOX_ZENODO_PUBLISH_NOW` | Convenience profile: attach `body.md` and publish when the deposition is otherwise valid (still respects **`VOX_ZENODO_DRAFT_ONLY`**). |
| `VOX_ZENODO_STAGING_DIR` | Directory produced by `publication-scholarly-staging-export` (Zenodo layout). When set, Zenodo submit uploads files from this tree (plan + optional **`VOX_ZENODO_UPLOAD_ALLOWLIST`**) instead of or in addition to manifest-only attach; see [`zenodo_relpaths_to_upload`](../../../crates/vox-publisher/src/scholarly/zenodo.rs). |
| `VOX_ZENODO_UPLOAD_ALLOWLIST` | Comma-separated relative paths under **`VOX_ZENODO_STAGING_DIR`** to upload; when empty, uploads all Zenodo plan files present (excluding arXiv-only artifacts). |
| `VOX_ZENODO_VERIFY_STAGING_CHECKSUMS` | When truthy, requires `staging_checksums.json` and verifies SHA3-256 per file before bucket `PUT`. |
| `VOX_ZENODO_REQUIRE_METADATA_PARITY` | When truthy, requires `zenodo.json` metadata title to match manifest title (trim / ASCII space normalization). |
| `VOX_OPENREVIEW_HTTP_MAX_ATTEMPTS` | Max attempts per OpenReview HTTP call (`notes`, `notes/edits`) for retryable errors. Integer **1–10**, default **3**. |
| `VOX_SCHOLARLY_JOB_LOCK_OWNER` | Optional lock-owner string for `external_submission_jobs` lease ticks (default `vox {<pid>`). |
| `VOX_NEWS_SITE_BASE_URL` | Public site base URL for RSS links (overrides `[orchestrator.news].site_base_url`). |
| `VOX_NEWS_RSS_FEED_PATH` | Repo-relative path to `feed.xml` (overrides `[orchestrator.news].rss_feed_path`). |
| `VOX_NEWS_SCAN_RECURSIVE` | `0`/`1`: whether `NewsService` walks `news_dir` recursively (default `1`). |
| `VOX_NEWS_TWITTER_TEXT_CHUNK_MAX` | Optional integer override for tweet chunk length (defaults to publisher contract value). |
| `VOX_NEWS_TWITTER_TRUNCATION_SUFFIX` | Optional suffix used when shortening non-thread tweets (default `...`). |
| `VOX_SOCIAL_REDDIT_CLIENT_ID` | Reddit OAuth client id for scientia/news syndication submission paths. |
| `VOX_SOCIAL_REDDIT_CLIENT_SECRET` | Reddit OAuth client secret for token refresh on publish. |
| `VOX_SOCIAL_REDDIT_REFRESH_TOKEN` | Reddit refresh token used to mint short-lived access tokens for `/api/submit`. |
| `VOX_SOCIAL_REDDIT_USER_AGENT` | Required descriptive Reddit User-Agent (`platform:app:version (by /u/name)`). |
| `VOX_SOCIAL_YOUTUBE_CLIENT_ID` | YouTube OAuth client id for channel upload automation. |
| `VOX_SOCIAL_YOUTUBE_CLIENT_SECRET` | YouTube OAuth client secret for channel upload automation. |
| `VOX_SOCIAL_YOUTUBE_REFRESH_TOKEN` | YouTube refresh token for user-channel upload scopes. |
| `VOX_SOCIAL_YOUTUBE_DEFAULT_CATEGORY_ID` | Optional default YouTube `categoryId` used when a manifest omits `youtube.category_id` (publisher fallback defaults to `28`). |
| `VOX_SOCIAL_TWITTER_SUMMARY_MARGIN_CHARS` | Optional integer reserve applied when deriving `twitter.short_text` from markdown (`twitter_text_chunk_max - margin`). |
| `VOX_SYNDICATION_TEMPLATE_PROFILE` | When `1`/`true`, applies `distribution_policy.channel_policy.<channel>.template_profile` to derived social copy caps (Twitter margin, Reddit self-post summary, YouTube description). When unset/false, profiles are ignored and `SyndicationResult.decision_reasons` may record `template_profile_inert` if a profile key is set. |
| `VOX_SOCIAL_REDDIT_SELFPOST_SUMMARY_MAX` | Optional integer cap for derived Reddit self-post body text when `text_override` is empty. |
| `VOX_SOCIAL_HN_MODE` | Hacker News publish mode (`manual_assist` only; official HN API is read-only). |
| `VOX_SOCIAL_WORTHINESS_ENFORCE` | `0`/`1`: enforce aggregate worthiness floor before **live** fan-out (orchestrator news tick, `vox db publication-publish`, MCP `vox_scientia_publication_publish` when not dry-run). On MCP, `[orchestrator.news].worthiness_enforce` also applies. |
| `VOX_SOCIAL_WORTHINESS_SCORE_MIN` | Minimum worthiness score when enforcement is on (default **0.85** if unset). MCP may set `[news].worthiness_score_min` instead. |
| `VOX_SOCIAL_CHANNEL_WORTHINESS_FLOORS` | Optional CSV `channel=floor` map (e.g., `reddit=0.82,hacker_news=0.86`) merged into runtime channel policy. |

Socrates numeric thresholds default from [`vox-socrates-policy`](../../../crates/vox-orchestrator-types/src/socrates_policy/mod.rs); optional TOML overrides live under `[orchestrator]` as `socrates_policy` (see `OrchestratorConfig`).

<a id="mcp-socrates-questioning"></a>
## MCP / Socrates questioning (vox-mcp)

Wall-time and attention telemetry for information-theoretic clarification (chat, plan, inline, ghost). Policy defaults (including default max attention when env is unset) also come from [`QuestioningPolicy`](../../../crates/vox-orchestrator-types/src/socrates_policy/mod.rs).

Calibration note: channel gain offsets / backlog penalty / trust-adjustment scale are configured in `Vox.toml` under `[orchestrator].interruption_calibration` (no env override yet).

| Variable | Role |
|----------|------|
| `VOX_QUESTIONING_MIRROR_GLOBAL_ATTENTION` | When **`0`** or **`false`**, questioning debits apply only to the **per-`session_id`** tally. When **unset** or any other value, the same milliseconds also increment the orchestrator [`BudgetManager`](../../../crates/vox-orchestrator/src/budget/mod.rs) global **`AttentionBudget::spent_ms`** (see [`add_questioning_attention_debit_ms`](../../../crates/vox-orchestrator/src/budget/mod.rs)); this does **not** emit an interrupt EWMA event. Implemented in [`ServerState::record_questioning_attention_spend`](../../../crates/vox-orchestrator-mcp/src/server_state.rs). |
| `VOX_QUESTIONING_MAX_ATTENTION_MS` | Optional **unsigned** cap (milliseconds) for the per-session clarification attention analogue. **Unset** or invalid → `QuestioningPolicy::default().max_clarification_attention_ms`. Used by [`questioning_attention_bounds`](../../../crates/vox-orchestrator-mcp/src/server_state.rs). |
| `VOX_SUBMIT_TASK_BYPASS_QUESTIONING_GATE` | When truthy, allows orchestrator **task submit** via MCP to skip the “pending Socrates clarification” gate (operator / CI escape hatch). Gate enforcement applies when `session_id` is provided and DB is attached. See [`task_tools`](../../../crates/vox-orchestrator-mcp/src/task_tools/mod.rs). |
| `VOX_MCP_AGENT_FLEET` | When **unset** or truthy, **vox-mcp** and **`vox-orchestrator-d`** spawn the same embedded `AgentFleet` + [`StubTaskProcessor`](../../../crates/vox-orchestrator/src/runtime.rs) loop ([`spawn_stub_agent_fleet_if_enabled`](../../../crates/vox-orchestrator/src/runtime.rs)) so queued tasks receive `ProcessQueue` wakes (**default on**). Set **`0`**, **`false`**, **`no`**, or **`off`** to disable. |
| `VOX_MCP_ORCHESTRATOR_DAEMON_REPOSITORY_ID_STRICT` | When **`1`** / **`true`** / **`yes`**, **`vox-mcp`** logs **ERROR** (vs default **WARN**) if **`orch.ping`**’s `repository_id` ≠ embedded repo id while **`VOX_ORCHESTRATOR_DAEMON_SOCKET`** points at a TCP daemon ([`ServerState::probe_external_orchestrator_daemon_if_configured`](../../../crates/vox-orchestrator-mcp/src/server_state.rs)). |
| `VOX_MCP_ORCHESTRATOR_RPC_READS` | When **`1`** / **`true`** / **`yes`**, enables all repo-aligned **read** RPC pilots below as if each per-tool flag were set ([`mcp_orch_daemon_reads_pilot_enabled`](../../../crates/vox-orchestrator-mcp/src/server_state.rs)); per-tool flags still work alone for partial enablement. |
| `VOX_MCP_ORCHESTRATOR_RPC_WRITES` | When **`1`** / **`true`** / **`yes`**, enables aligned daemon **write** pilots for task + agent lifecycle methods (`orch.submit_task`, `orch.complete_task`, `orch.fail_task`, `orch.cancel_task`, `orch.reorder_task`, `orch.drain_agent`, `orch.rebalance`, `orch.spawn_agent_ext`, `orch.retire_agent`, `orch.pause_agent`, `orch.resume_agent`) through MCP backend routing in [`ServerState`](../../../crates/vox-orchestrator-mcp/src/server_state.rs). |
| `VOX_MCP_ORCHESTRATOR_TASK_STATUS_RPC` | When **`1`** / **`true`** / **`yes`** (or umbrella **`VOX_MCP_ORCHESTRATOR_RPC_READS`**), MCP tool **`task_status`** calls **`orch.task_status`** on the TCP daemon **only if** startup probe confirmed **`repository_id`** matches the embed ([`orch_daemon_client_for_task_status_rpc`](../../../crates/vox-orchestrator-mcp/src/server_state.rs)). On RPC failure or missing field, falls back to the embedded [`Orchestrator`]. Requires matching tasks on the daemon process (typically: route **`vox_submit_task`** through the same daemon in a later IPC-first phase). |
| `VOX_MCP_ORCHESTRATOR_TASK_WRITES_RPC` | Per-slice override for task write pilots when the global write umbrella is off. Truthy values route MCP submit/complete/fail/cancel/reorder/drain/rebalance through aligned daemon RPC; fallback remains embedded orchestrator when the daemon is absent/misaligned. |
| `VOX_MCP_ORCHESTRATOR_AGENT_WRITES_RPC` | Per-slice override for agent write pilots when the global write umbrella is off. Truthy values route MCP spawn/retire/pause/resume through aligned daemon RPC; fallback remains embedded orchestrator when the daemon is absent/misaligned. |
| `VOX_MCP_ORCHESTRATOR_START_RPC` | When **`1`** / **`true`** / **`yes`** (or umbrella **`VOX_MCP_ORCHESTRATOR_RPC_READS`**), **`vox_orchestrator_start`** calls **`orch.status`** and **`orch.agent_ids`** on the aligned TCP daemon and returns **`daemon_reported_agent_count`**, **`daemon_reported_agent_ids`**, and optional RPC error fields ([`orchestrator_start`](../../../crates/vox-orchestrator-mcp/src/dei_tools/control.rs)). Read-only telemetry; does not replace embedded runtime state. |
| `VOX_MCP_ORCHESTRATOR_STATUS_TOOL_RPC` | When **`1`** / **`true`** / **`yes`** (or umbrella **`VOX_MCP_ORCHESTRATOR_RPC_READS`**), **`vox_orchestrator_status`** attaches **`daemon_orch_status`** (full **`orch.status`** JSON) and optional **`daemon_orch_status_rpc_error`** from the aligned TCP daemon ([`orchestrator_status`](../../../crates/vox-orchestrator-mcp/src/dei_tools/orchestrator_snapshot.rs)). Embedded MCP-built fields unchanged; use to compare daemon vs embed until IPC-first. |
| `VOX_EMBEDDING_MODEL` | Optional embedding model id override for MCP memory retrieval (`vox-mcp` [`retrieval`](../../../crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs)). |
| `VOX_SEARCH_POLICY_VERSION` | Optional override for [`vox_search::SearchPolicy::version`](../../../crates/vox-search/src/policy.rs) (telemetry / diagnostics). |
| `VOX_SEARCH_MEMORY_VECTOR_WEIGHT` | Optional `f32` in `[0, 1]` for memory hybrid fusion (BM25 vs vector leg; default `0.55`). |
| `VOX_SEARCH_VERIFICATION_QUALITY_THRESHOLD` | Optional evidence-quality threshold in `[0, 1]` that triggers the automatic verification pass (default `0.55`). |
| `VOX_SEARCH_REPO_MAX_FILES` | Cap for per-query repository path inventory walks (default `20000`). |
| `VOX_SEARCH_REPO_SKIP_DIRS` | CSV extra skip-dir list for repo inventory (replaces defaults when non-empty). |
| `VOX_SEARCH_QDRANT_URL` | Optional Qdrant HTTP base (e.g. `http://127.0.0.1:6333`) for the `qdrant-vector` backend. |
| `VOX_SEARCH_QDRANT_COLLECTION` | Qdrant collection name used by [`vox_search::vector_qdrant`](../../../crates/vox-search/src/vector_qdrant.rs) (default `vox_docs`). |
| `VOX_SEARCH_QDRANT_VECTOR_NAME` | When the collection uses **named** vectors, set the vector config name (request body `{ "name", "vector" }`). |
| `VOX_SEARCH_QDRANT_API_KEY` | Qdrant `api-key` header for secured / cloud instances. Canonical secret: [`SecretId::VoxSearchQdrantApiKey`](../../../crates/vox-secrets/src/lib.rs) via Secrets ([`secrets-ssot`](./secrets-ssot.md)). |
| `VOX_SEARCH_TANTIVY_ROOT` | Optional directory root for on-disk Tantivy indices (subpath `docs/` holds the docs mirror index). |
| `VOX_SEARCH_PREFER_RRF` | When truthy, runs **reciprocal rank fusion** across non-empty corpus hit lists and exposes **`rrf_fused_lines`** / **`rrf_fused_hit_count`** in MCP retrieval ([`SearchPolicy::prefer_rrf_merge`](../../../crates/vox-search/src/policy.rs)). |
| `VOX_SEARCH_SEARXNG_URL` | Optional SearXNG base URL (Tier 2 web meta-search); when unset, SearXNG is skipped. |
| `VOX_SEARCH_SEARXNG_MAX_RESULTS` / `VOX_SEARCH_SEARXNG_MAX_SCRAPE` | Result cap and deep-scrape cap for SearXNG / fallback web retrieval (see [`SearchPolicy`](../../../crates/vox-search/src/policy.rs)). |
| `VOX_SEARCH_SEARXNG_ENGINES` | Optional override for the SearXNG `engines=` query parameter (comma-separated ASCII engine ids; default from [`contracts/scientia/searxng-query.defaults.v1.yaml`](../../../contracts/scientia/searxng-query.defaults.v1.yaml)). |
| `VOX_SEARCH_SEARXNG_LANGUAGE` | Optional override for the SearXNG `language=` query parameter (short tag; default from the same contract). |
| `VOX_OPENROUTER_HTTP_REFERER` | Optional `HTTP-Referer` header for OpenRouter-compatible calls ([`provider_auth`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)). |
| `VOX_OPENROUTER_APP_TITLE` | Optional `X-Title` header for OpenRouter-compatible calls ([`provider_auth`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)). |
| `VOX_OPENROUTER_ROUTE_HINT` | For **`openrouter/auto`**, selects OpenRouter broker routing via `X-OpenRouter-Provider-Preferences`: `price` / `economy` / `cheap`, `quality` / `performance` / `best`, or `fallback` / `resilience` ([`openrouter_route_hint_from_env`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)). |
| `VOX_COST_PREFERENCE` | When `VOX_OPENROUTER_ROUTE_HINT` is unset or unknown, `performance` / `quality` vs default economy maps to the same route hint for **`openrouter/auto`** ([`provider_auth`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/provider_auth.rs)). |
| `VOX_MCP_GRAMMAR_MASK` | Grammar-mask knob for speech constraints ([`speech_constraints`](../../../crates/vox-orchestrator-mcp/src/speech_constraints.rs)). |
| `VOX_TELEMETRY` | **Master telemetry switch.** `off` / `0` / `false` → all telemetry silenced (no-op recorder, all per-category checks skipped). `on` or unset → default-on local collection. `debug` → reserved for future verbose mode. Overrides all per-category env vars. See [`vox-telemetry::config`](../../../crates/vox-telemetry/src/config.rs). |
| `VOX_MCP_LLM_COST_EVENTS` | When truthy, enables LLM cost telemetry emission ([`infer`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs)). Trust SSOT: [telemetry-trust-ssot](../architecture/telemetry-trust-ssot.md). Superseded by `VOX_TELEMETRY=off` (master switch). |
| `VOX_MCP_TEST_INFER_STUB_BODY` / `VOX_MCP_INFER_STUB_ACK` | **Diagnostics only:** when `VOX_MCP_TEST_INFER_STUB_BODY` holds JSON for a plan payload and `VOX_MCP_INFER_STUB_ACK` is `1` or `true`, `vox_plan` skips real LLM HTTP (see [`infer_test_stub`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/infer_test_stub.rs)). Do not enable on production MCP hosts. |
| `VOX_MCP_HTTP_ENABLED` | When truthy, enables the optional MCP HTTP/WebSocket gateway (`/v1/tools`, `/v1/ws`, `/v1/mobile`) for bounded remote/mobile control of a host machine. |
| `VOX_MCP_HTTP_HOST` / `VOX_MCP_HTTP_PORT` | Bind address for the optional MCP HTTP gateway (defaults: `127.0.0.1:3921`). |
| `VOX_MCP_HTTP_BEARER_TOKEN` | Required bearer token for MCP HTTP gateway requests unless explicitly bypassed with `VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED=1`. Cloudless migration target is Secrets-managed resolution with env retained only as compatibility input under non-strict profiles. |
| `VOX_MCP_HTTP_ALLOW_UNAUTHENTICATED` | Explicit insecure override for local-only testing of the MCP HTTP gateway; default is authenticated mode when enabled. |
| `VOX_MCP_HTTP_ALLOWED_TOOLS` | CSV allowlist for MCP HTTP tool calls. Names are canonicalized through tool aliases. |
| `VOX_MCP_HTTP_READ_BEARER_TOKEN` | Optional read-only bearer token for MCP HTTP gateway access; grants `Read` role (tool list view and read-scoped calls) while `VOX_MCP_HTTP_BEARER_TOKEN` remains full write access. Cloudless migration target is Secrets-managed resolution with env retained only as compatibility input under non-strict profiles. |
| `VOX_MCP_HTTP_READ_ROLE_ALLOWED_TOOLS` | Optional CSV allowlist for read-role tool visibility/invocation. Read-role defaults come from MCP registry metadata (`http_read_role_eligible`) and are always intersected with `VOX_MCP_HTTP_ALLOWED_TOOLS`; this env provides an additional narrowing filter. |
| `VOX_MCP_HTTP_RATE_LIMIT_PER_MINUTE` | Per-client-IP request budget for the MCP HTTP gateway (default `120`). |
| `VOX_MCP_HTTP_REQUIRE_FORWARDED_HTTPS` | When truthy, HTTP gateway requests must carry `X-Forwarded-Proto: https` (reverse-proxy hardening). |
| `VOX_MCP_HTTP_HEALTH_AUTH` | When truthy, `/health` also requires gateway bearer auth; when unset/false, `/health` is rate-limited but unauthenticated. |
| `VOX_MCP_HTTP_TRUST_X_FORWARDED_FOR` | When truthy, rate-limit identity may use the first `X-Forwarded-For` value (for trusted reverse-proxy deployments). |
| `VOX_REPOSITORY_ID` | Optional repository identity label used by MCP A2A queue metadata; defaults to `default` when unset (see [`a2a`](../../../crates/vox-orchestrator-mcp/src/a2a_tools.rs)). |
| `OLLAMA_HOST` | Upstream Ollama base URL override read by MCP provider metadata ([`metadata`](../../../crates/vox-orchestrator-mcp/src/llm_bridge/providers/metadata.rs)). |
| `VOX_ORCHESTRATOR_EVENT_LOG` | Path to a **JSONL** file: **`vox-mcp`** and **`vox-orchestrator-d`** append one JSON object per orchestrator [`AgentEvent`](../../../crates/vox-orchestrator/src/events.rs) when set ([`orchestrator_event_log::spawn_orchestrator_event_log_sink`](../../../crates/vox-orchestrator-mcp/src/dei_tools/orchestrator_snapshot.rs); MCP wires a join slot for re-root). **`vox live`** can tail the same file when built with the `live` feature. |
| `VOX_DASH_HOST` / `VOX_DASH_PORT` | Bind host and port for the local dashboard / **vox-audio-ingress** HTTP surface (**default** `127.0.0.1` / **`3847`**). MCP Oratio helpers use the same vars when calling the ingress ([`oratio_tools`](../../../crates/vox-orchestrator-mcp/src/oratio_tools.rs)). |
| `VOX_BROWSER_LLM_CONTEXT_CHARS` | Optional positive integer: max characters of browser snapshot / summary text included in MCP browser+LLM tool context (**default** `24000` when unset or invalid). See [`browser_tools`](../../../crates/vox-orchestrator-mcp/src/browser_tools.rs). |

## OpenClaw gateway interop (`vox-skills`, `vox openclaw`, script builtins)

| Variable | Role |
|----------|------|
| `VOX_OPENCLAW_URL` | OpenClaw HTTP gateway base URL for skill import/list and compatibility calls (default in CLI/adapter codepaths is localhost). |
| `VOX_OPENCLAW_WS_URL` | OpenClaw Gateway WebSocket control-plane URL (WS-first runtime path for subscribe/notify and generic gateway methods). |
| `VOX_OPENCLAW_TOKEN` | Optional OpenClaw bearer token; resolves via Secrets (`SecretId::OpenClawToken`) where configured. |
| `VOX_OPENCLAW_WELL_KNOWN_URL` | Optional explicit upstream discovery endpoint (`/.well-known/openclaw.json`) used to resolve canonical HTTP/WS/catalog URLs. |
| `VOX_OPENCLAW_CATALOG_LIST_URL` | Optional override for the resolved OpenClaw catalog list endpoint. |
| `VOX_OPENCLAW_CATALOG_SEARCH_URL` | Optional override for the resolved OpenClaw catalog search endpoint. |
| `VOX_OPENCLAW_SIDECAR_DISABLE` | When `1`/`true`, skips managed OpenClaw sidecar install during bootstrap/upgrade release flows. |
| `VOX_OPENCLAW_SIDECAR_EXPECT_VERSION` | Optional operator hint checked by `vox openclaw doctor`; reports match/mismatch against detected sidecar `--version` output. |
| `VOX_OPENCLAW_SIDECAR_START_MAX_ATTEMPTS` | Optional bounded retry count for `vox openclaw doctor --auto-start` WS readiness checks after spawn/state restore (default `3`). |
| `VOX_OPENCLAW_SIDECAR_START_BACKOFF_MS` | Optional initial retry backoff in milliseconds for sidecar readiness checks (default `500`, exponential up to cap). |

See also { [`openclaw-discovery-sidecar-ssot.md`](openclaw-discovery-sidecar-ssot.md).

**MCP tools (VoxDb required for persistence):** `vox_questioning_pending` (unanswered assistant questions + structured `question_options` and session `belief_state_json`), `vox_questioning_submit_answer`, `vox_questioning_sync_ssot`. Canonical names: [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml). Protocol SSOT: [Information-theoretic questioning](information-theoretic-questioning.md).

## Mens / Candle

| Variable | Role |
|----------|------|
| `VOX_CANDLE_DEVICE` | Forces Candle device (e.g. `cpu`); see Mens training SSOT. |
| `VOX_VRAM_OVERRIDE_GB` | Overrides VRAM autodetect for preset hints in `vram_autodetect` (useful in CI/headless hosts). |
| `VOX_MENS_EXPERIMENTAL_OPTIMIZER` | Guard flag required when `optimizer_experiment_mode` is set to a non-`off` value. |
| `VOX_MENS_SKIP_EVAL` | `1` / `true` — skip Step 2 pre-flight eval gate in `full-pipeline.vox` (use when `eval_results.json` is stale or absent on first run). |
| `VOX_MENS_SKIP_MIX` | `1` / `true` — skip Step 1 corpus mix in `full-pipeline.vox` (use when corpus is already fresh). |
| `VOX_MENS_FORCE_TRAIN` | `1` / `true` — proceed past a failing pre-flight or post-train gate in `full-pipeline.vox`; marks receipt as degraded. Dev iteration only. |
| `VOX_MENS_BACKGROUND` | `1` / `true` — spawn training as a background process in `full-pipeline.vox`. |
| `VOX_MENS_DOMAIN` | Domain adapter target for `full-pipeline.vox` (default: `vox-lang`). Selects the domain-specific mix config (`mix-{domain}.yaml`). |
| `VOX_MENS_EPOCHS` | Override training epoch count for `full-pipeline.vox` (default: `3`). |
| `VOX_INFERENCE_PROFILE` | `desktop_ollama` (default), `cloud_openai_compatible`, `mobile_litert`, `mobile_coreml`, `lan_gateway`; gates **vox-mcp** local Ollama + Ollama fallback to `desktop_ollama` / `lan_gateway` only; see [`vox_config::inference`](../../../crates/vox-config/src/inference.rs) and [mobile-edge-ai.md](mobile-edge-ai.md). |
| `VOX_AUTO_MODEL_STRATEGY` | OpenRouter strategy for auto model ids: `provider_auto` or `preferred_model`; see [`vox_config::routing_policy`](../../../crates/vox-config/src/routing_policy.rs). |
| `VOX_AUTO_ROUTING_PRIORITY` | Weighted MCP auto-routing priorities (`efficiency,precision,latency,availability,balance,mobile`) as `k=v` CSV. |
| `VOX_GEMINI_ROUTE_POLICY` | Gemini routing policy: `openrouter_first` (default), `google_direct_only`, or `registry_default`. |
| `OPENROUTER_GEMINI_MODEL` / `GEMINI_DIRECT_MODEL` | Explicit OpenRouter/GoogleDirect Gemini model pair for policy routing/fallback. |
| `VOX_PROVIDER_DAILY_LIMIT_DEFAULT` / `VOX_PROVIDER_LIMIT_PROVIDERS` | Dynamic provider quota defaults before JSON/file overrides in [`usage_policy`](../../../crates/vox-orchestrator/src/usage_policy.rs). |
| `VOX_PROVIDER_DAILY_LIMITS_FILE` | Optional JSON file of per-provider daily limits (merged after defaults in [`usage_policy`](../../../crates/vox-orchestrator/src/usage_policy.rs)). |
| `VOX_PROVIDER_DAILY_LIMITS_JSON` | Inline JSON for the same structure as the file variant. |
| `ANTHROPIC_DIRECT` | Optional direct Anthropic flag for provider metadata resolution. |

## Mens (`vox-populi`, orchestrator probe)

| Variable | Role |
|----------|------|
| `VOX_MESH_ENABLED` | Enables mens registry publish and related hooks. |
| `VOX_MESH_CONTROL_ADDR` | This process’s control plane URL (publish/join target). |
| `VOX_MESH_TOKEN` / `VOX_MESH_WORKER_TOKEN` / `VOX_MESH_SUBMITTER_TOKEN` / `VOX_MESH_ADMIN_TOKEN` | Populi control-plane bearer roles (Secrets SSOT); legacy single-token mode uses `VOX_MESH_TOKEN` only. See [mens SSOT](populi.md). |
| `VOX_MESH_JWT_HMAC_SECRET` | Optional HS256 secret so clients can use `Authorization: Bearer <jwt>` with claims `role`, `jti`, `exp` (Secrets SSOT). |
| `VOX_MESH_WORKER_RESULT_VERIFY_KEY` | Optional Ed25519 public key (hex or Standard base64) -> verify signed `job_result` / `job_fail` deliveries (worker signs raw BLAKE3 digest). |
| `VOX_MESH_SCOPE_ID` | Tenancy for join/heartbeat when enforced server-side. |
| `VOX_MESH_A2A_LEASE_MS` | Inbox claim lease duration (default 120s, clamped). |
| `VOX_MESH_MAX_STALE_MS` | Client-side staleness filter for mens snapshots (MCP). |
| `VOX_MESH_CODEX_TELEMETRY` | Emit Codex `populi_control_event` rows when set. Trust SSOT: [telemetry-trust-ssot](../architecture/telemetry-trust-ssot.md). |
| `VOX_MESH_HTTP_JOIN` | `0`/`false` disables MCP HTTP join to the control plane; see [mens SSOT](populi.md). |
| `VOX_MESH_HTTP_HEARTBEAT_SECS` | MCP heartbeat interval after join (`0` = no background heartbeat). |
| `VOX_MESH_HTTP_RATE_LIMIT` | When `1`/`true`/`on`/`yes`, enables per–client-IP HTTP rate limiting on **`vox populi serve`** (see `tower_governor` in `vox-populi` transport). |
| `VOX_MESH_HTTP_RATE_LIMIT_PER_SEC` | Steady-state requests per second per key when rate limiting is on (default **50**). |
| `VOX_MESH_HTTP_RATE_LIMIT_BURST` | Burst capacity (default scales with per-sec). |
| `VOX_MESH_ADVERTISE_GPU` | Legacy: sets `gpu_cuda` on the host capability snapshot. |
| `VOX_MESH_GPU_READINESS_PROBE_OFF` | When `1` / `true`, workers skip populating **`NodeRecord.gpu_readiness_ok`** / **`gpu_readiness_reason`** / **`gpu_readiness_checked_unix_ms`** from the NVML probe path in **`vox_populi::node_record_for_current_process`** (inventory fields may still be filled). |
| `VOX_MESH_ADVERTISE_VULKAN` | Sets `gpu_vulkan`. |
| `VOX_MESH_ADVERTISE_WEBGPU` | Sets `gpu_webgpu`. |
| `VOX_MESH_ADVERTISE_NPU` | Sets `npu`. |
| `VOX_MESH_DEVICE_CLASS` | Optional `TaskCapabilityHints.device_class` string. |

## GPU probe overrides (Mens training)

| Variable | Role |
|----------|------|
| `VOX_GPU_MODEL` | With `VOX_GPU_VRAM_MB`, overrides [`probe_gpu`](../../../crates/vox-populi/src/mens/tensor/device.rs) (CI / headless / Android host injection). |
| `VOX_GPU_VRAM_MB` | Paired with `VOX_GPU_MODEL` for VRAM heuristics. |

## CI / diagnostics

| Variable | Role |
|----------|------|
| `VOX_COMPILER_HIR_DUMP` | `0` | If `1`, prints HIR AST to stdout after typeck (needs `vox-compiler` built with `--features debug-dump`). |
| `VOX_COMPILER_LOG_FILE` | (none) | Tee diagnostics to a local `.log` (compiler + `vox build/check`). |
| `VOX_COMPILER_RECONCILE_MAX_RETRY` | `3` | Maximum reconciliation attempts during `vox fmt` re-parsing. |
| `VOX_SECRET_GUARD_GIT_REF` | Git revision range for **`vox ci secret-env-guard`** on clean checkouts (e.g. `origin/main...HEAD` on PRs, `${{ github.event.before }}...${{ github.sha }}` on push). Avoids an empty diff scope when `git diff` would otherwise scan nothing. See [`guards.rs`](../../../crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs). |
| `VOX_BUILD_TIMINGS_BUDGET_WARN` | Soft budget warnings for **`vox ci build-timings`**. |
| `SKIP_CUDA_FEATURE_CHECK` | Skip optional `nvcc` gates (documented hatch in [runner contract](../ci/runner-contract.md)). |
| `VOX_BENCHMARK_TELEMETRY` | When `1` or `true`, CLI paths may append **`benchmark_event`** rows to Codex **`research_metrics`** (`bench:<repository_id>`). See [`benchmark_telemetry.rs`](../../../crates/vox-cli/src/benchmark_telemetry.rs) and [Telemetry and research_metrics contract](telemetry-metric-contract.md). Trust SSOT: [telemetry-trust-ssot](../architecture/telemetry-trust-ssot.md). |
| `VOX_SYNTAX_K_TELEMETRY` | When `1` or `true`, enables **`syntax_k_event`** writes; if unset, falls back to **`VOX_BENCHMARK_TELEMETRY`**. Same implementation module as above. |
| `VOX_DOGFOOD_TRACE_PATH` | Path to the local JSONL file for dogfooding/telemetry collection during development runs. |

## Optional telemetry upload (`vox telemetry`)

| Variable | Role |
|----------|------|
| `VOX_TELEMETRY_UPLOAD_URL` | HTTPS ingest URL for **`vox telemetry upload`** (resolved via Secrets; optional until upload is used). See [ADR 023](../adr/023-optional-telemetry-remote-upload.md), [remote sink spec](../archive/research-2026-q1/telemetry-remote-sink-spec.md). |
| `VOX_TELEMETRY_UPLOAD_TOKEN` | Bearer token for ingest when required (Secrets `SecretId::VoxTelemetryUploadToken`). |
| `VOX_TELEMETRY_SPOOL_DIR` | Override directory for the upload queue (default: `<cwd>/.vox/telemetry-upload-queue`). Non-secret path override. |

## Code-Audit / scaling-audit (`vox-code-audit`, `emit-reports`)

| Variable | Role |
|----------|------|
| `VOX_TOESTUB_MAX_RUST_PARSE_FAILURES` | Maximum allowed `rust_parse_failures` in the **`toestub --format json`** v1 envelope before **`vox ci scaling-audit emit-reports`** fails (and before PR CI’s full-`crates/` audit step fails). Non-negative integer. **Unset or invalid** ⇒ no limit (historical `emit-reports` behavior). **PR CI** sets this to **`3`** while the repo baseline is low (recent full `crates/` runs reported **`1`**); tighten to **`0`** once every Rust file parses under `syn::parse_file`, or raise the cap when adding deliberate snapshot exclusions. |

**CLI feature flag (not an env var):** `toestub --feature-flags unresolved-regex-fallback` (comma-separated with other flags) relaxes unresolved-ref’s AST `call_sites` gate so regex-only matches can surface again (e.g. macro-expanded calls). Default remains AST-gated for fewer false positives. See [scaling TOESTUB rules](../archive/research-2026-q1/scaling-toestub-rules.md).

## Web / Vite / TanStack codegen

| Variable | Role |
|----------|------|
| `VOX_BUILD_TARGET` | After TOML merge, overrides **`build_target`**: **`fullstack`**, **`server`**, or **`client`**. CLI **`vox build --target …`** still wins when set. See [`VoxConfig::merge_build_target_from_env_var`](../../../crates/vox-config/src/config/impl_ops.rs), [`BuildTarget`](../../../crates/vox-config/src/config/gamify_web.rs). |
| `VOX_WEB_TANSTACK_START` | When `1` / `true`, enables TanStack **Start** scaffold (`src/routes/*`, `routeTree.gen.ts`, `router.tsx`). Compiler output is **`routes.manifest.ts`** + components (no `VoxTanStackRouter.tsx`). Must stay aligned with **`Vox.toml`** `[web] tanstack_start` for **`vox build`**. See [`VoxConfig::merge_env_overrides`](../../../crates/vox-config/src/), [TanStack how-to](../how-to/tanstack-ssr-with-axum.md). |
| `VOX_WEB_EMIT_SCAFFOLD` | When **`1`** / **`true`**, **`vox build`** may write one-shot user scaffold files next to the TS out dir (`app/App.tsx`, `main.tsx`, Tailwind entry, etc.) if missing. Prefer explicit **`vox build --scaffold`** when scripting. See [`codegen_ts::scaffold`](../../../crates/vox-codegen/src/codegen_ts/scaffold.rs). |
| `VOX_EMIT_EXPRESS_SERVER` | Opt-in: emit legacy **`server.ts`** (Express-style) from `vox-codegen-ts`. Default server is **Axum**; TS consumers use **OpenAPI** + generated **`vox-client`** (no Rust-emitted **`api.ts`**). See [vox-fullstack-artifacts.md](vox-fullstack-artifacts.md). |
| `VOX_ORCHESTRATE_VITE` | If `1`, **`vox run`** spawns **`pnpm run dev:ssr-upstream`** in `dist/.../app` (Vite on **3001**). See [`OrchestratedViteGuard`](../../../crates/vox-cli/src/frontend.rs). |
| `VOX_SSR_DEV_URL` | Origin (e.g. `http://127.0.0.1:3001`) for generated Axum to proxy non-`/api` **GET** document requests before `rust_embed`. Often injected when **`VOX_ORCHESTRATE_VITE=1`**. |
| `VOX_WEB_VITE_SMOKE` | Opt-in: set to **`1`** when running **`cargo test -p vox-integration-tests --test web_vite_smoke -- --ignored`** (full **`pnpm install`** + **`vite build`** on a golden `.vox` fixture). |
| `VOX_GUI_PLAYWRIGHT` | Opt-in: set to **`1`** for **`cargo test -p vox-integration-tests --test playwright_golden_route -- --ignored`** (Playwright screenshot + accessibility snapshot; requires **`pnpm install`** + **`pnpm exec playwright install chromium`** under `crates/vox-integration-tests`). Also gates the Playwright half of **`vox ci gui-smoke`**. |
| `VOX_PLAYWRIGHT_APP_DIR` / `VOX_PLAYWRIGHT_OUT_DIR` | Set by the Playwright harness: absolute path to the built Vite **`app/`** dir and writable artifact dir for **`route.png`** / **`a11y.json`**. |
| `VOX_V0_API_URL` | Optional override for the full v0 chats endpoint URL (default `https://api.v0.dev/v1/chats`); used by tests and local proxies ([`v0.rs`](../../../crates/vox-cli/src/v0.rs)). |
| VOX_WEB_TS_OUT | Optional: absolute or relative directory where **`vox build`** writes generated **`*.tsx`** (same path as the build output). When set, **`vox doctor`** scans **`*.vox`** under the current tree for **`@v0`** declarations and verifies each **`{Name}.tsx`** in this directory uses a **named** export suitable for TanStack **`routes {`** (`export function Name`, etc.). See [`v0_tsx_normalize.rs`](../../../crates/vox-cli/src/v0_tsx_normalize.rs). |
| `VOX_ALLOW_LEGACY_COMPONENT_FN` | When `1`/`true`, enables the escape hatch for classic **decorator-on-fn** React component semantics (parse error by default in 2026). Use only during transitional migrations. See [react-interop-hybrid-adapter-cookbook.md](../archive/research-2026-q1/react-interop-hybrid-adapter-cookbook.md). |
| `VOX_EXAMPLES_STRICT_PARSE` | When **`1`**, **`cargo test -p vox-compiler --test golden_examples_strict_parse`** fails if any **`examples/golden/*.vox`** fails to parse (see [`golden_examples_strict_parse.rs`](../../../crates/vox-compiler/tests/golden_examples_strict_parse.rs)). Default CI sets this for that gate. See [`examples/PARSE_STATUS.md`](../../../examples/PARSE_STATUS.md). |
| `VOX_SUPPRESS_LEGACY_HOOK_LINTS` | When **`1`** / **`true`**, suppresses compiler **warnings** for direct Vox `use_*` hook calls inside legacy reactive bodies (Path C reactive syntax is preferred). Implemented in [`react_bridge::legacy_hook_lint_suppressed`](../../../crates/vox-compiler/src/react_bridge.rs) + [`lint_ast_declarations`](../../../crates/vox-compiler/src/typeck/ast_decl_lints.rs). |
| `VOX_WEBIR_VALIDATE` | **Default on** (unset): **`vox_compiler::codegen_ts::generate`** runs Web IR lower + [`validate_web_ir`](../../../crates/vox-codegen/src/web_ir/validate.rs) after assembly and **fails** if validation returns diagnostics. Set to **`0`** / **`false`** / **`no`** / **`off`** to skip the gate. See [`maybe_web_ir_validate`](../../../crates/vox-codegen/src/codegen_ts/emitter.rs), [`web_migration_env`](../../../crates/vox-codegen/src/web_migration_env.rs). |
| `VOX_WEBIR_REACTIVE_TRACE` | When **`1`** / **`true`**, logs one **`eprintln!`** line per reactive view decision (`component=…` + `pathway=…`). Pairs with aggregate counters via [`reactive_view_bridge_stats`](../../../crates/vox-codegen/src/codegen_ts/reactive.rs). |
| `VOX_RUNTIME_PROJECTION_INCLUDE_HOST_PROBE` | When **`1`** / **`true`**, [`project_runtime_from_hir`](../../../crates/vox-compiler/src/runtime_projection.rs) includes [`probe_host_capabilities`](../../../crates/vox-repository/src/capabilities.rs) in the serialized runtime projection (telemetry / envelope alignment). Default **off** so JSON stays machine-independent in tests. |

## Related

- [Deployment compose SSOT](deployment-compose.md) — Compose profiles and Coolify notes.
- [CI runner contract](../ci/runner-contract.md) — self-hosted labels and CUDA workflow notes.
- [ADR 005 / Socrates](../adr/) — policy and orchestration gates (index in repo).
- [Secrets SSOT](secrets-ssot.md) — canonical managed secret env names and secret-resolution precedence.

## Social credentials precedence

For scientia/news social distribution credentials, resolve in this order:

1. `VOX_SOCIAL_*` environment variables (preferred for CI/production injection),
2. OS keyring (`vox_db::secrets`) when explicitly configured by operator tooling,
3. local `~/.vox/auth.json` fallback for developer-only sessions.

Do not persist raw social API credentials in publication metadata or VoxDb domain tables.
