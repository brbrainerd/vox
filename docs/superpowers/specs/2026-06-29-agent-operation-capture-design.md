---
title: Agent Operation Capture (telemetry for skill suggestion)
date: 2026-06-29
status: design
audience: contributors
---

# Agent Operation Capture

## Context

This is **sub-project 1 of 4** in the "agent-authored skills from repeated
operations" program. The end goal: when an agent repeats the same operations,
propose authoring a skill (HITL) to save tokens/time. That program decomposes as:

1. **Operation capture (this spec)** — record every tool call (redacted) as a
   mineable signal.
2. Sequence mining — detect repeated operation sequences over the captured stream.
3. HITL proposal — turn a cluster into a `DraftFrontmatter` proposal via the
   existing `FeedbackStore` / `Surface::NeedsYou`.
4. Accept → author → install — synthesize a SKILL.md and install via the shipped
   `install_to_user_root`.

Nothing detects repetition without the signal, so capture is the foundation. It
is also independently useful as local observability.

This is **local-only** capture for skill suggestion — distinct from the existing
opt-in centralized server telemetry (`vox-telemetry` → `vox-server`).

## Problem

Per-tool-call inputs/results are not recorded anywhere today. `agent_session_events`
captures task lifecycle and ~50 event kinds, but not tool calls with their args.
Without that stream, sub-projects 2–4 have nothing to mine.

## Goal

Capture every MCP tool call as a redacted, size-capped row in vox-db, written
best-effort off the hot path, gated by a config flag (default on). No mining, no
UI, no proposals — those are later sub-projects.

## Decisions (locked during brainstorming)

- **Capture point:** wrap the single dispatch chokepoint `handle_tool_call`
  (`vox-orchestrator-mcp/src/dispatch.rs`), so every tool is captured uniformly
  with no per-tool instrumentation.
- **Redaction:** layered scrub — a secret-ish-key denylist plus the existing
  `redact_owned` pattern redactor — applied to args AND result before persist.
- **Enablement:** on by default; a `config.toml` flag disables it.
- **Writes:** async, fire-and-forget, best-effort — never block, slow, or fail a
  tool call.

## Architecture

```
handle_tool_call(name, args)
   ├─ start = now;  result = <inner dispatch>(name, args)     ← unchanged, returned as-is
   └─ if capture_enabled:  spawn {                            ← fire-and-forget; all errors swallowed
          rec = redact(name, args, result, session, agent, dur)
          db.record_operation(rec);  db.prune_operations()
      }
   return result
```

The capture path is strictly additive: the inner dispatch result is returned
verbatim regardless of capture success or failure.

## Components

### 1. `vox-redact` (new tiny crate)

The reusable redactor `redact_owned` currently lives in
`vox-terminal-core/src/corpus/redact.rs`. Importing `vox-terminal-core` from
`vox-orchestrator-mcp` would be a backwards/awkward dependency edge, so promote
the redactor to a new leaf crate `vox-redact`:

- `pub fn redact_owned(text: &str) -> String` — moved verbatim (emails,
  bearer/api-key/token patterns, private IPv4, home paths).
- `pub fn redact_args(value: &serde_json::Value) -> serde_json::Value` — NEW:
  recursively walk the JSON; for object entries whose key (lowercased) contains
  any denylist substring, replace the value with `"[REDACTED]"`; otherwise run
  `redact_owned` over string scalars. Arrays/nested objects recurse.

`vox-terminal-core` then depends on `vox-redact` and re-exports / calls it, so its
existing callers and tests keep working unchanged.

**Denylist (case-insensitive substring on the key):** `token`, `key`, `secret`,
`password`, `passwd`, `authorization`, `auth`, `credential`, `apikey`, `bearer`,
`cookie`, `session`.

### 2. `operation_capture` module (in `vox-orchestrator-mcp`)

- `OperationRecord { ts_ms, session_id: Option<String>, agent_id: Option<String>,
  tool_name: String, args_redacted: String, result_redacted: Option<String>,
  duration_ms: u64, is_error: bool }`
- A builder that: serializes args, runs `redact_args`, serializes back to a
  string; runs `redact_owned` over the result string; truncates each of
  `args_redacted` / `result_redacted` to a cap (8 KB) with a `…[truncated]`
  marker; spawns the DB write.
- `capture_enabled(state)` reads the config flag once.

### 3. DB store (`vox-db`)

New table `agent_operations` + `record_operation(&OperationRecord)` +
`prune_operations()`, mirroring the existing `record_agent_event` store pattern.

```sql
agent_operations(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  ts_ms INTEGER NOT NULL,
  session_id TEXT,
  agent_id TEXT,
  tool_name TEXT NOT NULL,
  args_redacted TEXT NOT NULL,
  result_redacted TEXT,
  duration_ms INTEGER,
  is_error INTEGER NOT NULL DEFAULT 0
);
-- indexes: (session_id, ts_ms), (tool_name)
```

`prune_operations()`: delete rows older than 30 days, then if the table still
exceeds 50,000 rows, delete the oldest down to 50,000. Cheap, opportunistic,
called after each write.

### 4. Config

Add `capture_enabled: bool` (default `true`) under an `[operations]` section of
the orchestrator config. Read at the capture site; when false, the spawn block is
skipped entirely.

## Data flow

1. `handle_tool_call(state, name, args)` records `start`.
2. Inner dispatch runs; produces `result: String` (and whether it was an error).
3. If `capture_enabled(state)`: spawn a task that builds the redacted
   `OperationRecord` and calls `record_operation` + `prune_operations`. The
   spawned task owns clones of the needed data; it never holds up the return.
4. Return the inner `result` unchanged.

## Session / agent context (open implementation detail)

`handle_tool_call` may not have `session_id` / `agent_id` in scope. The plan will
resolve the source — a `ServerState` field if one exists, else capture at
`agent_id` granularity, else leave both `NULL`. Both columns are nullable;
capture degrades gracefully and is never blocked by missing context.

## Error handling

Capture is best-effort. Redaction, serialization, or DB errors are logged at
`debug` and swallowed. If the DB handle is absent, capture is skipped. The tool
call's result and latency are unaffected in all cases.

## Testing

- **Redactor (`vox-redact`):**
  - value under a denylist key (e.g. `api_key`, `Authorization`) → `[REDACTED]`.
  - bearer/api-key token inside a free-text string scrubbed by `redact_owned`.
  - nested objects/arrays recurse; non-secret values preserved verbatim.
  - existing `redact_owned` tests move with the code and still pass.
- **Capture builder:** args+result over the cap are truncated with the marker.
- **Best-effort guarantee:** a forced `record_operation` error still returns the
  inner tool result unchanged (wrap-level test).
- **Config flag:** `capture_enabled = false` ⇒ no row written.
- **Store/migration:** `agent_operations` table + indexes exist; `record_operation`
  round-trips; `prune_operations` enforces the age and row-count caps.

## Out of scope (later sub-projects)

- Sequence mining (2), HITL proposal (3), auto-authoring + install (4).
- Any operations-browser UI.
- Server upload — that is the separate, opt-in `vox-telemetry` path.
- Capturing non-MCP actions (e.g. raw CLI commands outside the MCP dispatch).
