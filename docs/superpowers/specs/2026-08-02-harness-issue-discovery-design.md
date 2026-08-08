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

1. Watches each live chat/agent turn for repeated-correction patterns and
   classifies genuine issues via an LLM judge. (Compile-drift in the golden
   corpus is already caught in CI — see the Corpus static scanner section —
   so this spec's own scope for the corpus side is staleness only.)
2. Statically scans the golden corpus, on demand, for staleness.
3. Surfaces both as a persistent, reviewable queue in the GUI (toast +
   session-rail badge + inline transcript summary + a dedicated panel).
4. For golden-corpus issues, lets a human-approved decision dispatch a real
   model call that proposes replacement corpus content — shown as a diff for
   review, applied only after a second explicit approval.

## Out of scope (named follow-ups, not built here)

- **Dispatch-to-fix for harness/prompt config** and **dispatch-to-fix for Vox
  source code** — separate specs, reusing the same
  `scientia_harness_issues` → decision → dispatch shape this spec builds.
- **Dispatch-to-fix triggered by a chat-session-detected issue.** In v1,
  dispatch-to-fix only ever fires for issues that carry a resolved
  `target_path` — currently only golden-corpus staleness findings
  (`source = corpus_scan`) set one. Chat-session findings never do:
  reliably inferring which specific golden-corpus file a chat-session error
  actually relates to is a retrieval problem, not a labeling exercise, and
  is deferred to a follow-up rather than approximated in v1.
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
Live chat turn (run_agent_turn / agent_loop.rs)
  → per tool-call dispatch hook: cheap heuristic scorer (in-memory, per-turn)
  → threshold crossed → fire-and-forget LLM-judge call (vox_actor_runtime::llm)
  → writes scientia_harness_issues row (source=chat_session, target_path=null)
  → surfaces: global toast (click → navigate to panel) + session-rail badge
    (pending-only) + inline transcript summary strip

Golden corpus (examples/golden/*.vox — frontmatter staleness only; compile
drift is already gated in CI by examples_golden_doctor_green.rs, not
duplicated here)
  → on-demand "Scan training corpus" button in GUI
  → static staleness scan, deduped against existing pending findings
  → writes scientia_harness_issues row (source=corpus_scan, target_path=<file>)
  → surfaces in the same review panel (no toast — not time-sensitive)

Either source → user reviews in the panel → decision recorded
  (scientia_harness_decisions: confirmed | dismissed)
  → if confirmed AND target_path is set (v1: corpus_scan only):
      dispatch LLM call proposes replacement content for the corpus entry
      → scientia_harness_fix_proposals row (proposed_content + display-only
        proposed_diff, status=pending_approval)
      → shown in panel for a second explicit approval
      → approved → proposed_content written to disk (via vox_repository's
        path-safety helpers), proposal marked applied
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
| `id` | integer pk (autoincrement) | matches the convention every existing table in this schema module uses (`scientia_discovery_inbox`, `scientia_review_decisions`) — not text |
| `source` | text | `chat_session` \| `corpus_scan` |
| `session_key` | text, nullable | null for `corpus_scan` |
| `target_path` | text, nullable | repo-relative path when this issue is tied to a specific corpus file — always set for `corpus_scan`, null for `chat_session` in v1 (see Out of scope). Gates dispatch-to-fix eligibility. |
| `detected_at_ms` | integer | |
| `category` | text | free-text from LLM judge (e.g. `repeated_compiler_error`) or scanner (`stale_frontmatter`) |
| `severity` | text | `low` \| `medium` \| `high` |
| `summary` | text | one-line, judge- or scanner-generated |
| `evidence_json` | text | signal history / offending file path(s) / error excerpt — redacted via `vox_redact` before storage, since raw tool-call args/results can carry secrets |
| `status` | text | `pending` \| `confirmed` \| `dismissed` |

**`scientia_harness_decisions`** (append-only, mirrors `scientia_review_decisions`)
| column | type | notes |
|---|---|---|
| `id` | integer pk (autoincrement) | |
| `issue_id` | integer | fk → `scientia_harness_issues.id` |
| `decision` | text | `confirmed` \| `dismissed` |
| `actor` | text | |
| `reason` | text, nullable | |
| `decided_at_ms` | integer | |

**`scientia_harness_fix_proposals`** (only for issues with a non-null `target_path` — v1: `corpus_scan` staleness findings)
| column | type | notes |
|---|---|---|
| `id` | integer pk (autoincrement) | |
| `issue_id` | integer | fk → `scientia_harness_issues.id` |
| `target_path` | text | e.g. `examples/golden/foo.vox` |
| `proposed_content` | text | the full replacement file content — the actual source of truth for applying the fix |
| `proposed_diff` | text | a unified diff computed once for human display only; **never** parsed back into content. A unified diff with surrounding context lines cannot be losslessly reconstructed by filtering just the added lines — an earlier draft of this design tried exactly that and would have silently truncated approved files to only their changed lines. |
| `status` | text | `pending_approval` \| `applied` \| `rejected` |
| `proposed_at_ms` | integer | |
| `resolved_at_ms` | integer, nullable | |

Query/CRUD layer: new `crates/vox-db/src/store/ops_harness_issues.rs`
(typed row structs + raw `turso::params!`, matching
`ops_discovery_inbox.rs`/`ops_review.rs`) — plus round-trip tests in the
same in-file `#[cfg(test)] mod tests` style those two files already use,
rather than a separate file under `crates/vox-db/tests/`.

## Detection mechanics

### Heuristic scorer (in-process, scoped per turn)

Hook point: the per-tool-call dispatch site inside `run_agent_turn`,
`crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:243-262`. This
already sees `call.name`, `call.arguments`, and the raw tool-result string —
including compiler/test errors surfaced through the tool-dispatch path —
before the result is fed back to the model. No new subsystem watches from
outside; this is an in-process accumulator scoped to a single
`run_agent_turn` invocation (one chat turn's tool-call loop), not held across
turns or keyed by session. This codebase has no per-session shared-mutable-
state mechanism at this layer, and one turn's tool-call loop is already
where the target pattern — repeated errors or retries within one exchange —
actually shows up; a persistent, session-keyed accumulator would need real
new infrastructure for a signal this detector doesn't need to observe.

Two signals, each contributing at most one point per call (a single call is
never double-counted by both signals at once):

- **Repeated error signature**: hash of the first line of a tool-error
  string, seen ≥2× (for the same tool) within the turn.
- **Retry loop**: the same tool name + same arguments called ≥3 times
  consecutively (a genuinely broken streak — resets the moment a different
  call interrupts it).

`// ponytail: fixed threshold=3, revisit with a GUI-configurable slider only
if false-positive rate in practice warrants it` — a constant, not a rules
engine, given the history of over-built, never-wired routing/scoring systems
in this codebase (see Out of scope).

### LLM-judge pass (fire-and-forget, threshold-triggered)

Fires only when the accumulator crosses the threshold — most turns pay zero
extra cost or latency. Runs as a detached background task rather than
blocking the chat turn on judge latency (an earlier draft of this design
called this "synchronous," which would have added real latency to the
common case for no user-visible benefit). Calls through the existing
model-agnostic boundary (`vox_actor_runtime::llm`, per the standing
architecture rule that all LLM calls go through this layer) with the
relevant recent tool calls/results — redacted via `vox_redact` before
either the LLM call or storage, since raw tool arguments/results can carry
secrets — asking for: category, severity, one-line summary, or an explicit
"not a real issue" verdict. On "not a real issue" (or judge failure), nothing
is written and the turn's accumulator resets. On a real issue, a
`scientia_harness_issues` row is written with `source=chat_session`,
`target_path=null`, `status=pending`.

### Corpus static scanner (on-demand, manual trigger)

A "Scan training corpus" button (see GUI section) checks one thing — golden
frontmatter staleness — deliberately not compile correctness: every
`examples/golden/*.vox` file is already compiled on every CI run by
`crates/vox-audit/tests/examples_golden_doctor_green.rs` (via
`vox_compiler::pipeline::check_file`), which fails the build on any
regression. Re-checking that here would duplicate an already-authoritative
gate and would also require a new crate dependency edge this scanner has no
standing reason to add. The staleness check — files whose `last_validated`
date exceeds a threshold — has no existing coverage anywhere, which is what
makes it this scanner's actual job. It honors the same `// vox:skip`
opt-out `examples_golden_doctor_green.rs` does. Each new finding becomes a
`scientia_harness_issues` row with `source=corpus_scan`,
`target_path=<the file>`; repeat scans skip files that already have a
pending finding for the same path/category, so clicking the button
repeatedly doesn't flood the queue.

## Dispatch-to-fix (golden corpus only)

When a `scientia_harness_issues` row is confirmed (via panel — the toast
itself has no action buttons, see GUI section) AND it has a non-null
`target_path` — in v1 this means `corpus_scan` findings only, per the Out
of scope note above:

1. A model call (again via `vox_actor_runtime::llm`) is dispatched with the
   issue's evidence and the current corpus entry, asked to propose a
   corrected version. The full replacement content is stored directly
   (`proposed_content`), not reconstructed later from a diff.
2. The result is stored as a `scientia_harness_fix_proposals` row,
   `status=pending_approval` — never written to disk automatically.
3. The proposal appears in the GUI panel as a diff (display-only rendering
   of `proposed_content` vs. the current file). A human must explicitly
   approve it.
4. On approval: `proposed_content` is written verbatim to `target_path` on
   disk — through `vox_repository`'s path-safety helpers, which reject any
   `target_path` that resolves outside the repository root — and the
   proposal is marked `applied`. On rejection: marked `rejected`, no write.

This is real, end-to-end — not a placeholder action — per this project's
"no stub implementations" rule: better to build one dispatch target for real
than fake three.

## Config & default state

New reactive `vox_config` field: `harness_issue_detection_enabled`
(**default `true`** — on by default, opt-out, per explicit decision). This
gates only the per-turn heuristic+judge path; the corpus scan button remains
available regardless (it's already manual/on-demand, no background cost).

Follows the existing config-field pattern already used for multiple
independent background-feature toggles (`scaling_enabled`,
`socrates_gate_enforce`/`socrates_gate_shadow`, `scope_enforcement`,
`exec_time_budget_enabled` — `crates/vox-gui/src/commands/orchestrator.rs`):
`set_orchestrator_config`/`get_orchestrator_config` write the field into the
orchestrator TOML config and bump `vox_config::snapshot`, and a `Toggle` in
`crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`
(registered in `settingsIndex.ts`, alongside the existing Auto-scaling
toggle) controls it live. On the read side inside the running MCP process,
the detector reads the field through the orchestrator's live
`config_handle()` — not a boot-time-copied config struct field — which is
what actually makes the toggle reactive rather than requiring a restart.

## GUI

**Global toast**: reuses the existing coalescing toast queue as-is
(`toastQueue.ts`/`Toasts.tsx`, `pushToast` threaded via
`SurfaceDecoratorProps`, rendered once at `App.tsx` root) — fires regardless
of which surface is currently active. The existing `Toast` type has no
action-button concept (it's `{tone, title, body, cause}`), so rather than
inventing new UI it doesn't support, the toast is informational and, on
click, navigates to the Harness Issues panel — reusing the same
`CustomEvent('vox://navigate-surface', ...)` pattern `DiscoveryInbox.tsx`
already uses for its own row-click navigation. The actual confirm/dismiss
decision (and, where eligible, the fix-proposal review) happens in the
panel, not the toast itself.

**Session-rail badge**: a small attention indicator on the relevant chat
session's list item in
`crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx`,
showing when that session has a `pending` `scientia_harness_issues` row —
deliberately pending-only, so a confirmed/dismissed issue doesn't leave a
stale dot. This is new work — no existing badge/unread mechanism was found
on that component.

**Inline transcript summary**: detected issues for the active session render
as a small summary strip at the top of that session's transcript, fetched
by session key and refreshed on the same poll cadence as the other
surfaces. This is a read-time merge, not an injection into the shared
message-timeline data model: the existing timeline builder
(`chatTranscriptTimeline.ts`) assigns messages small ordinal timestamps
(`index * messageStepMs`), not real epoch milliseconds, so a harness issue's
real `detected_at_ms` cannot be meaningfully sorted into that same sequence
— attempting precise interleaving (an earlier draft of this design assumed
it could) would have silently sorted every issue after every message,
regardless of when it was actually detected. Shown as a summary strip
avoids promising an ordering the underlying data can't support. Includes
dismissed/confirmed issues too (visually distinguished), since this view is
a historical record of the session, distinct from the badge's
attention-only purpose.

**Review panel**: a new panel in the Scientia surface family (new
components — not `DiscoveryInbox.tsx`/`DiscoveryReview.tsx`, which are
hardcoded to publication/claim shapes with no domain field), listing
`scientia_harness_issues` rows (filterable by status/source), with a "Scan
training corpus" button, and per-issue confirm/dismiss actions plus a
fix-proposal review (diff display, approve/reject) for issues with a
`target_path`. Reachable from `ScientiaSurface.tsx` as a new tab alongside
its actual existing tabs, Dashboard and Claims (not "Discovery Inbox/Review,"
which don't exist on that surface).

Seven new Tauri commands in `crates/vox-gui/src/commands/harness_issues.rs`:
`list_harness_issues`, `list_harness_issues_for_session`,
`record_harness_issue_decision`, `scan_training_corpus`,
`propose_harness_issue_fix`, `list_harness_fix_proposals`,
`resolve_harness_fix_proposal`.

## Testing

- **Heuristic scorer**: unit tests on the accumulator (threshold crossing
  from each individual signal, no call double-counted by both signals at
  once, streak reset on an interleaved different call, reset after a judge
  verdict) plus an explicit test that the config toggle actually stops
  detection when off — a kill switch with no test proving it kills anything
  is not verified.
- **Schema**: in-file round-trip tests in each new `ops_harness_*.rs` file,
  matching the existing convention in `ops_discovery_inbox.rs`/`ops_review.rs`
  (not a separate file under `crates/vox-db/tests/`).
- **Corpus scanner**: unit tests with fixture file content (stale, recent,
  missing-field, `// vox:skip`-annotated) asserting correct staleness
  findings, plus a dedup test proving repeat scans don't duplicate a pending
  finding for the same file.
- **Dispatch-to-fix**: a regression test proving the apply path writes
  `proposed_content` verbatim rather than reconstructing it from the diff
  (the exact bug an earlier draft of this design had), against a temp
  fixture — never against real `examples/golden/`. A path-traversal test
  proving an escaping `target_path` is rejected before any read/write.
- **Frontend**: component tests for the review panel and session-rail badge;
  an e2e Playwright spec covering the panel-based detect → confirm → propose
  → approve → applied flow (the toast's role in this flow is discovery/
  navigation only, not decision-making, so the e2e spec exercises the panel
  directly), following existing `gui-playwright-smoke` conventions. The
  Rust-side scorer/judge/gate wiring itself has no live-LLM integration
  test anywhere in this spec by design — its correctness is covered by the
  scorer and judge unit tests plus the kill-switch test above, not an
  end-to-end run against a real model.

## Non-goals / risks carried forward

- No auto-apply of any fix without an explicit human approval step, anywhere
  in this spec.
- No new rules-engine or GUI-configurable threshold UI in v1 (ponytail:
  fixed constant, documented upgrade path).
- Toggle default is **on**, so per-turn heuristic scoring runs continuously
  once shipped; the LLM-judge cost is threshold-gated to keep this cheap in
  the common case, but this should be watched after rollout — the toggle is
  read from the orchestrator's live, reactively-updated config handle (not
  a boot-time snapshot), so flipping it in Settings takes effect immediately
  without a restart.
- LLM calls in this feature (the judge and the fix-dispatch prompt) route
  through the standard `provider: "auto"` model-agnostic boundary like every
  other call site in this codebase, but neither classifies its content
  against `vox-orchestrator`'s privacy-router (`force_local_for_private`)
  the way some other subsystems do. That gap is pre-existing across the
  whole LLM boundary, not something this feature introduces — flagged here
  as a cross-cutting follow-up rather than blocking this spec.
