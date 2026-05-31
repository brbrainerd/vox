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
