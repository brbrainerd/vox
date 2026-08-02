# Sidebar Session List — Design Spec

**Status:** Approved (brainstormed and approved in-session; see conversation history for the
question-by-question decisions this spec encodes).

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
  is built but permanently unwired.
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

- `conversations.repository_id`: populate it. `chat_ensure_gui_session()` sets it from the current
  repo/worktree context at session-creation time (reuse whatever the GUI process already resolves
  this from for other repo-scoped features, e.g. `runs.rs`'s `StartGuiRunInput.repo`/`.worktree`).
- `conversations.archived_at: Option<DateTime>` — new column, nullable, default `NULL`.
  - `chat_archive_conversation(id)` becomes `UPDATE conversations SET archived_at = now() WHERE id = ?1`
    instead of the current `DELETE`.
  - New `chat_unarchive_conversation(id)`: `UPDATE conversations SET archived_at = NULL WHERE id = ?1`.
  - `chat_list_gui_sessions()` gains a `include_archived: bool` parameter; default listing filters
    `WHERE archived_at IS NULL`.
  - Migration: additive column, no backfill needed (existing rows get `NULL` = not archived).
- No changes to `plan_sessions`/`plan_versions`/`plan_nodes` schema — they already support what's
  needed; the gap is purely on the wiring side (§4).

## 4. Fixing `chatPlanSessionId` (`vox-gui` + `vox-orchestrator`)

- On session create (`chat_create_session`), also create a `plan_sessions` row with
  `origin_session_id` set to the new conversation's id, and return its `plan_session_id` in
  `ChatSessionDto` (new field).
- `App.tsx` stops hardcoding `chatPlanSessionId: null` — it comes from the active session's DTO.
- Sidebar session row renders a task-count badge = count of `plan_nodes` for that session's
  `plan_session_id` where `status != 'done'`. Clicking the badge (not the row) opens the existing
  `PlanPanel`/`TodosDockPanel` scoped to that session — this is the functional fix, the panel
  component itself does not need to change.
- Sessions created before this change have no `plan_session_id`: badge renders as absent/zero,
  lazily backfilled (create-on-first-open) rather than a bulk migration.

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
- "Show archived" link at the bottom of the whole Chat group reveals a muted archived sub-list
  (still grouped by repo) with an Unarchive action per row.
- Reuses `activeSessionId`/session-list state already threaded through `App.tsx` — this is a
  second consumer of existing state, not a new state tree. No changes to `sessionChatStore.ts`'s
  per-session task/agent correlation logic.

## 6. Error handling

- Session create failure (backend unreachable, DB error): surface existing toast/error pattern
  used elsewhere in `ChatSurface.tsx`; sidebar entry point uses the same handler, not a new one.
- Archive/Unarchive failure: optimistic UI update rolls back on error, matching existing
  rename-on-blur error handling in `ChatSessionRail.tsx`.
- Missing/null `repository_id` (sessions created before this change, or created outside a resolved
  repo context): group under an "Other" pseudo-header rather than failing to render.

## 7. Testing

- `vox-db`: unit tests for `chat_archive_conversation`/`chat_unarchive_conversation` asserting rows
  survive archive (no `DELETE`) and are excluded/included correctly by `include_archived`.
- `vox-gui` (Rust): test that `chat_create_session` creates a paired `plan_sessions` row and returns
  its id.
- `vox-gui/ui`: component test for `SessionSidebarSection` covering per-repo-group truncation
  (independent "Show more" state across two repo groups), archived-list toggle, and the "Other"
  fallback for null `repository_id`.
- Manual verification in the running app (per this project's UI-change convention): create sessions
  across two repos, confirm independent per-group truncation, archive/unarchive a session, confirm
  the task badge opens the right session's `PlanPanel`.
