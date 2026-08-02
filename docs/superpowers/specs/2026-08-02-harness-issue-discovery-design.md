---
status: draft
---

# Harness Issue Discovery (Phase 1): Detect, Queue, Fix the Golden Corpus

## Problem

Vox chat/agent sessions repeatedly hit the same compiler errors, retry the same
failing tool call, or otherwise burn turns on correctable mistakes — and none
of that signal is ever captured. The `mens/` training pipeline that's supposed
to learn from this already has the seam for it
(`crates/vox-ml-cli/src/commands/mens/autofeedback.rs`, doc-commented as an
"autofeedback loop MVP") but it is an inert stub: it logs and returns static
success, and its backing file (`mens/data/mix_sources/autofeedback.jsonl`) has
zero lines despite being wired into `mens/config/mix-agents.yaml`.

Separately, the golden training corpus (`examples/golden/*.vox`,
`mens/data/mix_sources/golden.jsonl`) has no active mechanism to catch stale
or broken examples — entries can go stale (fail to compile against the
current grammar, carry a `last_validated` date far in the past) without
anyone noticing, silently teaching VOX MENS wrong patterns.

This spec builds a system that:

1. Watches live chat/agent sessions synchronously for repeated-correction
   patterns and classifies genuine issues via an LLM judge.
2. Statically scans the golden corpus, on demand, for staleness/compile
   failures.
3. Surfaces both as a persistent, reviewable queue in the GUI (toast +
   session-rail badge + inline transcript marker + a dedicated panel).
4. For golden-corpus issues, lets a human-approved decision dispatch a real
   model call that proposes a corrected corpus entry — shown as a diff,
   applied only after a second explicit approval.

## Out of scope (named follow-ups, not built here)

- **Dispatch-to-fix for harness/prompt config** and **dispatch-to-fix for Vox
  source code** — separate specs, reusing the same
  `scientia_harness_issues` → decision → dispatch shape this spec builds.
- **Continuous model-performance telemetry and auto-tuned model selection**
  ("measured intelligence" — token cost vs. outcome, auto-dialing model
  choice in auto mode). This is a distinct concern (continuous, not
  threshold-triggered; fix target is live routing, not a corpus diff) and has
  a cautionary history: the 2026-07-16 Axis GUI audit found two prior
  auto-model-selection engines (a 7-way resolver and an earlier `ModelPool`
  system) built with zero callers and marked for deletion — never reaching
  real routing. A follow-up spec will reuse this system's tables/toast/queue
  plumbing but must be scoped and reviewed on its own so it doesn't repeat
  that failure mode.
- **User-side repetition detection feeding Vox's memory system.** Detecting
  when a *user* (not the agent) repeats a request or preference across
  sessions, and capturing that into Vox's own knowledge-base/memory system
  (`knowledge_bases`/`kb_entries`/`kb_routing_rules` tables,
  `crates/vox-orchestrator/src/knowledge_base/`) so it doesn't need repeating
  — distinct from this spec's target (agent/compiler-error repetition feeding
  the training corpus). A follow-up spec, reusing the same detect → toast →
  review-queue plumbing built here.

## Architecture

```
Live chat session (run_agent_turn / agent_loop.rs)
  → per tool-call dispatch hook: cheap heuristic scorer (in-memory, per-session)
  → threshold crossed → synchronous LLM-judge call (vox_actor_runtime::llm)
  → writes scientia_harness_issues row (source=chat_session)
  → surfaces: global toast + session-rail badge + inline transcript marker

Golden corpus (examples/golden/*.vox, mens/data/mix_sources/*.jsonl)
  → on-demand "Scan training corpus" button in GUI
  → static scan (frontmatter staleness + compile-check)
  → writes scientia_harness_issues row (source=corpus_scan)
  → surfaces in the same review panel (no toast — not time-sensitive)

Either source → user reviews (toast or panel) → decision recorded
  (scientia_harness_decisions: confirmed | dismissed)
  → if confirmed AND source is corpus-fixable:
      dispatch LLM call proposes a corrected corpus entry (diff, not applied)
      → scientia_harness_fix_proposals row (status=pending_approval)
      → shown in panel for a second explicit approval
      → approved → diff written to disk, proposal marked applied
```

This lives alongside the existing Scientia discovery/review system
(`crates/vox-scientia`, `crates/vox-db/src/schema/domains/scientia.rs`,
`crates/vox-gui/ui/src/components/surfaces/Scientia/*`) as a new, independent
set of tables and GUI components in the same module/surface family — not
forced into the existing `scientia_discovery_inbox`/`scientia_review_decisions`
tables, which are tightly coupled to `publication_id`/`claim_id` and assume a
`publication_manifests` row exists. What genuinely carries over as-is: the
global toast queue (`crates/vox-gui/ui/src/lib/toastQueue.ts` +
`components/ui/Toasts.tsx`, rendered once at `App.tsx` root so it fires
regardless of active view) and the append-only decision-ledger shape
(`scientia_review_decisions` pattern).

## Data model

New `SchemaFragment`s in `crates/vox-db/src/schema/domains/scientia.rs`,
alongside the existing `scientia_discovery_inbox`/`scientia_review_decisions`
definitions (idempotent `CREATE TABLE IF NOT EXISTS`, no migrations file —
`crates/vox-db/src/schema/manifest.rs`'s `BASELINE_VERSION` bumped with a
changelog comment, per existing convention). No SQL `CHECK` constraints
(Turso/libSQL doesn't support them here — validated in Rust, matching
existing tables in this module).

**`scientia_harness_issues`**
| column | type | notes |
|---|---|---|
| `id` | text pk | |
| `source` | text | `chat_session` \| `corpus_scan` |
| `session_key` | text, nullable | null for `corpus_scan` |
| `detected_at_ms` | integer | |
| `category` | text | free-text from LLM judge (e.g. `repeated_compiler_error`) or scanner (e.g. `stale_frontmatter`, `compile_failure`) |
| `severity` | text | `low` \| `medium` \| `high` |
| `summary` | text | one-line, judge- or scanner-generated |
| `evidence_json` | text | signal history / offending file path(s) / error excerpt |
| `status` | text | `pending` \| `confirmed` \| `dismissed` |

**`scientia_harness_decisions`** (append-only, mirrors `scientia_review_decisions`)
| column | type | notes |
|---|---|---|
| `id` | text pk | |
| `issue_id` | text | fk → `scientia_harness_issues.id` |
| `decision` | text | `confirmed` \| `dismissed` |
| `actor` | text | |
| `reason` | text, nullable | |
| `decided_at_ms` | integer | |

**`scientia_harness_fix_proposals`** (only for corpus-fixable confirmed issues)
| column | type | notes |
|---|---|---|
| `id` | text pk | |
| `issue_id` | text | fk → `scientia_harness_issues.id` |
| `target_path` | text | e.g. `examples/golden/foo.vox` |
| `proposed_diff` | text | unified diff, not yet applied |
| `status` | text | `pending_approval` \| `applied` \| `rejected` |
| `proposed_at_ms` | integer | |
| `resolved_at_ms` | integer, nullable | |

Query/CRUD layer: new `crates/vox-db/src/store/ops_harness_issues.rs`
(typed row structs + raw `turso::params!`, matching
`ops_discovery_inbox.rs`/`ops_review.rs`).

## Detection mechanics

### Heuristic scorer (synchronous, in-process)

Hook point: the per-tool-call dispatch site inside `run_agent_turn`,
`crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:243-262`. This
already sees `call.name`, `call.arguments`, and the raw tool-result string —
including compiler/test errors from `compiler_tools.rs`'s `validate_file`,
`run_tests`, `check_workspace`, `build_crate`, `lint_crate` — before the
result is fed back to the model. No new subsystem watches from outside; this
is an in-process accumulator keyed by `session_key`, held for the life of the
session.

Two signals, each adding to a weighted score:

- **Repeated error signature**: hash of the first line of a tool-error string
  seen ≥2× for the same file within the session.
- **Retry loop**: same tool name + same arguments called ≥3 times
  consecutively.

`// ponytail: fixed threshold=3, revisit with a GUI-configurable slider only
if false-positive rate in practice warrants it` — a constant, not a rules
engine, given the history of over-built, never-wired routing/scoring systems
in this codebase (see Out of scope).

### LLM-judge pass (synchronous, threshold-triggered)

Fires only when the accumulator crosses the threshold — most turns pay zero
extra cost or latency. Calls through the existing model-agnostic boundary
(`vox_actor_runtime::llm`, per the standing architecture rule that all LLM
calls go through this layer) with the relevant recent tool calls/results,
asking for: category, severity, one-line summary, or an explicit "not a real
issue" verdict. On "not a real issue," nothing is written and the session's
accumulator resets. On a real issue, a `scientia_harness_issues` row is
written with `source=chat_session`, `status=pending`.

### Corpus static scanner (on-demand, manual trigger)

A "Scan training corpus" button (see GUI section) runs two concrete checks —
no stubs:

1. **Frontmatter staleness**: `examples/golden/*.vox` files whose
   `last_validated` date exceeds a fixed age threshold.
2. **Compile failure**: each golden `.vox` file compiled against the current
   compiler; failures are flagged.

Each finding becomes a `scientia_harness_issues` row with
`source=corpus_scan`. (The exact invocation — reusing `vox mens corpus`
tooling vs. a new thin wrapper — is a planning-time detail; the checks
themselves are fixed scope for v1.)

## Dispatch-to-fix (golden corpus only)

When a `scientia_harness_issues` row is confirmed (via toast or panel) AND
its source is corpus-fixable (either a `corpus_scan` finding, or a
`chat_session` finding the judge tagged as corpus-related — e.g. the agent
kept failing against a golden example that itself has stale syntax):

1. A model call (again via `vox_actor_runtime::llm`) is dispatched with the
   issue's evidence and the current corpus entry, asked to propose a
   corrected version.
2. The result is stored as a `scientia_harness_fix_proposals` row,
   `status=pending_approval` — never written to disk automatically.
3. The proposal appears in the GUI panel as a diff. A human must explicitly
   approve it.
4. On approval: the diff is applied to `target_path` on disk, and the
   proposal is marked `applied`. On rejection: marked `rejected`, no write.

This is real, end-to-end — not a placeholder action — per this project's
"no stub implementations" rule: better to build one dispatch target for real
than fake three.

## Config & default state

New reactive `vox_config` field: `harness_issue_detection_enabled`
(**default `true`** — on by default, opt-out, per explicit decision). This
gates only the synchronous chat-session heuristic+judge path; the corpus scan
button remains available regardless (it's already manual/on-demand, no
background cost).

Follows the existing `vox_config::snapshot` pattern already used for multiple
independent background-feature toggles (`scaling_enabled`,
`socrates_gate_enforce`/`socrates_gate_shadow`, `scope_enforcement`,
`exec_time_budget_enabled` — `crates/vox-gui/src/commands/orchestrator.rs`):
a new Tauri command pair (`get_harness_issue_detection_enabled`/
`set_harness_issue_detection_enabled`) writes the field into the orchestrator
TOML config and bumps `vox_config::snapshot`, and a `Toggle` in
`crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`
(registered in `settingsIndex.ts`, alongside the existing Auto-scaling
toggle) controls it live.

## GUI

**Global toast**: reuses the existing coalescing toast queue as-is
(`toastQueue.ts`/`Toasts.tsx`, `pushToast` threaded via
`SurfaceDecoratorProps`, rendered once at `App.tsx` root) — fires regardless
of which surface is currently active. Toast body: one-line summary + two
actions, "Yes, fix it" / "Dismiss." Both write a `scientia_harness_decisions`
row; "Yes" additionally triggers dispatch-to-fix if the issue is
corpus-fixable.

**Session-rail badge**: a small attention indicator on the relevant chat
session's list item in
`crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx`,
showing when that session has a `pending` `scientia_harness_issues` row. This
is new work — no existing badge/unread mechanism was found on that
component.

**Inline transcript marker**: the detected issue also appears inline in that
session's own message history, so scrolling back shows it in context. The
existing session event/frame architecture already injects non-message
content into a transcript this way (e.g. `cost_incurred` frames, per
`sessionChatStore.resolveSessionForEvent`); this adds a
`harness_issue_detected` frame type to that same mechanism. (Confirming the
exact frame-registration site is a planning-time task.)

**Review panel**: a new panel in the Scientia surface family (new
components — not `DiscoveryInbox.tsx`/`DiscoveryReview.tsx`, which are
hardcoded to publication/claim shapes with no domain field), listing
`scientia_harness_issues` rows (filterable by source/status/severity), with
a "Scan training corpus" button, and a detail view for confirming/dismissing
an issue and approving/rejecting a pending fix-proposal diff. Reachable from
`ScientiaDashboard.tsx` alongside the existing Discovery Inbox/Review tabs.

New Tauri commands (`crates/vox-gui/src/commands/`, new module e.g.
`harness_issues.rs`): `list_harness_issues`, `record_harness_issue_decision`,
`scan_training_corpus`, `list_fix_proposals`, `resolve_fix_proposal`.

## Testing

- **Heuristic scorer**: unit tests on the accumulator (threshold crossing,
  reset on judge "not a real issue," per-session isolation).
- **Schema**: new test file mirroring
  `crates/vox-db/tests/scientia_cost_phase_tests.rs`, covering the three new
  tables' round-trip CRUD.
- **Corpus scanner**: unit tests with fixture `.vox` files (one stale, one
  broken, one clean) asserting correct issue rows.
- **Dispatch-to-fix**: test the proposal → approval → apply path against a
  temp corpus fixture (never against real `examples/golden/`).
- **Frontend**: component tests for the review panel and session-rail badge;
  an e2e Playwright spec covering the full detect → toast → confirm → approve
  diff → applied flow, following existing `gui-playwright-smoke` conventions.

## Non-goals / risks carried forward

- No auto-apply of any fix without an explicit human approval step, anywhere
  in this spec.
- No new rules-engine or GUI-configurable threshold UI in v1 (ponytail:
  fixed constant, documented upgrade path).
- Toggle default is **on**, so per-turn heuristic scoring runs continuously
  once shipped; the LLM-judge cost is threshold-gated to keep this cheap in
  the common case, but this should be watched after rollout.
