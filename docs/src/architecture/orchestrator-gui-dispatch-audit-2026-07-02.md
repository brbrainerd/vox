---
title: "Orchestrator + GUI Agent-Dispatch Audit 2026-07-02"
description: "Adversarially verified audit of the orchestrator core, MCP dispatch layer, and GUI dispatch path: confirmed/refuted findings with evidence, plus the gap map against a Claude-Code-grade 'it just works' harness."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Orchestrator + GUI Agent-Dispatch Audit (2026-07-02)

> **Provenance.** Three parallel code audits (orchestrator core/daemon, MCP dispatch
> layer + actor runtime, GUI dispatch path) against worktree
> `claude/inspiring-morse-053427`, followed by an **adversarial verification pass**:
> one independent verifier per claim, prompted to refute, plus five gap-hunting
> lenses (transport auth, journal feasibility, client parity, doc overlap, AI-first
> policy). Verdicts below are post-verification; two first-pass findings were
> refuted and eight were materially corrected. Companion:
> [`vox-gui-harness-buildout-plan-2026.md`](vox-gui-harness-buildout-plan-2026.md)
> (built B1–B6); remediation is sequenced in
> [`vox-axis-harness-reliability-spec-plan-2026-07-02.md`](vox-axis-harness-reliability-spec-plan-2026-07-02.md).

## Verdict

The B1–B6 plumbing landed and works on the happy path — and more of the old
deferral list has been paid down than the build-out plan's own logs admit (GUI tool
calls and approvals already ride one persistent daemon). What is missing is the
**reliability and trust layer**: the daemon transport is unauthenticated, restart
loses or *re-executes* work, several clients still embed private orchestrators, and
dead streams stay dead. Those properties are exactly what makes Claude Code feel
like "it just works."

## Confirmed broken (verified)

| # | Finding | Evidence | Verification notes |
|---|---------|----------|--------------------|
| 1 | **Trust is bypassable at two levels.** (a) Any caller that can write tool args can set `user_approval: true` and skip the HITL gate on all 6 dangerous tools. (b) Worse: the daemon TCP transport (default `127.0.0.1:9745`) has **zero authentication** — no handshake, token, or peer check — so any local process can call `orch.tool_call` directly, or enumerate `orch.list_pending_approvals` and forge the human resolve via `orch.resolve_approval`. The bind address is env-controlled with no loopback guard (`VOX_ORCHESTRATOR_DAEMON_SOCKET=0.0.0.0:9745` publishes the surface to the network). Meanwhile the HTTP gateway over the *same* `ServerState` hard-fails to boot without a bearer token — trust is anchored on the weaker of two co-resident transports. | `vox-orchestrator-mcp/src/dispatch.rs:134-137` (hand-verified); `vox-orchestrator/src/orch_daemon/mod.rs:999-1045,1059-1099`; `daemon_extra.rs:113-147`; `vox_orchestrator_d.rs:52-59`; contrast `http_gateway/mod.rs:306-314` | CONFIRMED, and deeper than first reported: fixing the arg alone is insufficient — a "transport-level field" is equally forgeable while the socket is open to all comers. GUI also adopts any process answering ping on 9745 as "the daemon" (`vox-gui/src/commands/daemon.rs:51-118`) — port-squat takeover. |
| 2 | **Restart mangles work — two distinct failure modes.** (a) *Directly-submitted* tasks (`submit_task*` — the MCP/API/workflow path) never touch the hopper: queued and in-flight tasks are memory-only (`VecDeque` + `in_progress` slot) and are **permanently lost** on crash; the journal hydrator only enriches tasks already in memory. (b) *Hopper-originated* tasks persist in `hopper_inbox`, but **nothing ever transitions them to Assigned/Done** (no production caller of `HopperIntake::assign/complete`), so an in-flight item is re-run from scratch at boot — and **already-completed items are re-executed on every restart**, guarded only by weak per-queue description dedup. Pending approvals (in-memory oneshot map) die too; their `hitl_approvals` audit rows are written best-effort but never rehydrated or expired, so orphaned rows stay `pending` forever. | `queue/mod.rs:31-33`; `orchestrator/core/init.rs:55-68`; `workflow_bridge.rs:221-231`; `hopper/sqlite_store.rs:85-125`; `pending_approvals.rs:38-49`; `dispatch.rs:157-181`; `vox-db/facade/hitl_approvals.rs` (readers are test-only) | PARTIAL vs first pass: not uniform loss — the hopper half is *duplicate re-execution*, a bug the first pass missed entirely. |
| 3 | **Split-brain is real but lives where we weren't looking.** The GUI's tool calls **and** approvals already converged on the one persistent daemon (`invoke_mcp_tool` → `orch.tool_call`; ApprovalsView resolves in the daemon's `ServerState`) — that finding from the first pass is **refuted**. The *actual* private-orchestrator sites: (a) **`vox mcp`** (`lifecycle.rs:27-85`) boots a full in-process `ServerState::new_full` + agent fleet + its own HTTP gateway — every external MCP client (e.g. Claude Code) gets a disjoint orchestrator with invisible approvals; (b) **CLI**: ~7 command modules embed orchestrators via `EmbeddedOrchestratorDriver`/`build_repo_scoped_orchestrator` (`dei.rs` ~20 sites, `safety.rs`, `attention.rs`, `live.rs:285`, `visus/mod.rs:223`, `ludus/hud.rs:14`); (c) **GUI residue**: 13 `control_plane.rs` commands plus `mission_control.rs:16`, `chat.rs:178`, `orchestrator.rs:189/287/445`, `models.rs:205`, `vcs_isolation.rs:18` spawn a one-shot daemon per call, and `memory.rs:150,208` builds an in-process orchestrator inside Tauri; (d) the HTTP gateway is spawned by **both** `vox mcp` and `vox-orchestrator-d`, so a mobile client can attach to the wrong state owner. | `vox-orchestrator-mcp/src/lifecycle.rs:35-61`; `vox-cli/src/commands/dei.rs:886-887`; `vox-gui/src/commands/control_plane.rs:39-46`; `vox-gui/src/commands/memory.rs:8,150,208`; `vox-cli-core/src/daemon_ipc/dispatch.rs:38-56` | GUI-tool-host half REFUTED (fixed since the build-out plan); the client-parity lens found the larger unnamed surface. |
| 4 | **Dead streams stay dead — and it's worse than "falls back to polling."** Both GUI streams exit silently when the daemon dies; the 2 s polling "fallback" **never triggers on daemon death** (it only engages when Tauri `listen()` registration rejects at mount, i.e. browser/dev context). `PersistentDaemon::ensure` caches the address in a `OnceCell` and never re-pings, so nothing ever respawns the daemon for the streams; on-demand status calls "succeed" via per-call spawned daemons, masking the outage while the UI silently shows stale snapshots. | `vox-gui/src/commands/orchestrator.rs:22-93`; `vox-gui/src/commands/daemon.rs:52`; `vox-gui/ui/src/hooks/useOrchestratorStatus.ts:102-122` | PARTIAL vs first pass: the failure is *silent staleness*, not degraded polling. |
| 5 | **Event bus: lossy with no replay — but the offset problem is half-solved.** `Lagged` is silently skipped (count discarded, no gap marker to the peer) and late subscribers see nothing prior. Corrections: production capacity is **1024, not 16** (`EventBus::new(1024)`; the 16s are test/headless constructors), and events already carry a monotonic `EventId(u64)` — but it's minted from an `AtomicU64` that **resets on restart** and isn't atomic with broadcast order, so it cannot serve as a durable replay offset as-is. Exactly one bus per process (MCP tools, HTTP-gateway WS, hopper all share it) — no multi-bus consolidation needed. | `events.rs:745-798`; `orchestrator/core/mod.rs:33`; `orch_daemon/mod.rs:103-130` | PARTIAL: capacity claim corrected; EventId discovery cuts both ways. |

## Half-built (verified, with corrections)

| Finding | Evidence | Correction vs first pass |
|---------|----------|--------------------------|
| `agent_runs.approval_ref` hardcoded `None`; `finish_gui_run` accepts cost/tokens but no caller passes them | `vox-gui/src/commands/runs.rs:79,130`; callers `App.tsx:505,515` | CONFIRMED. Note: per-run cost should come from the **existing vox-telemetry** `ModelCallEvent`/`TaskRootSummaryEvent` aggregates, not a new ledger. |
| Streaming LLM: `llm_stream` **exists but is dead code** (zero callers); `vox-llm-egress` has full SSE (`stream_once`); the `TokenStreamed` events the GUI shows come from **vox-gamify's third, parallel streaming stack** | `vox-actor-runtime/src/llm/stream.rs:11`; `vox-llm-egress/src/wire.rs:164-229`; `vox-orchestrator/src/runtime.rs:168-184`; `vox-gamify/src/ai/client/ctor.rs:305` | Reframed: consolidation work, not greenfield. |
| Context management: chat path passes messages through 1:1, but a **tested `CompactionEngine` exists with zero production `compact()` callers**, a manual `vox_session_compact` MCP tool is wired, and `apply_context_budget` truncates RAG chunks in production. The `ContextWindow` vox-db tables from the 2026-06-20 spec are vapor; that 498-line spec itself survives **only in `refs/jj/keep`** (recover via `git show f8b7ae35f9:docs/superpowers/specs/2026-06-20-context-window-management-design.md`). | `vox-orchestrator/src/compaction.rs:166+`; `handlers_session.rs:50`; `vox-actor-runtime/src/retrieval.rs:46` | Reframed: wire + fix (compact() discards dropped turns), don't build from scratch. |
| No per-tool-call timeout: `TimedExecution` **measures only** (`timeout_budget_ms` is telemetry, not a deadline); no caller wraps `handle_tool_call`. Some tools self-bound (agy exec/gates). | `dispatch.rs:226-244`; `vox-db/src/exec_time_telemetry.rs:54-88` | CONFIRMED; fix belongs at the dispatch seam, with per-tool budgets (agy delegations are legitimately long-running). |
| Remote (Populi) cancel is fire-and-forget — and **silently skipped entirely** when no tokio runtime is current or populi config is incomplete; relay discards the HTTP response | `lifecycle_ops.rs:158-231`; `a2a/dispatch/mesh.rs:236-239` | CONFIRMED (broader than first pass). The lock-poison half is **REFUTED**: the cancel path uses poison-recovering `sync_lock::rw_read/rw_write` throughout (`vox-orchestrator-queue/src/sync_lock.rs:13-20`, tested). |
| Budget check-then-reserve race on concurrent submission (self-documented, PR #61 follow-up); same check-only pattern on the tenant monthly gate | `task_submit.rs:241-263` (race), `:210-230` (tenant) | CONFIRMED. |
| Auto-approve tier: **4 conjuncts** (entropy < 0.15, repeats ≥ 10, trust ≥ 0.85, tier ∈ {Trusted, System}), re-classified per task from a live Kalman-updated trust snapshot — one failure typically drops trust below 0.85, and 3 sub-floor events demote the tier. Real staleness gaps: **no time-based decay** for idle agents; `repeated_approve_count` and the entropy input are lifetime-monotonic, never windowed. | `attention/routing.rs:52-190`; `attention_fields.rs:61-77`; `attention/budget.rs:50-52` | PARTIAL: "never re-calibrates" refuted; the durable gaps are decay + rolling windows. |
| Swallowed writes: `budget/persistence.rs:29` and `memory/manager.rs:129-217` confirmed, **plus** `sync_to_db:310-321` (counts a failed write as synced). The "event emitted before DB write" claim is **REFUTED** — completion persists first with retry×3 + degradation outbox; the real defect is `runtime.rs:682` discarding `complete_task`'s error `Result` unlogged. | `budget/persistence.rs:29`; `memory/manager.rs:129-217,310-321`; `task_dispatch/complete/success/mod.rs:513-539`; `impl_support.rs:152-183`; `runtime.rs:682` | Two of three sites confirmed; third replaced with the actual bug. |
| Cost-policy TODOs unimplemented (`clutch-budget`, `risk-safety-budget`) | `orchestrator_policy.rs:273-279` | CONFIRMED. |
| Oplog checkpoint stub — but **P3-T9 is mostly built**: the `Projection` trait, `ProjectionRegistry`, all four projections (locks/kudos/capabilities/affinity), and the bit-identical replay acceptance test exist. Missing: the `compact_now` body, the hydrate-from-Checkpoint startup path, and `HopperInboxProjection` (Hp-T5). | `vox-orchestrator-queue/src/oplog/checkpoint.rs:11-22`; `projection.rs:14-73`; `tests/projection_replay.rs` | Corrected path (vox-orchestrator-queue) and reframed: the durability substrate is designed and half-built — journal work should **complete it**, not parallel it. |
| Daemon methods for convergence exist and pass tests (`orch.tool_call`, `orch.list_pending_approvals`, `orch.resolve_approval` via `ExtraDispatch`); approvals methods lack direct RPC-level tests; `pending_approvals.rs` module doc is stale (calls the implemented RPC a "follow-up"); `contracts/orchestration/orch-daemon-rpc-methods.schema.json` is missing all five load-bearing methods and has **no parity gate** | `daemon_extra.rs:95-147`; `protocol.rs:77-96`; `tests/daemon_extra_tests.rs` (2 pass) | CONFIRMED + contract-drift addendum. |

## What is genuinely good (don't rebuild)

- ~389 dispatch arms + registry tools with schemars-derived schemas, a consistent
  `ToolResult` envelope (`remediation` field), and a solid preflight chokepoint
  (budget → toestub → scope → lock → skill allowlist → guardrail kernel).
- Sub-agent **worktree isolation** (`agy_worktree.rs`, `.vox/agy-worktrees/{slug}`).
- Multi-provider LLM routing, per-call token/cost telemetry (vox-telemetry
  `ModelCallEvent`/`TaskRootSummaryEvent` — implemented), rate-limit retry with
  backoff.
- One shared event bus per process with monotonic per-event ids; the P3-T9
  projection/replay architecture (trait + 4 projections + acceptance test).
- The HTTP gateway's auth model (bearer required by default, origin guard, rate
  limits, read/write roles) — the template the TCP path should reach parity with.
- GUI honesty held (no fixture data in primary panels, `gui-honesty` gate); GUI
  tool calls + approvals already ride one persistent daemon; task interrupt works
  from the GUI Stop button; poison-recovering lock helpers on the cancel path.

## Gap map vs a Claude-Code-grade harness

| Property | Claude Code | Vox today |
|---|---|---|
| **Durable session** — journal is SSOT; resume/replay free | Yes | P3-T9 substrate half-built; direct-submit tasks vanish, hopper tasks re-execute, approvals die parked |
| **Trust lives in the harness** — modes, allowlists, unforgeable channel | Yes | `user_approval` arg bypass **and** unauthenticated daemon socket; three uncomposed trust mechanisms (gate, attention tier, risk matrix) |
| **One state owner** | Yes | `vox mcp` + ~7 CLI modules + GUI residue each embed private orchestrators |
| **Self-healing plumbing** | Yes | No reconnect; silent staleness on daemon death |
| **Context "just fits"** | Yes | CompactionEngine unwired; per-run cost unattributed (telemetry exists, join missing) |

## Remediation

Sequenced in
[`vox-axis-harness-reliability-spec-plan-2026-07-02.md`](vox-axis-harness-reliability-spec-plan-2026-07-02.md)
(INV-1..5, Phases 0–5). Naming: **Vox Axis** is the canonical GUI product name
("Vox Access" in some meeting notes is a speech-to-text artifact — a
canonical-names row should record the alias so it is never re-litigated).
