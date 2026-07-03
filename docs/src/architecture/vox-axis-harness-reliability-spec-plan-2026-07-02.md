---
title: "Vox Axis Harness Reliability — Spec + Plan 2026-07-02"
description: "Adversarially audited spec and TDD-ready execution plan to close the reliability/trust gap between Vox Axis and a Claude-Code-grade harness: authenticated harness-owned trust, op-log-as-SSOT durable sessions, single-daemon state across ALL clients, self-healing streams, and runtime consolidation — with contracts and CI guards so every invariant is machine-enforced."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Vox Axis Harness Reliability — Spec + Plan (2026-07-02)

> **Provenance.** v2, rewritten after an adversarial verification pass (16
> independent claim verifiers + 5 gap lenses) over the v1 draft and
> [`orchestrator-gui-dispatch-audit-2026-07-02.md`](orchestrator-gui-dispatch-audit-2026-07-02.md).
> The pass refuted two v1 findings (GUI tool-host split-brain — already fixed;
> cancel-path lock panics — poison-safe helpers exist), corrected eight, and
> surfaced five structural gaps v1 missed: the unauthenticated daemon transport,
> the `vox mcp` private orchestrator, the half-built P3-T9 durability substrate
> the journal must complete rather than parallel, six existing ledgers the
> journal must reconcile with, and the absence of contracts/CI guards for the new
> invariants. Successor context:
> [`vox-gui-harness-buildout-plan-2026.md`](vox-gui-harness-buildout-plan-2026.md).
>
> **Naming.** The surface is **Vox Axis** (canonical; `AxisMark` in `vox-gui`).
> "Vox Access" in some dictated notes is a speech-to-text artifact. T0.0 adds the
> canonical-names row so this is never re-litigated.

## 1. Spec — what "it just works" means

Five invariants, each with an observable acceptance condition. Claude Code's
reliability is these invariants, not a feature list.

### INV-1 — Harness-owned trust, on an authenticated channel
The decision to run a dangerous action is made by the harness, keyed to an
**authenticated** caller identity, and is never derivable from data the model
controls.

- A tool argument (`user_approval` or any model-writable field) MUST NOT grant
  approval.
- **A DispatchRequest field is model-writable-equivalent absent transport
  authentication.** Today the daemon TCP socket (default `127.0.0.1:9745`) accepts
  any local connection with no handshake — so `orch.tool_call`,
  `orch.resolve_approval`, and any future `permission_mode` field are all
  forgeable by any process that can open a socket (including the supervised
  agent's own subprocesses). A bare loopback TCP socket does **not** satisfy "the
  model cannot write this field."
- Trust therefore has three layers, all required: (a) transport auth (per-daemon
  secret or OS-ACL'd pipe — reach parity with the HTTP gateway's existing
  bearer+origin model, which guards the *same* `ServerState`); (b) a
  `PermissionMode` bound to that authenticated identity; (c) a persisted per-repo
  allowlist keyed by the action manifest's `safety_class`, not hardcoded tool
  names.
- **Precedence (encoded in a contract, not prose):** explicit deny >
  PermissionMode > persisted allowlist > risk-confidence-matrix `hitl_actions` >
  attention auto-approve tier. The learned attention tier's inputs (repeated
  approvals) are model-influenced, so it may only auto-approve within the
  ceiling the manifest `safety_class` allows.
- *Observables:* (1) an agent emitting `{"user_approval":true}` in args still
  parks; (2) a process without the daemon secret cannot call `orch.tool_call` or
  resolve an approval it did not originate; (3) a non-loopback bind without auth
  configured refuses to start.

### INV-2 — Durable session (op-log as SSOT, two durability tiers)
Every **lifecycle** transition — task submitted/started/completed/failed, agent
spawned, approval requested/resolved, feedback requested/resolved, hopper
admit/assign/complete — is appended to the durable op-log *before* it is
broadcast. High-frequency deltas (`TokenStreamed`, heartbeats, ticks) are
broadcast-first and either ephemeral or batch-flushed; the crash contract is
**recovery to the last completed turn/lifecycle transition, never mid-token**.
In-memory structures are derived caches.

- *Observable:* kill the daemon process mid-task (Rust `Child::kill`); on restart
  the run list, the task's last lifecycle state, and any pending approval are
  present; a completed hopper item is NOT re-executed; a subscriber replays from
  an offset and reconstructs the session's lifecycle exactly.

### INV-3 — One state owner, for every client
Exactly one process (`vox-orchestrator-d`) owns dispatch state. **All** clients —
Vox Axis desktop, `vox mcp` stdio (external agent harnesses), CLI commands,
mobile via the HTTP gateway — are thin: they authenticate, subscribe, and send
commands. The allowed in-process constructors are an explicit, guard-enforced
allowlist (the daemon's own bootstrap + test harnesses).

- *Observable:* an approval parked via any client is listable and resolvable from
  any other client; the HTTP gateway is spawned only by the daemon; a CI guard
  fails on new `ServerState`/orchestrator construction outside the allowlist.

### INV-4 — Self-healing plumbing
Reconnect, retry, and resume are invisible. A daemon death degrades latency,
never correctness or visibility — and is *detected*, not masked.

- *Observable:* kill the daemon while Vox Axis is open; within a bounded window
  the GUI detects silence, restarts the daemon, resubscribes from its last
  journal offset, replays the gap, and shows no stale state — zero user action.
  (Today the failure is silent staleness: the polling fallback only triggers on
  Tauri-listener registration failure, and `PersistentDaemon`'s OnceCell never
  re-pings.)

### INV-5 — Context that just fits, costs that attribute
Conversations don't fail because they got long; every run shows real cost.

- *Observable:* a conversation exceeding the model window keeps working (older
  turns compacted losslessly — archived, not discarded — with a visible
  compacted marker); every `agent_runs` row shows `cost_usd`/tokens joined from
  vox-telemetry aggregates.

### Non-goals (explicit)
- No new IPC transport in the webview (ADR-037 stands).
- No org-style RBAC roles/groups; INV-1 is authenticated-channel trust +
  mode/allowlist.
- No rewrite of the tool dispatch surface, preflight chain, worktree isolation,
  LLM routing, or vox-telemetry — all verified good.
- `clients/runtime-*` (VoxRuntime) is the app runtime for programs *built in
  Vox*, not an orchestrator client — out of scope here.
- Soft-HITL FeedbackStore durability: **in scope only as journal events**
  (T1.1 includes FeedbackRequested/Resolved so T1.4 can rehydrate gating edges);
  the approvals-vs-soft-feedback surface split from
  `2026-06-19-attention-aware-soft-hitl-design.md` is unchanged.

## 2. Target architecture

```text
            ┌────────────── vox-orchestrator-d — THE one state owner ──────────────┐
            │  authenticated DispatchRequest{permission_mode} ─► preflight chain    │
 Axis GUI ─▶│                                        │                              │
 (thin)     │              append(Tier A) ─► OP-LOG (convergence_op_log + P3-T9    │
 vox mcp ──▶│                    │           projections; offset minted at append)  │
 (proxy)    │                    ▼                        │                          │
 CLI ──────▶│   broadcast bus (1024, derived) ◄─ replay_from(offset) on attach     │
 (thin)     │        │  Tier B (tokens/ticks): broadcast-first, batched/ephemeral   │
 mobile ───▶│        ▼                                                              │
 (gateway,  │  pending-approvals · agent_runs · hopper (assign/complete wired)      │
  daemon-   │  hitl_approvals · workflow_run_log  — all projections/joined tables   │
  only)     └───────────────────────────────────────────────────────────────────────┘
```

Verified implementation facts this plan builds on (not aspirations):

- **The durability substrate is half-built.** P3-T9's `Projection` trait,
  `ProjectionRegistry`, four projections, and a bit-identical replay acceptance
  test exist (`vox-orchestrator-queue/src/projection.rs`,
  `tests/projection_replay.rs`); `convergence_op_log` has hot/warm/cold tiering
  with a stubbed checkpoint (`oplog/checkpoint.rs:11`). Phase 1 **completes this
  design** (per `mesh-phase3-vcs-gossip-plan-2026.md` P3-T9 + Hp-T5); it does not
  add a parallel journal.
- **Single-daemon methods exist and pass tests**: `orch.tool_call`,
  `orch.list_pending_approvals`, `orch.resolve_approval` via `ExtraDispatch`
  (`daemon_extra.rs`, `daemon_extra_tests.rs`).
- **GUI tool calls + approvals already converged** on `PersistentDaemon` →
  `orch.tool_call`. Phase 2 is about the *other* clients.
- **One bus per process** (capacity 1024), every event already stamped with a
  monotonic `EventId` — but it resets per-process and isn't broadcast-atomic, so
  the durable offset is minted at op-log append, never from `EventId`.
- **Streaming and compaction exist as dead/unwired code**: `llm_stream`
  (zero callers) over `vox-llm-egress::stream_once` (full SSE); tested
  `CompactionEngine` with no production `compact()` caller; vox-gamify runs a
  third parallel streaming stack that actually feeds `TokenStreamed`.
- **Cost accounting is implemented** (vox-telemetry `ModelCallEvent` /
  `TaskRootSummaryEvent` + vox-db sink; empirical pricing per
  `telemetry-driven-cost-accounting-research-2026.md`). The journal carries
  correlation ids only — never a second cost ledger.
- **The HTTP gateway's auth model** (bearer required by default, origin guard,
  rate limits, read/write roles) is the in-repo template for TCP transport auth.

### Existing-ledger reconciliation (who owns what after Phase 1)

| Ledger | Disposition |
|---|---|
| `convergence_op_log` (+ P3-T9 projections) | **THE journal.** Gains dispatch-lifecycle `OperationKind`s. |
| `hopper_inbox` | Intake SSOT (per `unified-task-hopper-research-2026.md`); becomes a projection for state (Hp-T5); replay keys on item/task id so `enqueue_dedup` can't double-enqueue. |
| `agent_runs`, `hitl_approvals` | Derived/joined tables; journal rows carry their ids as FKs, never duplicate payloads. |
| `workflow_run_log` | Stays workflow-runtime's ledger; journal references run ids. Lease rows are time-based state — replay must expire/re-acquire, never resurrect (explicit RED test). |
| `agent_session_events`, `orchestration_lineage_events`, `history_entries` | Untouched independent writers this phase; candidates for later projection subsumption (documented, not silently dual-written). |
| vox-telemetry sinks | Cost/latency SSOT; journal never dual-writes their payloads. |
| `vox-journal` FileJournal | Stays the workflow-runtime/terminal-transcript primitive; not the dispatch journal (no offset API, no compaction). |

## 3. Plan

TDD throughout: failing contract/integration test first. No stubs. **All test
orchestration is Rust integration tests (`std::process`/tokio) or `scripts/*.vox`
via `vox run` — no new `.ps1`/`.sh`/`.py`; process-kill on Windows uses
`Child::kill`, not shell.** Each phase that introduces a new concept adds its
`where-things-live.md` row in the same PR (dispatch event log, permission-mode/
allowlist store, GUI daemon supervisor). Every new contract registers in
`contracts/index.yaml` with an `enforced_by` gate.

### Phase 0 — Close the trust holes (urgent; ~2–3 days)

**T0.0 — Record the name.** Add a "GUI product surface" row to
`docs/src/architecture/canonical-runtime-names.md`: canonical **Vox Axis** /
`AxisMark`; deprecated alias "Vox Access" (transcription artifact, never
shipped). `contracts/naming/renames.v1.json` is deliberately not used — it is
for code identifiers and no code rename occurred.

**T0.1 — Kill the `user_approval` arg backdoor.** Remove the
`args.get("user_approval")` read (`dispatch.rs:134-137`). Gate machinery
(timeout, outcomes, `hitl_approvals` audit rows) unchanged.
- *RED:* dangerous tool + `{"user_approval":true}` in args still parks.
- *Guard:* register `user_approval` (as a tool-arg key) in the retired-symbol
  check so the backdoor cannot be reintroduced.

**T0.2 — Authenticate the daemon transport.** A per-daemon secret: generated at
daemon start, written to a mode-restricted file (and handed directly to a child
the GUI spawns), required on every DispatchRequest; reuse the HTTP gateway's
bearer semantics for parity. Refuse non-loopback binds unless auth is configured
(`normalize_tcp_bind_addr` gains an `is_loopback` guard). The GUI verifies the
daemon it adopts (secret round-trip on ping) — port-squatting on 9745 is
rejected, not adopted.
- *RED:* a client without the secret gets an auth error from `orch.tool_call`
  and `orch.resolve_approval`; a `0.0.0.0` bind without auth refuses to start;
  a fake ping-responder is not adopted by `PersistentDaemon::ensure`.

**T0.3 — Permission modes + allowlist, contract-first.**
`PermissionMode { Ask, AcceptEdits, AcceptAll, Plan }` bound to the
authenticated caller. Mechanism is data, not hardcoded names:
- Add `risk_class` per tool to `contracts/operations/catalog.v1.yaml`,
  regenerated into `tool-registry.canonical.yaml` (`vox ci operations-sync`);
  replace the `matches!` list in `dispatch.rs:124-131` with a registry lookup
  (parity test: every currently-gated tool carries the class).
- Author `contracts/orchestration/permission-modes.v1.yaml` mapping mode →
  auto-approved `safety_class`es (AcceptEdits ⇒ `mutating` ∧ `reversible`;
  AcceptAll ⇒ + `destructive`; Ask honors `confirmation_policy`), mirrored by a
  Rust default with a parity test (`risk-confidence-matrix.v1.yaml` precedent);
  the GUI action manifest (`contracts/gui/action-manifest.v1.yaml`, which
  exists) is the safety-classification SSOT.
- Persist the per-repo allowlist ("always allow X here") in vox-db; surface the
  mode toggle + always-allow checkbox on the approval prompt.
- Encode the INV-1 precedence order in the contract.
- *Acceptance:* AcceptEdits auto-approves a `mutating`+`reversible` tool but
  parks `vox_run_shell`; always-allow survives restart, scoped to repo; named
  tools appear only in tests, not the mechanism.

### Phase 1 — Op-log as SSOT (the keystone; ~1.5 weeks)

**T1.1 — Dispatch lifecycle events, contract-first.** Extend the existing op-log
rather than adding a ledger: new dispatch `OperationKind`s (or a `DispatchEvent`
enum embedded in ops) for task/agent/workflow lifecycle, ApprovalRequested/
Resolved `{approval_id, run_id, tool, resolver}`, FeedbackRequested/Resolved,
TaskDoubted, hopper admit/assign/complete — each carrying correlation ids
(`run_id`/`task_id`/`agent_id`/`approval_id`; cost lives in vox-telemetry, join
by `trace_id`). These are **new variants** — `AgentEventKind` (~80 variants) has
no approval events today, and the MCP gate writes `hitl_approvals` without
emitting; the gate becomes an emit site. Large payloads (e.g.
`AutoHealApplied.new_source`) go to blob refs (`payload_blob_id` exists), never
inline.
- Author `contracts/orchestration/dispatch-events.v1.schema.json` (+ fixtures
  validated in CI; `workflow-journal.v1.schema.json` / ADR-019 precedent);
  register with `enforced_by`. Consider a short ADR — this log becomes the
  orchestrator SSOT.

**T1.2 — Two-tier append-before-broadcast.**
- **Tier A (durable, WAL-commit before broadcast):** all lifecycle/approval/
  feedback/hopper events (<10/s) — the offset is minted at append under the
  op-log writer, is monotonic across restarts (high-water mark persisted /
  derived from log tail), and rides the broadcast frame.
- **Tier B (broadcast-first):** `TokenStreamed`, heartbeats, throughput/cost
  ticks, diag/lock chatter — not durably journaled; recovery contract is the
  last committed turn (`chat_append_message` + a Tier-A `TurnCompleted`). The
  daemon relay batches/coalesces Tier-B frames so a slow client lags on tokens,
  never on lifecycle.
- *RED:* after `emit_task_completed` with zero subscribers, the event is
  queryable from the log; a forced-fsync-per-token test does NOT exist because
  the contract forbids it — instead assert Tier-B events are absent from the
  durable log.

**T1.3 — Replay-from-offset subscribe.** `orch.subscribe`/`orch.subscribe_events`
accept `from_offset`; the daemon replays Tier-A events since that offset, then
tails live (Tier-B is tail-only). A `Lagged` subscriber logs the skipped count
and the client reconnects at its last offset — no more silent gaps.
- Also: update `contracts/orchestration/orch-daemon-rpc-methods.schema.json` to
  the full `protocol.rs` method set (it is missing all five load-bearing
  methods) and add a parity test protocol-consts ↔ schema-enum, registered in
  `contracts/index.yaml`.

**T1.4 — Rehydrate on startup, with a precedence rule.** **Journal wins**; derived
tables are rebuilt/patched from it (matching P3-T9's "restart replays log →
reconstructs every table"). `init_db()` replays to reconstruct in-flight direct
-submit tasks (restored `Running` → `Interrupted`-for-resume), pending approvals
(re-parked, resumable), and pending FeedbackRequests/gating edges. Hopper
interplay: the inbox remains intake SSOT; replay keys on ids so `enqueue_dedup`
suppresses duplicates. **Wire the hopper lifecycle** — assign-on-dispatch,
complete-on-finish (the missing `HopperIntake::assign/complete` callers) — so
completed items stop re-executing on every restart; add `HopperInboxProjection`
(Hp-T5). Reconcile orphaned `hitl_approvals` rows (expire stale `pending`).
- *RED tests:* (1) submit→start→`Child::kill`→restart: task state present,
  approval still awaitable; (2) completed hopper item is not re-executed after
  restart; (3) crash while a workflow run holds a lease → lease expired or
  cleanly re-acquired, never resurrected; (4) no double-enqueue journal×hopper.

**T1.5 — Wire correlation + cost joins.** Populate `agent_runs.approval_ref` from
ApprovalResolved events; thread `run_id` into the MCP gate; populate per-run
`cost_usd`/tokens by joining vox-telemetry `model_call_event`/
`task_root_summary` on `trace_id`/`parent_task_id` (**no journaled cost
deltas**).
- *Acceptance:* a run row shows its approval outcome and telemetry-derived cost.
- *Known gap (spec-compliance review + follow-up, 2026-07-03):* the `run_id`
  join relies on `task_id` (or an explicit `trace_id`/`correlation_id`) being
  present in a dangerous tool's call `args`. Audited call sites confirm this
  is populated for GUI-driven `invoke_mcp_tool`/`orch.tool_call` calls (human
  clicks "run tool"), but **not** for an agent's own tool calls made while the
  orchestrator autonomously executes a task — `AiTaskProcessor::process`
  (`vox-orchestrator/src/runtime.rs`) only logs an `@tool` intent line as a
  tracing breadcrumb and never calls `handle_tool_call`/
  `handle_tool_call_with_mode`. See the doc comment on
  `OperationKind::ApprovalRequested::run_id`
  (`crates/vox-orchestrator-queue/src/oplog/mod.rs`) for the full audit and
  the tracked follow-up (bridge `AiTaskProcessor`'s tool intents into a real
  `handle_tool_call_with_mode` dispatch, threading `task.id` through).

**T1.6 — Retention.** Finish the P3-T9 `compact_now` stub (snapshot projections →
blake3 → blob → prune warm rows) + hydrate-from-Checkpoint startup.
- *Acceptance:* startup rehydrate time bounded by live-state size, not lifetime
  event count.

### Phase 2 — One daemon for every client (~1 week)

**T2.1 — GUI residue.** Migrate ALL per-call `call_daemon` sites onto
`PersistentDaemon` + `OrchDaemonClient`: the 13 `control_plane.rs` commands plus
`mission_control.rs:16`, `chat.rs:178`, `orchestrator.rs:189/287/445`,
`models.rs:205`, `vcs_isolation.rs:18`; route `memory.rs:150,208` (in-process
orchestrator inside Tauri!) through daemon memory tools.
- *Acceptance:* no `call_daemon` and no `build_repo_scoped_orchestrator` in
  `vox-gui/src`.

**T2.2 — `vox mcp` becomes a thin stdio↔daemon proxy.** Today
`run_stdio_server_blocking` boots a full private `ServerState::new_full` + agent
fleet + its own HTTP gateway — every external MCP client gets an orchestrator
whose approvals/journal are invisible to Axis. Forward tool calls to
`vox-orchestrator-d` via `orch.tool_call` (authenticated per T0.2), spawning the
daemon if absent.
- *RED:* an approval parked via a `vox mcp` tool call is visible in the Axis
  ApprovalsView and resolvable there.

**T2.3 — CLI converges.** Route `dei`/`safety`/`attention`/`live`/`visus`/
`ludus-hud` through the daemon client; deprecate `EmbeddedOrchestratorDriver`
for client use (daemon bootstrap only). Either route `vox generate`'s
orchestrator mode through the daemon (`ai.generate` exists) or list it in the
exemption appendix as a stateless exception.

**T2.4 — Gateway single-spawn + exemption allowlist + guard.** The HTTP gateway
is spawned only by `vox-orchestrator-d` (delete/gate the `lifecycle.rs:56`
spawn) — a gateway request and a GUI `orch.tool_call` must observe the same
pending approval (test). Add the exemption appendix (test harnesses +
`new_for_daemon`) and a **new CI guard** (`harness-trust-guard`, registered via
the `cmd_enums.rs`/`run_body.rs` pattern like `gui-honesty`) asserting: no
`args.get("user_approval")` in dispatch paths; no `ServerState`/orchestrator
construction outside the allowlist; no `call_daemon` in vox-gui.

**T2.4 status (landed):** verified `spawn_http_gateway_if_enabled` already had
exactly one call site (`vox-orchestrator-d`'s `main()`) — T2.2's own removal of
the stdio-path spawn call left single-spawn already true; added the
cross-visibility test below rather than a source fix. Full-codebase audit
(item 3) found the constructor surface already clean except one disclosed,
pre-existing gap (`vox stop`, see appendix). See
[T2.4 exemption appendix](#t24-exemption-appendix-harness-trust-guard-allowlist)
immediately below for the allowlist the new `harness-trust-guard` gate
encodes.

#### T2.4 exemption appendix: harness-trust-guard allowlist

The following in-process `Orchestrator`/`ServerState` constructions in
`crates/vox-gui/src`, `crates/vox-cli/src`, and `crates/vox-orchestrator-mcp/src`
are legitimate and are **not** flagged by `vox ci harness-trust-guard`. Audited
2026-07-02 against the actual tree (not the original plan's assumptions) via:

```
grep -rn "Orchestrator::new\|ServerState::new_full\|ServerState::new_for_daemon\|build_repo_scoped_orchestrator" \
  crates/vox-gui/src crates/vox-cli/src crates/vox-orchestrator-mcp/src
```

1. **Test-only construction** — any hit inside a `#[cfg(test)]` module or
   under a `crates/*/tests/` integration-test directory. Examples found in
   this audit: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs`
   (7×`Orchestrator::new`), `crates/vox-orchestrator-mcp/src/registry.rs`'s
   `merged_registry_tests` module (`ServerState::new_full`). Tests legitimately
   need a real, disposable orchestrator instance; they never share state across
   process/client boundaries the way a live client path would.

2. **`vox-orchestrator-d`'s own daemon bootstrap** —
   `crates/vox-orchestrator-d/src/bin/vox_orchestrator_d.rs`'s call to
   `build_repo_scoped_orchestrator(cfg, None)` in `main()`. This IS the single
   state owner; it is what every other client (GUI, CLI, `vox mcp`, HTTP
   gateway) is supposed to converge onto, not a client-side violation. Its
   defining implementation, `ServerState::new_full`'s body in
   `crates/vox-orchestrator-mcp/src/server_state.rs:166` (which itself calls
   `build_repo_scoped_orchestrator`), is likewise exempt — it is the
   constructor's definition, not a client call site.

3. **`vox mcp`'s protocol-level-only `ServerState::new_full`** —
   `crates/vox-orchestrator-mcp/src/lifecycle.rs:49`
   (`run_stdio_server_blocking`). Per T2.2, this state backs ONLY
   tool-schema listing/resources/prompts; it does not spawn an agent fleet, DB
   connection, `FlywheelMonitor`, or attention-calibration loop, and tool
   *execution* is forwarded to the shared daemon via
   `crate::daemon_route::call_tool_via_daemon`. A second live orchestrator
   never actually runs here — it is a lightweight local shell for
   protocol-level concerns that don't need live orchestrator state.

4. **NOT exempt, and confirmed absent as of this audit:**
   `EmbeddedOrchestratorDriver`, `build_repo_scoped_orchestrator_cli`, and
   `build_repo_scoped_orchestrator_for_repository` have zero active
   (non-comment) call sites in `crates/vox-cli/src` today. The only textual
   hits are: (a) the known-dead `/* ... */` block in
   `crates/vox-cli/src/commands/dei.rs` (~line 1006-1009, `run_dei_analyze`,
   commented out, T2.3 predates this task and left it disabled), and (b) doc
   comments in `attention.rs`/`safety.rs` narrating the T2.3 fix (naming the
   retired call for context, not invoking it). The guard's comment-skipping
   must not flag either.

5. **Disclosed pre-existing gap (not fixed in T2.4, tracked as follow-up):**
   `crates/vox-cli/src/commands/dei.rs`'s `stop()` (`vox stop`, ~line 490)
   still builds a fresh, throwaway local `Orchestrator` via
   `build_repo_scoped_orchestrator` and calls `emergency_stop` on it — this
   does NOT reach the shared daemon's live agents. This was already
   self-disclosed in the source (T2.3's own comment says "T2.4 candidate")
   before this task began. Fixing it requires a new daemon RPC
   (`orch_daemon_method` has no `EMERGENCY_STOP` equivalent yet), which is a
   backend protocol change out of scope for a CI-gate task. `vox stop` prints
   an explicit warning at runtime naming this gap. Tracked as a Phase-5-shaped
   follow-up, not silently carried forward — `harness-trust-guard` does NOT
   flag this call site (it is `build_repo_scoped_orchestrator`, which is
   itself only interesting as a client construction when paired with
   `ServerState`/tool-call plumbing; `dei.rs:490`'s bare `.orchestrator` for a
   single synchronous `emergency_stop()` call is exempted by name below rather
   than by pattern, since the guard cannot safely infer "reaches live daemon
   state" vs. "one-shot local call" from source alone).

6. **`setInterval` orchestrator-status polling in `crates/vox-gui/ui/src`** —
   present today (`hooks/useOrchestratorStatus.ts`) but ONLY as a fallback
   when the primary event-stream listener (`listenOrchStatus`) fails to
   attach; this is exactly what T3.1 is scoped to remove (`PersistentDaemon`
   supervision + reconnect + gap-replay, "no `setInterval` orchestrator
   polling remains (guard-checked)"). `harness-trust-guard`'s polling check is
   therefore registered as a **no-op with a tracked TODO** today (see the
   guard's own doc comment) rather than a failing check — flipping it to
   enforce is T3.1's acceptance criterion, not T2.4's.

### Phase 3 — Self-healing (~3 days; needs T1.3)

**T3.1 — Supervise + reconnect.** `PersistentDaemon` drops the OnceCell-forever
cache for a supervised handle: liveness re-check (authenticated ping), respawn
on death, resubscribe from last offset, replay the gap. Frontend detects stream
silence (heartbeat timeout) instead of relying on the polling fallback that
never fires; delete the fallback once supervision lands.
- *RED:* `Child::kill` the daemon mid-stream → GUI reconnects and receives the
  Tier-A events emitted during the outage; no `setInterval` orchestrator polling
  remains (guard-checked).

### Phase 4 — Runtime consolidation (~1 week; parallel-safe after T1.2)

**T4.1 — One streaming stack.** Wire the existing-but-dead
`vox_actor_runtime::llm_stream` (over `vox-llm-egress::stream_once`) into the
durable-activity facade with the same retry/telemetry wrapping as `chat_once`;
migrate vox-gamify's parallel streaming stack onto it so `TokenStreamed` carries
provider deltas from one code path.
- *Acceptance:* transcript renders provider tokens incrementally; gamify's
  stream_ollama/gemini/openrouter are deleted or delegate.

**T4.2 — Context management = recover + wire, not build.** Recover the 2026-06-20
context-window spec from `refs/jj/keep`
(`git show f8b7ae35f9:docs/superpowers/specs/2026-06-20-context-window-management-design.md`)
into the docs tree as design SSOT (or explicitly supersede with rationale).
Deliverable: wire `vox_orchestrator::CompactionEngine` into message assembly
before `llm_chat`/`llm_stream`, fixing its audit defects (compact() must
**archive dropped turns losslessly**, not discard; per-model token estimation at
the boundary, not bytes-per-token). GUI surfacing follows the existing
`2026-06-19-dockable-workspace-context-memory-ssot-design.md`
(Context-Window Editor + `ContextWindowMeter`) — no new UI under this plan.
- *Acceptance:* a driven-over-the-limit conversation completes with a compaction
  event; dropped turns are retrievable.

**T4.3 — Per-tool timeouts from metadata.** Wrap dispatch (`dispatch.rs:237-244`)
in a timeout sourced per-tool from the registry/action-manifest
(`execution_mode`/duration metadata) with a default; agy delegation tools are
the canonical long-running exception, named in the acceptance tests.

### Phase 5 — Verified-smell sweep (rolling, one PR each)

- Swallowed writes: `budget/persistence.rs:29`; `memory/manager.rs:129-217` +
  `sync_to_db:310-321` (stop counting failed writes as synced).
- Log the discarded `complete_task` Result at `runtime.rs:682` (the
  event-before-persistence claim was refuted — ordering + retry + outbox are
  already correct).
- Atomic check-and-reserve on `BudgetManager` (`task_submit.rs:242-263`) and the
  tenant monthly gate (`:210-230`).
- Remote-cancel ack: await/ack the Populi relay, warn-level log on failure, and
  fix the silent skip when no runtime/config (`lifecycle_ops.rs:158-231`).
- Attention staleness: add time-based trust decay for idle agents and rolling
  windows for `repeated_approve_count`/approve-rate (revocation-on-failure
  already exists via Kalman + demotion — do not rebuild it).
- Stale docs: `pending_approvals.rs:11-14` module doc (the "follow-up" RPC is
  implemented); add RPC-level tests for `orch.list_pending_approvals`/
  `orch.resolve_approval`.
- ~~Cancel-path lock poison hardening~~ — dropped; `sync_lock` helpers already
  recover (verified).

## 4. Sequencing and execution shape

```text
Phase 0 (trust: name, backdoor, transport auth, modes)  ── first, independent
        │
Phase 1 (op-log SSOT: complete P3-T9)  ◄── KEYSTONE
        │
        ├──► Phase 2 (one daemon: GUI residue, vox mcp proxy, CLI, gateway, guard)
        │            └──► Phase 3 (self-healing; needs T1.3 offsets)
        └──► Phase 4 (runtime consolidation; needs T1.2 tiers)

Phase 5  ── rolling, one PR each
```

Per-phase execution shape (parallel reads, sequential writes — subagents are
read-only in the worktree sandbox; the main session writes and commits):

- **Phase 0:** sequential single-writer (small surface, one chokepoint file);
  contract authoring can be scouted by a parallel reader against
  `risk-confidence-matrix` precedent.
- **Phase 1:** scout with parallel readers (one per existing ledger in the
  reconciliation table) to confirm dispositions; then sequential writes —
  op-log/emit-site changes are one-writer territory. Verify with a workflow:
  one agent per RED scenario (kill-restart, completed-hopper, lease, dedup).
- **Phase 2:** pipeline over client modules (GUI residue / vox mcp / CLI
  modules) — each migration is independent; adversarial verify stage greps for
  `call_daemon`/constructor regressions before the guard exists.
- **Phase 4:** T4.1/T4.2/T4.3 are independent; parallel scouts, sequential
  landing.

## 5. Verification gates

- Existing: `cargo run -p vox-arch-check` EXIT=0; `vox ci gui-catalog-parity`,
  `gui-honesty`, `check-links`, `check-codex-ssot` (schema changes bump
  `BASELINE_VERSION` + digest per the B2 precedent);
  `cargo run -p vox-doc-pipeline -- --lint-only` on this doc and the audit doc
  (frontmatter: no hand-added `last_updated` — pipeline derives it).
- New, landed in the same PR as the behavior they guard:
  - `harness-trust-guard` (T2.4): no `user_approval` arg read; constructor
    allowlist; no `call_daemon` in vox-gui; no orchestrator `setInterval`
    polling in vox-gui/ui.
  - Retired-symbol registration for `user_approval` (T0.1).
  - Contract parity: `dispatch-events.v1.schema.json` fixtures test (T1.1);
    `orch-daemon-rpc-methods.schema.json` ↔ `protocol.rs` consts (T1.3);
    `permission-modes.v1.yaml` ↔ Rust default (T0.3); `risk_class` coverage of
    the gated-tool set (T0.3). All registered in `contracts/index.yaml` with
    `enforced_by`.
- Every task: failing test first; no fixture fallback in promoted panels.

## 6. Effort + risk summary

| Phase | Effort | Risk | Unlocks |
|---|---|---|---|
| 0 Trust | ~2–3 days | Low–Med (transport auth touches all clients) | Closes both CRITICALs (arg backdoor + open socket) |
| 1 Op-log SSOT | ~1.5 weeks | Medium (emit sites, init, hopper lifecycle) | INV-2; offsets for INV-4; correlation for INV-5 |
| 2 One daemon | ~1 week | Medium (`vox mcp` proxy is the big one) | INV-3 across GUI/MCP/CLI/mobile |
| 3 Self-healing | ~3 days | Low (needs T1.3) | INV-4 |
| 4 Runtime | ~1 week | Medium (stack consolidation) | INV-5 |
| 5 Smells | rolling | Low | Hardening |

If only one phase ships, ship 0 — the trust holes are exploitable today. The
keystone remains Phase 1: with a durable, offset-addressable op-log, phases 2–4
land on rock instead of re-plumbing in-memory state.
