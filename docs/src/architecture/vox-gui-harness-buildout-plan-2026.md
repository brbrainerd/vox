---
title: "Vox GUI Harness Build-Out Plan 2026"
description: "Three-track plan to evolve the Tauri vox-gui from a CLI-derived dashboard into a full agentic code harness: stateful core, CLI-derived command surface, and design/UX. Successor to the 2026-05-28 GUI capability audit."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-30"
training_eligible: false
---

# Vox GUI Harness Build-Out Plan 2026

> **Provenance.** Derived from a four-surface investigation (CLI, GUI, orchestrator
> backend, frontend infra) plus firsthand verification of the load-bearing claims.
> Successor to [`vox-gui-capability-audit-2026.md`](vox-gui-capability-audit-2026.md)
> (2026-05-28): the audit diagnosed; this plan sequences the work into executable,
> TDD-ready tasks. Strategic frame is [ADR-037](../adr/037-tauri-gui-replaces-axum-dashboard.md).

## Context

The `vox-dashboard` → `vox-gui` migration is **complete**: the standalone Axum SPA
crate is gone, `vox-gui` is the canonical Tauri 2 surface (`surface-ownership.v1.yaml`
`status: canonical`), version parity is fixed, and the residual references to the
deleted crate have been reconciled across `Cargo.toml`, `layers.toml`, the surface
registry, env-var/dependency/link contracts, secret-guard allowlists, and CI (the
obsolete `check-dashboard-ssot` guard is retired in favor of `gui-catalog-parity` /
`gui-surface-coverage` / `gui-version-sync`).

What remains is **capability**, not migration. The GUI's discoverability spine is
already CLI-derived per ADR-037 (a 472-entry compiled command catalog → action
manifest → sidecar execution, plus 11 React surfaces and 23 IPC commands). It is a
real app, not a mockup. But it is still a *dashboard*, not a *harness*: it lacks the
stateful core — event streaming, a canonical run store, interactive approvals,
streaming chat — that turns command discovery into an agentic loop.

## Architecture decision (reaffirms ADR-037)

The GUI is **neither hand-designed end-to-end nor fully generated from the CLI**. The
durable shape, confirmed by the capability audit:

1. The compiled clap catalog is the SSOT for command *existence*.
2. A versioned GUI **action manifest** enriches it with typed args, execution
   semantics, output contracts, capability gates, and UX hints.
3. Generated command forms + command-palette entries are derived from that manifest.
4. **Curated panels** are reserved for workflows that need live state, event streams,
   timelines, approvals, or visual comparison — runs, repositories, models, memory,
   approvals.
5. Mobile lowers against the portable `VoxRuntime` contract (`clients/runtime-*`),
   not direct `@tauri-apps/api/*` imports. Desktop stays Tauri 2.

## Harness gap map (what we have vs. a full harness)

| Element | Today | Gap class |
|---|---|---|
| Command discovery + execution | Strong (catalog → action manifest → sidecar) | A: typed forms, safety metadata |
| Conversational agent loop | Loquela composer is optimistic local state | B: streaming + real runs |
| Task/Run store + timeline | Runs view + GUI-local `start_gui_run`/`finish_gui_run`; no canonical store | B: persist to execution domain |
| Live activity / log streaming | 2s polling; `/api/v2/` is health-only | B: daemon event stream → Tauri events |
| Human-in-the-loop / approvals | `ApprovalTier`/`ApprovalOutcome` exist but always auto-resolve | B: interactive prompt surface |
| Code review / diff | Git MCP tools (diff/log/blame) exist | B: diff/review panel |
| File/workspace browsing | none | B/A: file tree + open |
| Model management | Strong (registry, routing, scoreboard, explain) | B: persist active-model change |
| Memory / knowledge | Partly fixtures | B: wire to `vox-search` + provenance |
| Repository health | Incomplete panel | B: back with `vox ci`/`check`/`repair` |
| MCP execution from GUI | Hard error at `transport.ts:187` | B: IPC bridge to MCP dispatch |
| Mesh / distributed | Simulated peers | B: topology graph API |
| Speech-to-code | `vox-tauri-stt` exists; mic unwired | B: wire STT → composer |
| Design / UX quality | Real shell; Latin labels, fixtures, no token discipline | C: tokens, a11y, copy, hierarchy |

## Track B — Stateful harness core (FIRST; backend-API-first)

This is the highest-value track and the prerequisite for an honest harness. Each task
builds on an existing backend seam rather than greenfield. TDD: write the failing
contract/integration test first, then the implementation.

- **B1 — Daemon event stream → Tauri events.** Replace the 2s `setInterval(poll)` in
  `App.tsx` with a push channel. Add a streaming method to the daemon wire protocol
  (`vox-foundation::protocol`) that emits orchestrator deltas (agent state, queue,
  task transitions, cost); relay them through a Tauri `emit`/`listen` bridge in
  `vox-gui`. *Acceptance:* GUI reflects an agent state change with no polling timer;
  integration test asserts an emitted event arrives at a mock listener.
- **B2 — Canonical agent-run store.** Extend the existing execution domain
  (`vox-db/src/schema/domains/execution.rs`, which already has `scheduled_runs`) with
  an `agent_runs` table: run id, command, repo/worktree, model, cost/tokens, logs ref,
  artifacts, diagnostics, approval refs, status. Repoint the GUI's
  `start_gui_run`/`finish_gui_run` IPC at it. *Acceptance:* a run created via the GUI
  survives a restart and is queryable by id; replay returns the recorded invocation.
- **B3 — Interactive approvals (HITL).** The approval *types* already exist
  (`vox_orchestrator::ApprovalTier::{Confirm,Review,AutoApprove}`,
  `ApprovalOutcome`). Add a pending-approval queue + a synchronous resolve path
  (daemon method + MCP tool) so a `Confirm`/`Review` tier *blocks on a human* instead
  of auto-approving, and surface it as a GUI prompt with the risk context. *Acceptance:*
  a `Review`-tier action stays pending until the GUI resolves it; test asserts the
  agent does not proceed before resolution.
- **B4 — Streaming chat.** `chat_message` currently returns full JSON. Add chunked/SSE
  emission and relay tokens to Loquela via B1's event bridge; have the composer create
  a real B2 run and attach messages/model-routing/approvals to it. *Acceptance:* tokens
  render incrementally; the conversation is persisted as a run.
- **B5 — MCP execution bridge.** Replace the `transport.ts:187` error path with an IPC
  handler that dispatches to the MCP layer (`vox-orchestrator-mcp::dispatch`, ~100+
  tools). Generate forms from real JSON schemas instead of the generic `payload` blob.
  *Acceptance:* a representative MCP tool (e.g. `vox_git_diff`) runs end-to-end from the
  GUI with a typed form.
- **B6 — Promote panels onto real contracts.** Memory → `vox-search` retrieval with
  provenance; Repository → `vox ci`/`check`/`repair`/drift; Mesh → a topology graph
  endpoint; persist Models active-model and Settings policy. *Acceptance per panel:* no
  fixture fallback remains; an IPC-mock test proves the panel renders from real handler
  output, not sample data.

*Workflow execution shape:* `parallel` by capability (B1 stream / B2 store / B3 HITL /
B4 chat / B5 MCP), each agent owning a backend contract + its GUI binding, followed by
an adversarial verify stage that greps each panel for fixture fallbacks and asserts the
integration test exists. B6 fans out one agent per panel after B1/B2 land.

## Track A — CLI-derived command surface (parallel-safe, low risk)

- **A1 — GUI action manifest.** Author `contracts/gui/action-manifest.v1.yaml` generated
  from the clap catalog + hand metadata: typed args (flags/options/positionals/enums/
  defaults/conflicts/deps/examples), execution semantics (read-only vs mutating,
  destructive, dry-run, net/fs/process, expected duration), output contract
  (text/json/msgpack/stream/artifacts/run-id/diagnostics), capability metadata
  (secrets, tools, daemon, feature gates, mobile suitability), UX hints (grouping,
  palette-only vs full panel).
- **A2 — Generated forms + types.** Emit TS types from the manifest; render real command
  forms in the catalog surface, replacing ad-hoc object-key conversion.
- **A3 — Parity gates.** Extend `gui-catalog-parity` so every GUI-executable command has
  typed args, safety metadata, and an output contract; fail on drift vs compiled clap.

*Workflow execution shape:* `pipeline` over the ~60 command groups — one agent per group
authors its commands' metadata; a verify stage checks each entry against compiled clap
reality.

## Track C — Design / UX (continuous)

- **C0 — Plain-language + 5-surface convergence.** Replace Latin/opaque nav labels;
  converge on Command Center, Runs, Models, Repositories, Memory as primary, everything
  else secondary or palette-driven.
- **C1 — Adopt the token system.** Vox already ships a contrast-validated token system
  (`contracts/tokens/tokens.v1.json` + WCAG engine from the GUI-native roadmap Phase 4–6).
  Adopt `vox-tokens.css`/`tokens.ts` in `vox-gui/ui`; eliminate hardcoded colors/px.
- **C2 — Visual hierarchy + layout pass** per surface (spacing scale, typographic
  hierarchy, density, consistent component variants).
- **C3 — Accessibility audit:** contrast along the ancestor chain, keyboard navigation,
  focus order, touch-target size, screen-reader labels.
- **C4 — Microcopy + states:** real empty/loading/error states for every panel (no
  fixture placeholders); CTA and confirmation wording.
- **C5 — Handoff specs** for promoted panels so Tracks A/B build to a defined spec.

*Workflow execution shape:* one design agent per surface produces critique +
token-mapped redesign + a11y findings + copy; a synthesis agent reconciles into a shared
design-system doc under `docs/src/reference/`.

## B1 implementation log (2026-05-30)

**Design decision (transport).** The daemon (`vox-orchestrator-d`) is a persistent
server holding `Arc<Orchestrator>`, but `dispatch_request` returns exactly one
`DispatchResponse` and both connection loops (`handle_connection` TCP,
`run_stdio_server` stdio) were strictly one-response-per-request. Streaming reuses
the existing newline-delimited frame format by:

1. Adding a non-terminal `DispatchPayload::Event { value: Value }` variant
   (`vox-foundation::protocol`) — the existing streaming variants (`Chunk`,
   `Progress`) are text-only; orchestrator deltas are structured JSON.
2. Adding `orch_daemon_method::SUBSCRIBE = "orch.subscribe"`.
3. Special-casing `SUBSCRIBE` in both connection loops: the daemon pushes an
   initial `orch.status()` snapshot as an `Event` frame, then re-samples on a
   500 ms interval and emits a new frame only when the snapshot changes, until the
   peer disconnects (a write error ends the stream). This moves polling
   server-side (one daemon loop) and gives clients a push stream — ADR-037
   compliant (daemon-side stream, no webview `WebSocket`).
4. `OrchDaemonClient::subscribe(tx: mpsc::Sender<Value>)` forwards each `Event`
   value into a channel and returns on `Done` / receiver-drop / disconnect — the
   shape the GUI will drain to re-emit Tauri events.

**Landed + verified (TDD):**
- `crates/vox-foundation/src/protocol.rs` — `SUBSCRIBE` const + `Event` variant.
- `crates/vox-orchestrator/src/orch_daemon/mod.rs` — `stream_status_events` +
  `write_frame` helpers; `SUBSCRIBE` branch in both loops.
- `crates/vox-orchestrator/src/orch_daemon/client.rs` — `subscribe(tx)`.
- Exhaustive-match `Event` arms in `vox-cli-core::daemon_ipc::dispatch` (both
  fns) and `vox-ml-cli::dei_daemon`.
- Test: `orchestrator_daemon_subscribe_streams_status_event` in
  `crates/vox-orchestrator/tests/orchestrator_daemon_tcp.rs` (RED→GREEN); all 3
  daemon TCP tests pass; consumer crates compile clean.

**Frontend flip — landed (2026-05-30):** The GUI reaches the daemon by *spawning* it
over stdio (`call_daemon`), not a persistent TCP connection — so the GUI consumes the
stream via a spawn-based helper, while the TCP `OrchDaemonClient::subscribe` remains
for TCP-mode/tests.
- `crates/vox-cli-core/src/daemon_ipc/dispatch.rs` — `subscribe_daemon(daemon, method,
  params, tx)`: spawns the daemon over stdio (mirrors `call_daemon_streaming`),
  forwards each `Event { value }` into an mpsc `Sender`, terminates the child on
  `Done`/receiver-drop/EOF.
- `crates/vox-gui/src/commands/orchestrator.rs` — `spawn_orchestrator_status_stream`
  drains the channel, maps each raw `orch.status()` snapshot through the existing
  `to_gui_status`, and re-emits it as the `"vox://orch-status"` Tauri event (const
  `ORCH_STATUS_EVENT`).
- `crates/vox-gui/src/main.rs` — `.setup()` hook starts the subscription for the app
  lifetime.
- `crates/vox-gui/ui/src/{transport.ts,App.tsx}` — `listenOrchStatus` helper; `App.tsx`
  now subscribes to the pushed stream (initial snapshot on mount, then `listen()`),
  with 2 s polling retained **only** as a fallback when `listen()` rejects (browser/dev).
- Verified: `cargo check -p vox-gui` + `-p vox-cli-core` clean; `vite build` passes;
  backend daemon tests green. **Remaining verification:** a live Tauri launch to confirm
  the UI renders from the stream end-to-end (no automated harness for the Tauri runtime).

## B2 implementation log (2026-05-31)

**Reframe (verified):** GUI runs already persisted — but onto the *shared*
`workflow_run_log` table (the workflow-runtime ledger). B2 gave the GUI a
purpose-built store, not "add persistence."

**Landed + verified (TDD):**
- `crates/vox-db/src/schema/domains/execution.rs` — `agent_runs` table (run_id,
  workflow_name, command, repo, worktree, model, status, planned/completed_steps,
  cost_usd, tokens_in/out, logs_ref, artifacts_json, `approval_ref` (nullable; wired
  by B3), started/updated/completed_at_ms, last_error) + status/updated indexes.
- `crates/vox-db/src/schema/manifest.rs` — `BASELINE_VERSION` 67 → 68 (additive
  table; baseline re-runs idempotently via `CREATE TABLE IF NOT EXISTS`).
- `contracts/db/baseline-version-policy.yaml` — `repository_baseline_integer` 68 +
  refreshed Keccak-256 digest (`vox ci check-codex-ssot` parity).
- `crates/vox-db/src/facade/agent_runs.rs` — `AgentRunRow` + `agent_runs_upsert` /
  `agent_runs_get` / `agent_runs_recent` (mirrors `facade/scheduled.rs`; row stays in
  the facade, so `row-serde-lint` does not apply; `db-schema-coverage` is satisfied
  because vox-db owns the table).
- Test: `agent_runs_persist_query_and_survive_restart` (RED→GREEN) — upsert, query
  by id, recent list, and **survives a file reopen** (the restart-durability bar).
- GUI repoint: `crates/vox-gui/src/commands/runs.rs` now uses the facade for
  start/finish/list (richer `StartGuiRunInput`/`GuiRunRecord`) + a new `get_gui_run`
  replay-by-id command (registered in `main.rs`); `RunsView.tsx` fetches the selected
  run by id (so restart-then-open works) and shows command/model/cost.
- Verified: `cargo check -p vox-gui` clean; `vite build` passes; backend test green.
  (Pre-existing: vox-db *lib unit* tests don't compile — `missing field primary_key`
  in `ddl/diff.rs` + `schema_digest/helpers.rs`, unrelated to B2; integration tests
  unaffected.)

**Deferred to B3:** `approval_ref` is a real nullable column but unwired until HITL.

## B5 implementation log (2026-05-31)

**Decision (transport).** Three options existed: (a) call `vox_orchestrator_mcp::handle_tool_call`
as a library, (b) spawn `vox mcp` over rmcp JSON-RPC stdio, (c) add a tool-call method to
`vox-orchestrator-d`. Path (c) is the most coherent long-term (reuses the GUI's existing
`call_daemon` channel, no second orchestrator) **but** the daemon's `dispatch_request` lives in
the vox-orchestrator library, which cannot reference `ServerState` (vox-orchestrator →
vox-orchestrator-mcp would cycle) — so (c) needs a serve-API refactor. Chose **(a)** as the
lowest-risk initial integration.

**Landed + verified:**
- `crates/vox-gui/Cargo.toml` — `vox-orchestrator-mcp` dep (L5→L3, downward; arch-check EXIT=0,
  no new violation).
- `crates/vox-gui/src/commands/mcp.rs` — `McpToolHost { OnceCell<ServerState> }` (built once via
  `ServerState::new_full(load_config())` + optional `with_db_initialized`, reused across calls) and
  `#[tauri::command] invoke_mcp_tool(tool, args)` → `handle_tool_call`, returning
  `{ tool, is_error, result }` (`is_error` via `tool_json_envelope_is_error`); dispatch failures
  surface as `Err`, never panic.
- `crates/vox-gui/src/{commands/mod.rs,main.rs}` — module + `.manage(McpToolHost)` +
  `invoke_mcp_tool` registration.
- `crates/vox-gui/ui/src/transport.ts` — the `handler_kind === 'mcp'` dead-end (was a 64-exit
  "not executable" error) now invokes `invoke_mcp_tool` and wraps the JSON envelope as
  `ExecuteOutput` (exit 1 when `is_error`). CLI/IPC branches unchanged.
- Verified: `cargo check -p vox-gui` clean, `cargo run -p vox-arch-check` EXIT=0, `vite build` passes.

**Caveats / follow-ups:**
- The integration test (`crates/vox-gui/tests/mcp_bridge_tests.rs`) builds a real `ServerState` and
  dispatches read-only `vox_git_status`, asserting a non-error envelope — it runs in the default
  `cargo test` (the orchestrator's background pollers are fire-and-forget; `vox_git_status` is
  deterministic local git). End-to-end through `handle_tool_call`, not just type wiring.
- **Two orchestrators**: this runs an in-process `ServerState` orchestrator alongside the GUI's
  per-call spawned `vox-orchestrator-d`. Acceptable for read-only tools; the coherent convergence is
  **path (c)** — add an `orch.tool_call` daemon method (needs the serve-API refactor) so the GUI
  reuses one daemon. Tracked as a follow-up.
- Dangerous tools (`vox_write_file`, `vox_run_shell`) require `"user_approval": true` in `args` or
  `handle_tool_call` returns an RBAC_VIOLATION envelope — the GUI must set this when wiring a
  destructive action's form.

## B4 implementation log (2026-05-31) — core (event stream); chat-transcript deferred

**Scope decision.** B4's full intent (streaming chat attached to runs, with `run_id`
correlation threaded through `submit_orchestrator_task → runtime`) has an invasive,
risky correlation step. Landed the **safe, high-value core** instead: bridge the
orchestrator's *existing* event bus (which already emits `TokenStreamed` + task/agent
lifecycle on a `tokio::broadcast`, `runtime.rs:148`) to the GUI as a live activity
stream. Run-correlation + a Loquela chat transcript are deferred (see below).

**Landed + verified (TDD):**
- `vox-foundation` — `orch_daemon_method::SUBSCRIBE_EVENTS = "orch.subscribe_events"`.
- `vox-orchestrator/src/orch_daemon/mod.rs` — `stream_agent_events`: subscribes to
  `orch.event_bus().subscribe()` and pushes one `DispatchPayload::Event` frame per
  `AgentEvent` (fully push-driven, no polling); handles broadcast `Lagged` (skip) and
  `Closed` (end); special-cased in both the TCP and stdio loops like `SUBSCRIBE`.
- `vox-orchestrator/src/orch_daemon/client.rs` — `subscribe_events(tx)` (shares a
  method-parameterized `subscribe_with_method` with `subscribe`).
- **No cli-core change** — `subscribe_daemon` is method-agnostic (forwards `Event`
  frames regardless of method).
- `vox-gui` — `spawn_agent_event_stream` (subscribe_daemon `SUBSCRIBE_EVENTS` →
  `AppHandle::emit("vox://agent-events")`), started from `main.rs .setup()`;
  `transport.ts::listenAgentEvents` + `App.tsx` maps each `AgentEvent` into the
  Dashboard stream (capped at 100), giving the GUI **live token/lifecycle visibility**
  (previously only status snapshots).
- Test: `orchestrator_daemon_subscribe_events_streams_agent_events` (RED→GREEN) — emits
  a `TokenStreamed` on the bus and asserts the subscriber receives a `token_streamed`
  frame; all 4 daemon TCP tests pass. `cargo check -p vox-gui` clean; `vite build` passes.

**B4-chat — landed (2026-05-31), client-side, NO orchestrator change.** Scouting showed
the invasive `run_id` threading is unnecessary: `submit_orchestrator_task` already returns
a `task_id`, `task_started{task_id,agent_id}` seeds an `agent_id→task` map, `token_streamed`
(agent_id only) routes through it, and `task_completed/failed{task_id}` finalize — all
client-side (agent_id is consistent and tasks run serially per agent).
- `crates/vox-gui/ui/src/lib/chatCorrelation.ts` — a **pure reducer** owning the transcript:
  `submit` / `submitResolved` / `agentEvent` actions, `task_id` normalized to string. This is
  the trickiest logic (token-before-submit race, type mismatch), so it's unit-tested.
- **New frontend test capability**: added `vitest@^0.34` (vite-4 compatible) + `pnpm test`
  + `vitest.config.ts` scoping to `src/` (Playwright `e2e/` stays under `test:e2e`).
  `chatCorrelation.test.ts` — 5 tests, RED→GREEN.
- `App.tsx` — `useReducer(chatReducer)`; `handleLoquelaSubmit` dispatches `submit` (mints
  runId via an `executeIpcWithRun` `onRun` hook) then `submitResolved` with the returned
  `task_id`; the existing `listenAgentEvents` listener now feeds the chat reducer too (one
  listener, two consumers). `Transcript.tsx` renders the bubbles above the composer with
  streaming/failed states.
- Verified: `pnpm test` 5/5, `vite build` clean.
- Still deferred (minor): per-thread `session_id` grouping; multi-task-per-agent
  disambiguation (not possible today — serial per agent).

## B3 implementation log (2026-05-31) — interactive HITL approvals (in-process)

**Design decision.** Scouting found no existing await/wake mechanism (the clarification
inbox + `question_*` tables are poll/gate-based; A2A `Question/Answer` is fire-and-forget).
Two homes for a pending-approval registry: on `Orchestrator` (reachable from the daemon
`dispatch_request` — needed for *autonomous daemon* agents, via new `orch.resolve_approval`
RPCs) vs on `ServerState` (reachable from `handle_tool_call` — the GUI path). Since **B5
already runs MCP tools in-process** (`McpToolHost`'s `ServerState`), the GUI both awaits and
resolves in the same process — **no daemon RPC needed**. Chose the `ServerState` path.

**Landed + verified (TDD):**
- `vox-orchestrator-mcp/src/pending_approvals.rs` — `PendingApprovals` registry (in-memory:
  `oneshot` map keyed by id + a pending-metadata list). `register → (id, Receiver)`,
  `resolve(id, outcome)`, `cancel`, `list`. Reuses `vox_orchestrator::ApprovalOutcome`.
- `ServerState` gains `Arc<PendingApprovals>` (init in all constructors) — shared by the gate
  and the resolve/list tools.
- **Gate** (`dispatch.rs`, the RBAC chokepoint): a dangerous tool (`vox_run_shell`/`vox_deploy`/
  `vox_write_file`/…) *without* `user_approval:true` now **registers a pending approval and
  `.await`s** (300 s timeout → `TimedOut`); `Approved`/`Modified` → execute, else error. The
  `user_approval:true` fast path is unchanged (backwards-compatible).
- **Tools** `vox_pending_approvals` (list) + `vox_resolve_approval{approval_id, outcome}` —
  reach `ServerState.pending_approvals`.
- **GUI** Approvals surface (`ApprovalsView.tsx` + sidebar + view branch) polls
  `vox_pending_approvals` and Approve/Rejects via `vox_resolve_approval`, **reusing B5's
  `invoke_mcp_tool`** (no new Tauri command / daemon method).
- Tests: `pending_approvals_tests` — 3 registry unit tests + 1 **end-to-end gate** test
  (`vox_run_shell` parks → resolve Rejected → error envelope). `cargo check -p vox-gui` clean,
  `cargo run -p vox-arch-check` EXIT=0, `vite build` + `vitest` (5/5) pass.

**Follow-up — daemon dispatch backend landed (2026-05-31):** rather than move the registry
to `Orchestrator`, the daemon now exposes its own `ServerState` via an `ExtraDispatch` hook
(`vox-orchestrator::orch_daemon::ExtraDispatch` + `serve_listener_with_extra` /
`run_{tcp,stdio}_server_with_extra`; impl `vox-orchestrator-mcp::daemon_extra::McpExtraDispatch`,
wired in `vox-orchestrator-d`). New methods `orch.tool_call` (B5 path-c — run a tool through the
one shared orchestrator), `orch.list_pending_approvals`, `orch.resolve_approval` (B3
cross-process). Test `daemon_extra_tests` (orch.tool_call runs `vox_git_status`). All on the
daemon's single `ServerState`, no cycle (the trait lives in vox-orchestrator, the heavy state in
vox-orchestrator-mcp).

**Still deferred (gated on persistent-daemon lifecycle):** switching the GUI's `invoke_mcp_tool`
+ Approvals panel to the daemon path can't use the current `call_daemon` (it **spawns the daemon
per call** → the gate-await and the resolve would land in different processes/`ServerState`s). It
needs the GUI to manage one long-lived `vox-orchestrator-d` (TCP) and use `OrchDaemonClient` — a
separate piece. Until then the GUI keeps its working in-process `McpToolHost`. Also still open:
DB persistence (`hitl_approvals`) — most meaningful with the persistent-daemon path (in-process
parked calls die on restart regardless); and `agent_runs.approval_ref` wiring (the MCP gate has
no run_id today).

## Sequencing and dependencies

```text
C0 + A1/A2  ──►  honest, discoverable GUI (fast wins)
        │
B1 (event stream) ──► B2 (run store) ──► B4 (chat), B3 (HITL)
        │                       │
        └─────────► B5 (MCP), B6 (panels) ──► C2/C3/C4 design pass per promoted panel
```

- B1 is the spine: B3/B4/B6 all need push events.
- A1/A2 and C0/C1 are parallel-safe and can land immediately.
- Each promoted panel (B6) pairs with a C2–C4 design pass.

## Verification gates

- Migration: `cargo run -p vox-arch-check`, `vox ci gui-catalog-parity`,
  `vox ci check-links` must pass before commit.
- Per task: failing test first (contract or integration), then implementation; no
  fixture fallback may remain in a "promoted" panel.
- No stubs: a task that cannot be completed as a real artifact is scoped down to a
  smaller real one, not shipped hollow.

## Open questions

1. **Mobile**: ratify Tauri-mobile vs React Native/Expo vs runtime-contract-first. The
   plan assumes runtime-contract-first; a different answer changes B-track bindings.
2. **Streaming transport**: daemon protocol extension vs a reintroduced local WS channel
   for the event stream. ADR-037 forbids `fetch`/`WebSocket` *in the webview*; a
   daemon-side stream relayed through Tauri events stays compliant.
3. **Run-store scope**: minimal agent-run table now vs full timeline/artifact model
   up front.
