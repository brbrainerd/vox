# Sidebar Session List — Design Spec

**Status:** Approved, then revised following an adversarial review pass (direct code verification +
4 independent blind reviewers across security/scope/test-coverage/operational-readiness dimensions).
The review found and this revision fixes: a migration-mechanism bug that would have broken app
startup for every existing user (§3), a task-badge data model built on a fabricated id convention
that real dispatch code never writes to (§4), a silently-dropped rename feature and non-functional
archived-toggle stub (§5), and a factually-incorrect claim about existing error-handling behavior
(§6). See the implementation plan's own revision note for the corresponding plan-side fixes.

## 1. Problem statement

The user's original mental model was "one continuous chat, dispatch and spawn agents as needed" —
no session-switching UI needed. An audit (direct code read + a scoped `graphify` graph over the
session-relevant files, `.graphify-session-scope/graphify-out/` in the main repo checkout) found
that model is already stale:

- [`ChatSessionRail.tsx`](../../../crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx)
  is a real, working multi-session list (create/rename/archive) backed by the `conversations` table
  — but it lives as a dockview panel *inside* the Chat surface, not in the global
  [`Sidebar.tsx`](../../../crates/vox-gui/ui/src/components/layout/Sidebar.tsx) (app navigation:
  Chat/Runs/Agents/Knowledge/Workspace/Commands/Compute/Mercatus/Settings — unrelated to switching
  chat sessions today).
- [`sessionChatStore.ts`](../../../crates/vox-gui/ui/src/lib/sessionChatStore.ts) is real
  infrastructure (per-session task/agent correlation, 30s/200-frame replay buffer for event races)
  built specifically so many concurrent sessions don't cross-talk — "one session → many dispatched
  tasks → many agents" is already the live model, just not exposed where the user expects it.
- Four weakly-linked "session" concepts exist in the backend (GUI `conversations` table,
  orchestrator in-memory `Session`, MCP `chat_history:{session_id}` KV store, `plan_sessions`).
  The graphify god-node analysis independently confirmed this: `Session` (orchestrator) and `VoxDb`
  are both high-degree bridge nodes connecting otherwise-separate graph communities — i.e. genuinely
  different subsystems stitched together by a thin seam, not one canonical entity.
- `conversations.repository_id` exists in schema but `chat_ensure_gui_session()` never sets it —
  every GUI session has `repository_id = NULL` today. No repo/worktree concept exists anywhere in
  `vox-gui/ui/src` (confirmed by full-tree grep).
- `plan_sessions`/`plan_versions`/`plan_nodes` (task-list-per-session) exist in schema
  ([`execution.rs`](../../../crates/vox-db/src/schema/domains/execution.rs)) and have a working
  `PlanPanel.tsx`/`TodosDockPanel`, but `App.tsx` hardcodes `chatPlanSessionId: null` — the panel
  is built but permanently unwired. **Correction from adversarial review:** the real link back to a
  chat session already exists and is already populated — `plan_sessions.origin_session_id` is set
  to the chat session's `external_session_id` by real orchestrator dispatch code
  ([`goal.rs:648`](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs)),
  with a randomly-minted `plan_session_id = format!("plan-{uuid}")` per dispatched goal
  ([`goal.rs:533`](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs)).
  A chat session can accumulate **multiple** `plan_sessions` rows over time (one per dispatched
  goal), not one. §4 below was originally written around a fabricated `chatplan-{id}` convention
  that no real dispatch code ever writes to; it has been corrected to join on `origin_session_id`.
- `chat_archive_conversation` (`vox-db/src/codex_chat.rs:668`) is a literal
  `DELETE FROM conversations WHERE id = ?1` — "Archive" is a hard, unrecoverable delete today.
- `TasksView.tsx`'s session filter reads a `vox_chat_sessions` `localStorage` key that nothing in
  the codebase ever writes — dead code.

Decision made in-session (see §Scope decisions below): reuse the existing `conversations`/
`ChatSessionRail` session model rather than invent a new "batch" entity (the heavier hopper-batch
direction from `docs/src/architecture/unified-task-hopper-research-2026.md` is explicitly deferred).

## 2. Scope decisions (from brainstorming Q&A)

| Question | Decision |
|---|---|
| Session entity | Same as today's `conversations`/`ChatSessionRail` rows — promoted to global sidebar scope, not a new grouping concept |
| Repo targeting | In scope for v1: start populating `repository_id`, group sidebar by repo |
| New-session repo assignment | Defaults to current repo/worktree context, no picker modal on create |
| Task lists | In scope for v1: fix `chatPlanSessionId`, add per-session task-count badge |
| Archive semantics | Fix to real soft-archive (`archived_at` column) — sessions are becoming a persistent nav surface, hard-delete is no longer acceptable |
| Sidebar layout | Sessions grouped under collapsible repo sub-headers (not a flat list with badges, not filter chips) |
| Overflow scope | **Per-repo-group** truncation — each repo sub-header has its own "Show N more" and its own scroll/expand state, independent of other repo groups (revised from an initial list-wide-scroll proposal) |
| Default visible count | 5 sessions per repo group before truncating |
| Archived sessions | Hidden by default; "Show archived" toggle at the bottom of the Chat group reveals a muted sub-list with Unarchive |

**Explicitly out of scope for this spec:** a repo *picker* UI for retargeting an already-created
session, hopper-batch-style multi-session grouping, mesh-wide session sync, resolving the
divergence between the four session-id spaces (they remain distinct — this spec only makes the
GUI `conversations` one navigable from the sidebar and repo/plan-aware).

**Open decision point for the implementation plan** (not resolved here): whether
`ChatSessionRail.tsx` inside the Chat surface is removed in favor of the sidebar, or kept as a
thin secondary view over the same store. Shipping two divergent session-switcher UIs is worth
avoiding; the plan should pick one and say why.

## 3. Data model changes (`vox-db`)

- `conversations.repository_id`: populate it. `chat_ensure_gui_session()` sets it from
  `vox_repository::discover_repository_or_fallback(cwd)`, called with the Tauri process's
  `std::env::current_dir()`. **Known limitation (adversarial review, security dimension):** this is
  process-global state, not a per-window/per-request value — unlike `runs.rs`'s
  `StartGuiRunInput.repo`/`.worktree`, which the frontend supplies *explicitly* per call rather than
  having the backend "resolve" it (the original phrasing here mischaracterized `runs.rs` as
  precedent for CWD-resolution; it is not). In a GUI process that has more than one
  repository/workspace open, all sessions created from that process get tagged with whichever repo
  the process's CWD happened to be at launch. Accepted for v1 because there is no existing
  app-wide "current workspace" signal to use instead and a real fix (passing an explicit workspace
  context through the whole GUI, or building one) is out of scope for a sidebar UI change — flagged
  here so it isn't rediscovered as a surprise later, and revisited if/when a multi-repo-workspace
  concept lands elsewhere in the app.
- **Known limitation:** `repository_id` (`vox_repository::compute_repository_id`) hashes
  `origin_url + canonical_root`. Renaming or moving a repo's local directory changes it, so sessions
  created before and after such a move land in two different sidebar groups for what the user
  considers one repo. No reconciliation is planned — flagged, not fixed, in v1.
- **Known limitation:** every session created before this feature ships has `repository_id = NULL`
  (confirmed — `chat_ensure_gui_session()` never sets it today) and there is no backfill (see §4's
  removed "lazily backfilled" language — that mechanism doesn't apply here). Pre-upgrade session
  history permanently renders under the "Other" pseudo-group (§5/§6) unless a user starts fresh
  sessions after upgrading.
- `conversations.archived_at: Option<DateTime>` — new column, nullable, default `NULL`.
  - `chat_archive_conversation(id)` becomes `UPDATE conversations SET archived_at = now() WHERE id = ?1`
    instead of the current `DELETE`.
  - New `chat_unarchive_conversation(id)`: `UPDATE conversations SET archived_at = NULL WHERE id = ?1`.
  - `chat_list_gui_sessions()` gains a `include_archived: bool` parameter; default listing filters
    `WHERE archived_at IS NULL`.
  - **`chat_find_gui_conversation_id`** (used by the find-or-create path in `chat_ensure_gui_session`)
    must also filter `archived_at IS NULL`. Without this, a resumed/deep-linked `external_session_id`
    pointing at an archived conversation would silently find and reuse it — writing new messages into
    an archived session with no unarchive step, and that session would stay hidden from the default
    list while quietly accumulating new content. With the filter, the same resumed id instead creates
    a fresh conversation row, which is the correct behavior for something the user archived.
  - **Migration mechanism (adversarial review, operational-readiness dimension — corrects an earlier,
    broken version of this spec).** `VoxDb::migrate()` only runs `baseline_sql()` — a batch of
    `CREATE TABLE IF NOT EXISTS` statements — for any database already at or below the current
    `BASELINE_VERSION` ([`open.rs:93-148`](../../../crates/vox-db/src/store/open.rs)). `CREATE TABLE
    IF NOT EXISTS` is a no-op against a table that already exists; it does **not** add new columns
    to it. Simply adding `archived_at` to the `conversations` DDL string and bumping
    `BASELINE_VERSION` does nothing for any pre-existing `.vox` database file — and an accompanying
    `CREATE INDEX ... ON conversations(archived_at)` in the same batch would hard-fail
    `execute_batch()` (no such column), breaking app startup entirely for every upgrading user on
    their first launch. This is a real, reachable failure mode, not a hypothetical — verified against
    the live `migrate()` code path, which every `VoxDb::open()`/`open_default()`/`open_remote()` call
    goes through. The implementation must add the column via an explicit, idempotent
    `ALTER TABLE conversations ADD COLUMN archived_at TEXT`, gated on a `PRAGMA table_info` check (a
    bare `ALTER TABLE ADD COLUMN` appended to the schema string would instead break *fresh* databases
    with a "duplicate column" error, since `CREATE TABLE` already includes the column there) — see the
    implementation plan's Task 1 for the exact steps. This repo's existing `auto_migrate`/`ddl/diff.rs`
    engine does real `ALTER TABLE ADD COLUMN` diffing already, but only for Vox-AST `@table`-declared
    schemas, a separate mechanism from the raw baseline-SQL tables this feature touches — it is not
    wired into `VoxDb::migrate()` and does not cover this table.
- No changes to `plan_sessions`/`plan_versions`/`plan_nodes` schema — they already support what's
  needed; the gap is purely on the wiring side (§4).

## 4. Fixing `chatPlanSessionId` (`vox-gui` + `vox-orchestrator`)

**Corrected by adversarial review.** The original version of this section invented a
`plan_session_id = format!("chatplan-{chat_session_id}")` convention and a
`create_paired_plan_session` call at chat-session-creation time. That id space is never written to
by real dispatch code and would have made the task badge always read zero. The real link already
exists: `plan_sessions.origin_session_id` is set to the chat session's `external_session_id` by
[`goal.rs:648`](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs)
whenever a task is actually dispatched from that session, with the `plan_session_id` itself minted
as `plan-{uuid}` per dispatched goal — so one chat session can have zero, one, or several
`plan_sessions` rows depending on how many goals have been dispatched from it, not exactly one.

- **No new row is created at chat-session-creation time.** A brand-new chat session simply has zero
  matching `plan_sessions` rows until the user dispatches a task from it, which is already how
  `plan_sessions` gets populated today.
- The task-count badge is `SELECT ps.origin_session_id, COUNT(*) FROM plan_sessions ps JOIN
  plan_nodes pn ON pn.plan_session_id = ps.plan_session_id AND pn.version = ps.current_version
  WHERE ps.origin_session_id IN (<chat session ids>) AND pn.status IN ('pending', 'queued',
  'in_progress') GROUP BY ps.origin_session_id` — summed across every `plan_sessions` row for that
  chat session, at each one's own current version, batched across all visible sessions in one query
  (not one round-trip per session).
- `App.tsx` stops hardcoding `chatPlanSessionId: null`. Clicking a session's task badge sets the
  active plan-panel target to that session's *most recently updated* `plan_sessions` row (there may
  be several); the panel scopes to that one plan session's DAG, same as today's `PlanPanel`/
  `TodosDockPanel` already expect.
- No "lazily backfilled" mechanism is needed — a session with no dispatched goals correctly has no
  badge (count is genuinely zero), and the first real dispatch creates its own `plan_sessions` row
  through the existing, unmodified dispatch path.

## 5. Sidebar component (`vox-gui/ui`)

- New `SessionSidebarSection` component, rendered inside `Sidebar.tsx` nested under the "Chat"
  `NavItem`, reusing its existing expand/collapse (`peekedParent`) state — no new top-level
  collapse mechanism invented.
- Sessions grouped by `repository_id` into collapsible sub-headers (repo name). Each sub-header
  independently shows its top 5 sessions (sorted by `updated_at` desc) plus its own
  "Show N more" affordance and its own scroll/expand state — expanding one repo's list does not
  affect another's.
- "+ New session" pinned above the repo groups. Creates immediately against the current
  repo/worktree context (§3) — no modal.
- **Rename must be retained.** `ChatSessionRail.tsx` (removed by this feature per the resolved open
  decision point above) currently ships inline rename-on-blur; `SessionSidebarSection` must carry
  the equivalent affordance (adversarial review, scope dimension: an earlier plan draft deleted
  `ChatSessionRail` without replacing rename anywhere, silently dropping the ability to rename a
  chat session from the GUI at all).
- "Show archived" link at the bottom of the whole Chat group must actually fetch and render the
  archived sub-list (still grouped by repo) with a working Unarchive action per row — not merely
  toggle a label. (Adversarial review: an earlier plan draft implemented this as a non-functional
  stub.)
- Reuses `activeSessionId`/session-list state already threaded through `App.tsx` — this is a
  second consumer of existing state, not a new state tree. No changes to `sessionChatStore.ts`'s
  per-session task/agent correlation logic.

## 6. Error handling

- Session create failure (backend unreachable, DB error): surface existing toast/error pattern
  used elsewhere in `ChatSurface.tsx`; sidebar entry point uses the same handler, not a new one.
- **Corrected by adversarial review:** the previous wording ("optimistic UI update rolls back on
  error") misdescribed `ChatSurface.tsx`'s actual current behavior. The real pattern
  ([`ChatSurface.tsx:671-690`](../../../crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx))
  is await-then-update: call `invoke`, update local state only on success, show a toast
  (`pushToast({ tone: 'warn', ... })`) on failure — there is no optimistic update or rollback
  anywhere in the existing rename/archive code. Rename/archive/unarchive/create in the new sidebar
  must follow this same await-then-update-on-success, toast-on-failure pattern, not introduce a new
  optimistic-with-rollback one.
- Archiving the currently-active session must reassign the active session to another remaining one
  (matching `ChatSurface.tsx:685`'s existing `if (activeId === sessionId && remaining.length > 0)
  onSessionChange?.(remaining[0].session_id)` behavior) — an earlier plan draft dropped this when
  extracting session logic into a shared hook.
- Missing/null `repository_id` (sessions created before this change, or created outside a resolved
  repo context): group under an "Other" pseudo-header rather than failing to render (see §3's related
  known limitations — this bucket is expected to be large and permanent for pre-upgrade sessions).

## 7. Testing

- `vox-db`: unit tests for `chat_archive_conversation`/`chat_unarchive_conversation` asserting rows
  survive archive (no `DELETE`) and are excluded/included correctly by `include_archived`; a test
  that an archived conversation's `external_session_id` is no longer found by
  `chat_find_gui_conversation_id` (so `chat_ensure_gui_session` creates a fresh row instead of
  reusing the archived one).
- `vox-db`: a migration-safety test that applies the *pre-`archived_at`* baseline to a fresh
  connection, then runs the post-change `migrate()` again and asserts the column exists and is
  queryable — a test against `connect_memory()` alone (fresh DB, version 0) cannot catch the
  existing-database upgrade failure described in §3, because a fresh DB never exercises the
  `current_version < BASELINE_VERSION` + already-has-the-table branch.
- `vox-gui` (Rust): test that the batched task-count query correctly sums open nodes across
  *multiple* `plan_sessions` rows sharing one `origin_session_id`, and correctly excludes nodes from
  a superseded plan version after `append_plan_version` bumps `current_version`.
- `vox-gui/ui`: component test for `SessionSidebarSection` covering per-repo-group truncation
  (independent "Show more" state across two repo groups, using distinct `updated_at` fixture values
  so sort order is actually exercised), the archived-list toggle actually rendering fetched archived
  sessions (not just flipping a label), rename, and the "Other" fallback for null `repository_id`.
- Manual verification in the running app (per this project's UI-change convention): **upgrade an
  existing `.vox` database file that predates this change** and confirm the app still starts and
  archive/unarchive work (this is the scenario §3's migration-mechanism fix specifically addresses
  and a fresh-DB dev environment will not naturally exercise); create sessions across two repos,
  confirm independent per-group truncation; archive the active session and confirm another session
  becomes active; confirm the task badge opens the right session's `PlanPanel` after dispatching a
  real task from that session.
