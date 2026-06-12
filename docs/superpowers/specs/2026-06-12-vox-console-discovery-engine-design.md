# Vox Console — discovery engine design (2026-06-12)

## Summary

A new top-tier surface in the existing Tauri GUI (`vox-gui`): **Vox Console**, a
Warp-model discovery terminal. The console owns its input line (enabling
fish-style ghost-text autosuggest), runs a real PTY underneath for full
terminal control with multiple tabs, and keeps a persistent discovery rail
that live-updates as the user types with help, examples, and
spaced-repetition tips. Agents are first-class: a strip shows open agents,
any agent opens as a live tab, and output blocks can be sent to agent
inboxes over the existing A2A path. The goal is to make Vox's ~470 CLI
entries discoverable by typing, and to have that discovery surface grow
automatically as Vox grows.

Decision context: Warp went open-source on 2026-04-28, but its core is
AGPLv3 (license contamination) and its UI framework is a parallel stack to
Tauri+React. We copy Warp's *architecture trick* — the app owns the input
editor and only uses the PTY for execution — rather than its code.

## Goals

1. Fish-style as-you-type discovery of `vox` commands (ghost text,
   tab-completion, arg hints) plus history-based ghost text for arbitrary
   shell commands.
2. Full terminal control: real shell, multiple tabs, blocks with per-block
   actions.
3. Help-as-you-type in a persistent, collapsible right rail.
4. A local, deterministic learning model: what has this user seen/used, for
   how long (dwell), and when should an unexplored command resurface
   (FSRS-style spaced repetition).
5. Agent visibility and wiring through the *same orchestrator daemon* the
   Dashboard uses — one agent identity everywhere.
6. Zero-maintenance growth: new CLI commands appear in suggestions, help,
   and the discovery ledger with no console changes.

## Non-goals (v1)

- LLM-powered next-need prediction or co-occurrence mining (schema leaves
  room; see Future work).
- Forking/embedding Warp or adopting its UI framework; Warp Workflows
  export.
- Mobile (desktop-only ADR stands).
- Replacing the existing Catalog forms surface — the console is its
  discovery-first sibling, not a replacement.
- Remote/web access to the console (Tauri desktop only).

## Architecture

```
clap catalog + catalog.v1.yaml ──► action manifest (existing spine)
        │                                    │
        ▼                                    ▼
vox-gui backend (Rust)              vox-gamify::discovery (new module)
  • PTY manager (portable-pty,        • exposure ledger (seen/used/dwell)
    one per tab, new)                 • FSRS-style scheduler
  • suggest/help Tauri cmds (new)     • fed via existing route_event
  • agent/status streams (existing)
        │
        ▼
Console surface (React, new)
  • tabs + blocks (xterm.js, OSC 133 markers)
  • input editor (ghost text; shell sees nothing until Enter)
  • discovery rail (help, examples, tips; records seen + dwell)
  • agent strip / agent tabs / send-to-agent composer / inbox badge
```

### Terminal core

- **PTY manager** (`vox-gui/src`, new module): `portable-pty` (ConPTY on
  Windows), default shell `pwsh` (configurable), one PTY per tab. Output
  streams to the frontend as Tauri events; submitted lines are written back.
  All child spawns follow the `CREATE_NO_WINDOW` discipline
  (`quiet_command` pattern).
- **Blocks**: each tab renders the PTY stream in xterm.js. OSC 133 shell
  integration markers (injected via shell profile snippet, the VS Code/Warp
  technique) delimit command/output blocks. Block affordances: copy, re-run,
  send-to-agent. If markers are unavailable (user-chosen exotic shell), the
  tab degrades to plain scrollback — input editor and rail still work.
- **Input editor** (React): sits below the xterm viewport and owns every
  keystroke until Enter. Submitted lines go to the PTY verbatim, so
  anything (git, cargo, vox) runs for real. Running `vox` commands also
  emits the existing universal Ludus ActionEvents from the CLI itself —
  no double-instrumentation.

### Suggestion engine

- Candidates:
  - `vox …` input → action manifest / command catalog (commands,
    subcommands, args with types from clap metadata).
  - everything else → per-user shell history (this is most of the fish
    feel).
- Ranking = frecency (usage counters) + novelty boost (never-seen commands
  related to the typed prefix/group) + spaced-repetition due-ness.
- Render: best candidate as inline ghost text (Tab/→ accepts); alternates
  in a compact list under the prompt. Debounced, computed locally
  (manifest preloaded at surface mount; history + discovery state via
  Tauri commands). Typing never blocks on suggestions.

### Discovery rail

- Resolves the token under the cursor to its manifest entry and shows:
  description, args with help text, one example invocation, safety class,
  related commands (same group + same capability), and a tips slot
  ("never tried: `vox scientia cost`").
- Records exposure: which entries were displayed, and dwell time (visible
  ≥ 2 seconds while the user is active; constant in one place). Fire-and-forget writes through
  the gamify event router.
- Collapsible; persistence of collapse state per user.

### Discovery ledger + spaced repetition (vox-gamify::discovery)

- New module in `vox-gamify` (it already owns usage counters, hint
  telemetry, and the single `route_event` entry point).
- One Codex table keyed by action-manifest id:
  `seen_count, used_count, last_seen, last_used, dwell_ms_total,
  fsrs_state (stability, difficulty, due_at)`.
- Events: `discovery.seen` (rail display w/ dwell), `discovery.used`
  (command executed), `discovery.tip_shown`, `discovery.tip_engaged`.
  Idempotency via the existing `ludus_dedupe_id` mechanism.
- Scheduler: FSRS-style update on each event; "due" unexplored commands
  feed the tips slot and the suggestion novelty boost. Deterministic, no
  LLM, fully local (telemetry sensitivity: S1, local-only per ADR 023).
- Migration: `BASELINE_VERSION` bump in `manifest.rs` per SSOT §5.5 —
  no date-stamped SQL files.
- Growth: rows are created lazily on first sight of a manifest id, so new
  CLI commands join the ledger automatically; removed commands age out
  (ids absent from the manifest are excluded from tips).

### Agent integration

- **Agent strip**: chips with live agent state (queued/in-progress/cost),
  fed by the existing `vox://orch-status` + `vox://agent-events` Tauri
  streams — the same source the Dashboard uses, so the two cannot
  disagree.
- **Agent tabs**: a chip opens a tab streaming that agent's events/activity
  via the daemon RPC (`subscribe_events`). Spawning is just running
  `vox orchestrate …` in the console.
- **Send-to-agent**: block context menu → composer → existing A2A path
  (`a2a_messages` Codex table via daemon; unicast or broadcast). Inbox
  badge surfaces unread replies/clarification requests in the tab bar.
- **Cross-menu wiring**: Dashboard agent rows gain "Open in Console";
  console agent tabs deep-link back to the Dashboard view. One agent
  identity everywhere; no second orchestrator.
- **Registry**: console registers in `contracts/gui/surface-registry.v1.yaml`
  as `live_backend` with a `view_key` in `App.tsx`; the
  `vox ci gui-surface-registry` gate enforces wiring.

## Error handling & degradation

- PTY death → exit banner in the block stream + one-click restart.
- Orchestrator daemon down → agent strip shows offline badge (existing GUI
  pattern); terminal and discovery keep working.
- Codex unavailable → ledger writes are fire-and-forget; prompt and
  suggestions (manifest + in-memory history) unaffected.
- Suggestion latency → debounce + local-first; ghost text simply doesn't
  appear if a query is slow. Never block keystrokes.

## Testing

- Rust unit: FSRS scheduler transitions, exposure ledger upserts,
  suggestion ranking (frecency/novelty/due-ness ordering), OSC 133 marker
  parser.
- Rust integration: PTY manager spawn/write/read round-trip (Windows
  ConPTY), `CREATE_NO_WINDOW` audit.
- Vitest: input editor (ghost accept/reject, Tab cycling, history vs
  manifest candidates), rail resolution from cursor token, agent strip
  rendering from fixture streams.
- Gates: `vox ci gui-surface-registry`, `vox-arch-check`, ssot-drift,
  existing GUI Playwright sweep extended with a console smoke test.

## Where things live

| Concept | Location |
|---|---|
| PTY manager, suggest/help Tauri commands | `crates/vox-gui/src` (new modules) |
| Console surface (tabs, blocks, input editor, rail, strip) | `crates/vox-gui/ui/src/components/surfaces/Console/` |
| Discovery ledger + FSRS scheduler | `crates/vox-gamify/src/discovery/` (new module) |
| Suggestion candidates source | existing `vox_cli::command_catalog` + action manifest |
| Agent streams / A2A | existing `vox-orchestrator(-d/-mcp)` paths, reused |

`docs/src/architecture/where-things-live.md` gets the new rows in the same
PR that introduces the code.

## Future work (explicitly deferred)

- Next-need prediction: command co-occurrence mining over the ledger;
  optional LLM suggestions via the `vox_actor_runtime::llm` facade.
- Warp Workflows YAML export from the catalog (cheap external integration).
- Console over the HTTP gateway for the web dashboard.
