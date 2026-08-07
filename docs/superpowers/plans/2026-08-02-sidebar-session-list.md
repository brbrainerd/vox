# Sidebar Session List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Revision note (adversarial review):** this plan was rewritten after a review pass combining direct
code verification and 4 independent blind reviewers (security, scope/YAGNI, test-coverage,
operational-readiness). Two blocker-level defects were found and fixed here: (1) the original Task 1
would have broken app startup for every user with a pre-existing database (`CREATE TABLE IF NOT
EXISTS` does not add columns to an existing table, and the accompanying `CREATE INDEX` on the new
column would hard-fail `execute_batch()`); (2) the original Tasks 4/5/6/10 built the task-count badge
on a fabricated `chatplan-{id}` plan-session convention that real orchestrator dispatch code never
writes to (real dispatch links back via the existing `plan_sessions.origin_session_id` column), so
the badge would always have read zero. Both are fixed below. Also fixed: a silently-dropped rename
feature, a non-functional "Show archived" stub, a spec-vs-code mismatch on error-handling pattern,
and several test-coverage gaps. Task 11 (unrelated `TasksView.tsx` dead-code fix) was removed as
scope creep — see the changelog at the end of this document.

**Goal:** Move chat-session switching out of the Chat-surface-internal dockview panel and into the global `Sidebar.tsx`, grouped by repository with independent per-repo-group overflow, wired to real per-session task lists, with Archive changed from a hard delete to a recoverable soft-archive.

**Architecture:** Three layers, bottom-up. (1) `vox-db`: add `conversations.archived_at` via a real existing-database-safe migration, start populating the already-existing `conversations.repository_id`, and read task-badge counts through the *existing* `plan_sessions.origin_session_id` link rather than inventing a new one. (2) `vox-gui` Tauri commands: extend `ChatSessionDto`, resolve the current repo via `vox-repository`, expose archive/unarchive/batched-task-count. (3) `vox-gui/ui`: extract session-list state out of `ChatSurface.tsx` into a shared hook so both `Sidebar.tsx` and the Chat surface read the same state, build `SessionSidebarSection.tsx` (with rename, real archive/unarchive, and repo grouping), wire it under the "Chat" nav item, and retire `ChatSessionRail.tsx`/`SessionsPanel` including their dockview docking dependencies.

**Tech Stack:** Rust (turso async SQLite driver, Tauri commands), React + TypeScript (Vite), Vitest/React Testing Library for frontend tests, `cargo test` for backend tests.

**Spec:** [docs/superpowers/specs/2026-08-02-sidebar-session-list-design.md](../specs/2026-08-02-sidebar-session-list-design.md)

---

## Task 1: `conversations.archived_at` column — existing-database-safe migration

**Files:**
- Modify: `crates/vox-db/src/schema/domains/conversations.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs:23`
- Modify: `crates/vox-db/src/store/open.rs` (the `migrate()` function)
- Modify: `contracts/db/baseline-version-policy.yaml`
- Test: `crates/vox-db/src/local_tests.rs`

**Why this shape:** `VoxDb::migrate()` ([`open.rs:93-148`](../../../crates/vox-db/src/store/open.rs))
only runs `baseline_sql()` — a batch of `CREATE TABLE IF NOT EXISTS` statements — and `CREATE TABLE
IF NOT EXISTS` is a no-op against a table that already exists. Adding `archived_at` to the DDL string
alone does nothing for anyone with a pre-existing database; it only takes effect on a brand-new one.
The fix adds an explicit, idempotent `ALTER TABLE` step, gated on whether the column already exists
(checked via `PRAGMA table_info`), run unconditionally on every `migrate()` call regardless of
version — cheap (one extra query) and safe to run every startup.

- [ ] **Step 1: Add the column to the baseline DDL (for fresh databases)**

In `crates/vox-db/src/schema/domains/conversations.rs`, change the `conversations` table definition:

```rust
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
    title TEXT NOT NULL DEFAULT '',
    code_version TEXT,
    repository_id TEXT,
    external_session_id TEXT,
    thread_id TEXT,
    origin_surface TEXT,
    archived_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Do **not** add the `idx_conversations_archived` index in this same DDL fragment yet — see Step 3,
which creates it only after the column is guaranteed to exist on every database, fresh or upgraded.

- [ ] **Step 2: Bump `BASELINE_VERSION`**

In `crates/vox-db/src/schema/manifest.rs:23`, change:

```rust
pub const BASELINE_VERSION: i64 = 87;
```

- [ ] **Step 3: Add an existing-database-safe `ALTER TABLE` step to `migrate()`**

In `crates/vox-db/src/store/open.rs`, find the `migrate()` function (around line 93). After the
`conn.execute_batch(sql).await?;` call inside the `if current_version < BASELINE_VERSION` block
(around line 129) and before the `apply_schema_extensions` call, add:

```rust
            // `baseline_sql()`'s CREATE TABLE IF NOT EXISTS is a no-op against a table that
            // already exists, so a column added to the DDL string above only takes effect on a
            // brand-new database. Existing databases need an explicit, idempotent ALTER TABLE —
            // checked via PRAGMA table_info first, since blindly running ALTER TABLE ADD COLUMN
            // would fail with "duplicate column name" on a fresh database (where CREATE TABLE
            // just created the column already).
            let has_archived_at = {
                let mut cols = conn.query("PRAGMA table_info(conversations)", ()).await?;
                let mut found = false;
                while let Some(row) = cols.next().await? {
                    let name: String = row.get(1)?;
                    if name == "archived_at" {
                        found = true;
                        break;
                    }
                }
                found
            };
            if !has_archived_at {
                conn.execute_batch("ALTER TABLE conversations ADD COLUMN archived_at TEXT;")
                    .await?;
            }
            conn.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_conversations_archived ON conversations(archived_at);",
            )
            .await?;
```

This runs every time `current_version < BASELINE_VERSION` (i.e. on the version bump that ships this
change, for every existing database) and is a no-op on repeat runs once the column exists — the
`has_archived_at` check itself makes the `ALTER TABLE` idempotent, and `CREATE INDEX IF NOT EXISTS`
is idempotent by construction. On a **fresh** database, `has_archived_at` is `true` immediately
(the `CREATE TABLE IF NOT EXISTS` from Step 1 already created the column), so the `ALTER TABLE`
branch is skipped there and only the index gets created — no "duplicate column" error.

- [ ] **Step 4: Recompute the baseline digest and update the policy file**

Run:

```bash
vox ci check-codex-ssot
```

This fails with the expected digest for the new baseline (the digest is computed over
`baseline_sql()`, i.e. Step 1's DDL — the `ALTER TABLE` in `migrate()` is Rust control flow, not
part of the hashed baseline string, so it does not itself change the digest beyond what Step 1
already changed). Read the "expected" digest from the failure output, then update
`contracts/db/baseline-version-policy.yaml`:

```yaml
  repository_baseline_integer: 87
  # re-updated for 87: conversations.archived_at (soft-archive for GUI chat sessions,
  # sidebar session-list feature). Existing databases get the column via an explicit
  # ALTER TABLE in VoxDb::migrate() (see open.rs), not via the baseline DDL alone.
  repository_baseline_digest_hex: "0x<PASTE THE DIGEST PRINTED BY THE CHECK ABOVE>"
```

- [ ] **Step 5: Verify the SSOT check passes**

Run: `vox ci check-codex-ssot`
Expected: passes with no digest mismatch.

- [ ] **Step 6: Write a fresh-database migration test**

Add to `crates/vox-db/src/local_tests.rs` (find the existing `#[cfg(test)] mod` block and add a new
`#[tokio::test]` alongside the others):

```rust
#[tokio::test]
async fn conversations_archived_at_column_exists_and_defaults_null() {
    let db = crate::VoxDb::connect_memory().await.expect("memory db");
    let conv_id = db
        .chat_ensure_gui_session("sess-archived-at-test", "Test session")
        .await
        .expect("create session");
    let mut rows = db
        .connection()
        .query(
            "SELECT archived_at FROM conversations WHERE id = ?1",
            turso::params![conv_id],
        )
        .await
        .expect("query");
    let row = rows.next().await.expect("row").expect("one row");
    let archived_at: Option<String> = row.get(0).expect("archived_at column");
    assert_eq!(archived_at, None, "new sessions must not be pre-archived");
}
```

- [ ] **Step 7: Write an existing-database migration test — this is the test that actually catches the bug Step 3 fixes**

A test against `connect_memory()` alone starts at `schema_version = 0` and always takes the
fresh-database path, so it cannot exercise the "table already exists, needs ALTER TABLE" branch.
Add a second test that manually recreates the pre-this-change table shape, then runs `migrate()`
again and asserts the column was actually added:

```rust
#[tokio::test]
async fn migrate_adds_archived_at_to_a_pre_existing_conversations_table() {
    let db = crate::VoxDb::connect_memory().await.expect("memory db");
    let conn = db.connection();

    // Simulate a pre-this-change table: drop the column-having table and recreate the
    // OLD shape (no archived_at), matching what a real upgrading user's database looks like.
    conn.execute_batch("DROP TABLE conversations;").await.unwrap();
    conn.execute_batch(
        "CREATE TABLE conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id TEXT,
            title TEXT NOT NULL DEFAULT '',
            code_version TEXT,
            repository_id TEXT,
            external_session_id TEXT,
            thread_id TEXT,
            origin_surface TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
    .await
    .unwrap();

    // Re-run the same migration path a real app startup takes.
    crate::store::VoxDb::migrate(conn).await.expect("migrate should backfill archived_at");

    let mut cols = conn.query("PRAGMA table_info(conversations)", ()).await.unwrap();
    let mut found = false;
    while let Some(row) = cols.next().await.unwrap() {
        let name: String = row.get(1).unwrap();
        if name == "archived_at" {
            found = true;
        }
    }
    assert!(found, "migrate() must add archived_at to a pre-existing conversations table");
}
```

(If `VoxDb::migrate` is not `pub(crate)`-visible from `local_tests.rs`'s module path, adjust the
call path to however this file already reaches other `pub(crate)` `VoxDb` internals — check an
existing test in the same file for the pattern rather than widening `migrate`'s visibility.)

- [ ] **Step 8: Run both tests**

Run: `cargo test -p vox-db conversations_archived_at_column_exists_and_defaults_null migrate_adds_archived_at_to_a_pre_existing_conversations_table`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/vox-db/src/schema/domains/conversations.rs crates/vox-db/src/schema/manifest.rs crates/vox-db/src/store/open.rs contracts/db/baseline-version-policy.yaml crates/vox-db/src/local_tests.rs
git commit -m "feat(vox-db): add conversations.archived_at with an existing-database-safe migration (BASELINE_VERSION 87)"
```

---

## Task 2: Real soft-archive/unarchive in `vox-db`

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs:679-693` (existing `chat_archive_conversation`)
- Modify: `crates/vox-db/src/codex_chat.rs:566-585` (`chat_find_gui_conversation_id`)
- Test: same file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn archive_conversation_is_recoverable_not_deleted() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    let conv_id = db.chat_ensure_gui_session("sess-1", "Session 1").await.unwrap();

    db.chat_archive_conversation(conv_id).await.unwrap();

    // Row must still exist.
    let mut rows = db
        .connection()
        .query(
            "SELECT archived_at FROM conversations WHERE id = ?1",
            turso::params![conv_id],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().expect("row survives archive");
    let archived_at: Option<String> = row.get(0).unwrap();
    assert!(archived_at.is_some(), "archived_at must be set");

    // Excluded from the default (non-archived) listing.
    let active = db.chat_list_gui_sessions(40, false).await.unwrap();
    assert!(!active.iter().any(|s| s.0 == conv_id));

    // Included when include_archived=true.
    let all = db.chat_list_gui_sessions(40, true).await.unwrap();
    assert!(all.iter().any(|s| s.0 == conv_id));

    db.chat_unarchive_conversation(conv_id).await.unwrap();
    let active_again = db.chat_list_gui_sessions(40, false).await.unwrap();
    assert!(active_again.iter().any(|s| s.0 == conv_id));
}

#[tokio::test]
async fn archived_session_is_not_found_by_external_session_id_lookup() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    let conv_id = db.chat_ensure_gui_session("sess-resume-1", "Session 1").await.unwrap();
    db.chat_archive_conversation(conv_id).await.unwrap();

    // A resumed/deep-linked external_session_id must not find the archived row...
    let found = db.chat_find_gui_conversation_id("sess-resume-1").await.unwrap();
    assert_eq!(found, None, "archived conversations must not be resurrected by find-or-create lookups");

    // ...so calling chat_ensure_gui_session again creates a fresh row instead of reusing the archived one.
    let new_conv_id = db.chat_ensure_gui_session("sess-resume-1", "Session 1").await.unwrap();
    assert_ne!(new_conv_id, conv_id, "must create a new conversation, not resurrect the archived one");
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-db archive_conversation_is_recoverable_not_deleted archived_session_is_not_found_by_external_session_id_lookup`
Expected: compile error (`chat_unarchive_conversation` doesn't exist yet, `chat_list_gui_sessions` takes 1 arg not 2).

- [ ] **Step 3: Replace `chat_archive_conversation` and add `chat_unarchive_conversation`**

In `crates/vox-db/src/codex_chat.rs`, replace the existing `chat_archive_conversation` (lines 679-693):

```rust
    /// Soft-archive a conversation (recoverable — see `chat_unarchive_conversation`).
    pub async fn chat_archive_conversation(&self, conversation_id: i64) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET archived_at = datetime('now') WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Reverse `chat_archive_conversation`.
    pub async fn chat_unarchive_conversation(&self, conversation_id: i64) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE conversations SET archived_at = NULL WHERE id = ?1",
                    params![conversation_id],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
```

- [ ] **Step 4: Make `chat_find_gui_conversation_id` skip archived rows**

Replace the existing method (lines 566-585) with:

```rust
    /// Locate a GUI chat session by its external session id (`origin_surface = gui`).
    /// Archived conversations are excluded — a resumed/deep-linked `external_session_id`
    /// pointing at an archived row must not silently reuse it (see
    /// `archived_session_is_not_found_by_external_session_id_lookup`).
    pub async fn chat_find_gui_conversation_id(
        &self,
        external_session_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        let sid = external_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM conversations
                 WHERE origin_surface = 'gui' AND external_session_id = ?1 AND archived_at IS NULL
                 LIMIT 1",
                params![sid.as_str()],
            )
            .await?;
        let row = rows.next().await?;
        Ok(match row {
            Some(r) => Some(r.get(0).map_err(|e| StoreError::Db(e.to_string()))?),
            None => None,
        })
    }
```

- [ ] **Step 5: Update `chat_list_gui_sessions` to filter by archive state**

Replace the existing method (lines 628-656) with:

```rust
    /// List recent GUI chat sessions for the sidebar/tab strip.
    ///
    /// Excludes `bg-task-*` session ids (see prior comment — unchanged). When
    /// `include_archived` is false, also excludes rows with `archived_at IS NOT NULL`.
    pub async fn chat_list_gui_sessions(
        &self,
        limit: usize,
        include_archived: bool,
    ) -> Result<Vec<(i64, String, String, String, i64, Option<String>)>, StoreError> {
        let lim = limit.max(1) as i64;
        let archive_clause = if include_archived { "" } else { "AND c.archived_at IS NULL" };
        let sql = format!(
            "SELECT c.id, c.title, c.external_session_id, c.updated_at,
                    (SELECT COUNT(*) FROM conversation_messages m WHERE m.conversation_id = c.id),
                    c.repository_id
             FROM conversations c
             WHERE c.origin_surface = 'gui'
               AND c.external_session_id NOT LIKE 'bg-task-%'
               {archive_clause}
             ORDER BY c.updated_at DESC
             LIMIT ?1"
        );
        let mut rows = self.connection().query(&sql, params![lim]).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            let id: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let title: String = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            let ext: String = row.get(2).map_err(|e| StoreError::Db(e.to_string()))?;
            let updated: String = row.get(3).map_err(|e| StoreError::Db(e.to_string()))?;
            let count: i64 = row.get(4).map_err(|e| StoreError::Db(e.to_string()))?;
            let repository_id: Option<String> = row.get(5).map_err(|e| StoreError::Db(e.to_string()))?;
            out.push((id, title, ext, updated, count, repository_id));
        }
        Ok(out)
    }
```

Note the tuple shape changed (added `repository_id`, `include_archived` param) — this breaks the one existing caller, fixed in Task 5.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p vox-db archive_conversation_is_recoverable_not_deleted archived_session_is_not_found_by_external_session_id_lookup`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-db): soft-archive conversations instead of hard delete; exclude archived rows from find-or-create"
```

---

## Task 3: Populate `repository_id` on session create

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs:587-614` (`chat_ensure_gui_session`)
- Test: same file

**Known limitation, accepted for v1 (see spec §3):** the repository is resolved from the Tauri
process's current working directory, which is process-global, not per-window. A GUI process with
more than one repository/workspace open will tag every session created from it with whichever repo
the process's CWD was at launch. No fix is planned here — flagged in the spec so it's a documented
tradeoff, not a surprise.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn ensure_gui_session_persists_repository_id() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    let conv_id = db
        .chat_ensure_gui_session_with_repo("sess-repo-1", "Session 1", Some("abc123"))
        .await
        .unwrap();
    let mut rows = db
        .connection()
        .query(
            "SELECT repository_id FROM conversations WHERE id = ?1",
            turso::params![conv_id],
        )
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let repo: Option<String> = row.get(0).unwrap();
    assert_eq!(repo.as_deref(), Some("abc123"));
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-db ensure_gui_session_persists_repository_id`
Expected: compile error, `chat_ensure_gui_session_with_repo` not found.

- [ ] **Step 3: Add the repo-aware variant, keep the old one delegating**

In `crates/vox-db/src/codex_chat.rs`, replace `chat_ensure_gui_session` (lines 587-614):

```rust
    /// Ensure a GUI-scoped conversation row exists; returns SQLite id.
    pub async fn chat_ensure_gui_session(
        &self,
        external_session_id: &str,
        title: &str,
    ) -> Result<i64, StoreError> {
        self.chat_ensure_gui_session_with_repo(external_session_id, title, None)
            .await
    }

    /// Same as [`Self::chat_ensure_gui_session`], additionally recording which
    /// repository this session targets (see `vox_repository::RepositoryContext::repository_id`
    /// for how callers derive `repository_id`). If a conversation with this
    /// `external_session_id` already exists, `repository_id` is ignored on this call — the
    /// existing row's value is left as-is (find-or-create semantics; this method never updates
    /// an existing row's repository tag).
    pub async fn chat_ensure_gui_session_with_repo(
        &self,
        external_session_id: &str,
        title: &str,
        repository_id: Option<&str>,
    ) -> Result<i64, StoreError> {
        if let Some(id) = self
            .chat_find_gui_conversation_id(external_session_id)
            .await?
        {
            return Ok(id);
        }
        let sid = external_session_id.to_string();
        let title = title.to_string();
        let repo = repository_id.map(str::to_string);
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO conversations (title, external_session_id, origin_surface, repository_id)
                     VALUES (?1, ?2, 'gui', ?3)",
                    params![title.as_str(), sid.as_str(), repo.as_deref()],
                )
                .await?;
                Ok::<i64, StoreError>(conn.last_insert_rowid())
            })
            .await
    }
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p vox-db ensure_gui_session_persists_repository_id`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-db): allow tagging GUI chat sessions with a repository_id"
```

---

## Task 4: Batched open-task count via the real `plan_sessions.origin_session_id` link

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs` (new function, near `chat_ensure_gui_session_with_repo`)
- Test: same file

**Corrected by adversarial review — read before implementing.** An earlier version of this task
invented a `create_paired_plan_session` call that minted a `plan_session_id = "chatplan-{chat_id}"`
row at chat-session-creation time. That id space is never written to by real dispatch code: real
task dispatch ([`goal.rs:533,648`](../../../crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/goal.rs))
mints `plan_session_id = format!("plan-{uuid}")` per dispatched goal and links back via the
**existing** `plan_sessions.origin_session_id` column, set to the chat session's `external_session_id`.
A chat session can have zero, one, or several `plan_sessions` rows (one per dispatched goal) — never
exactly one paired row. This task does **not** create any `plan_sessions` row; it only reads the ones
real dispatch already creates.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn open_task_counts_join_on_origin_session_id_and_current_version() {
    let db = crate::VoxDb::connect_memory().await.unwrap();

    // Session "chat-a" has dispatched two goals -> two plan_sessions rows.
    db.create_plan_session("plan-a1", Some("chat-a"), "goal one", "sequential").await.unwrap();
    db.append_plan_version("plan-a1", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("plan-a1", 1, "n1", "step", "[]", "{}", "pending", None).await.unwrap();

    db.create_plan_session("plan-a2", Some("chat-a"), "goal two", "sequential").await.unwrap();
    db.append_plan_version("plan-a2", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("plan-a2", 1, "n1", "step", "[]", "{}", "in_progress", None).await.unwrap();

    // Session "chat-b" has one dispatched goal, already completed.
    db.create_plan_session("plan-b1", Some("chat-b"), "goal three", "sequential").await.unwrap();
    db.append_plan_version("plan-b1", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("plan-b1", 1, "n1", "step", "[]", "{}", "completed", None).await.unwrap();

    // Session "chat-c" has never dispatched anything.
    let counts = db
        .open_task_counts_for_sessions(&["chat-a".to_string(), "chat-b".to_string(), "chat-c".to_string()])
        .await
        .unwrap();

    assert_eq!(counts.get("chat-a").copied(), Some(2), "sums across both of chat-a's plan_sessions rows");
    assert_eq!(counts.get("chat-b"), None, "zero-count sessions are absent from the map, not present with 0");
    assert_eq!(counts.get("chat-c"), None);
}

#[tokio::test]
async fn open_task_counts_exclude_superseded_plan_versions() {
    let db = crate::VoxDb::connect_memory().await.unwrap();

    db.create_plan_session("plan-v1", Some("chat-v"), "goal", "sequential").await.unwrap();
    db.append_plan_version("plan-v1", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("plan-v1", 1, "n1", "step", "[]", "{}", "pending", None).await.unwrap();

    // Bump to version 2 — current_version moves to 2, so version 1's pending node must no
    // longer count (it belongs to a superseded version).
    db.append_plan_version("plan-v1", 2, Some(1), None, None).await.unwrap();
    db.upsert_plan_node("plan-v1", 2, "n1", "step", "[]", "{}", "completed", None).await.unwrap();

    let counts = db.open_task_counts_for_sessions(&["chat-v".to_string()]).await.unwrap();
    assert_eq!(counts.get("chat-v"), None, "version 1's pending node must not count once version 2 is current");
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-db open_task_counts_join_on_origin_session_id_and_current_version open_task_counts_exclude_superseded_plan_versions`
Expected: compile error, `open_task_counts_for_sessions` not found.

- [ ] **Step 3: Implement it**

Add to `crates/vox-db/src/codex_chat.rs` (anywhere inside `impl crate::VoxDb { ... }`):

```rust
    /// Count open (pending/queued/in_progress) plan nodes at each plan session's *current*
    /// version, summed per originating chat session, across every `plan_sessions` row that
    /// chat session has ever produced (one per dispatched goal — see `goal.rs`). Sessions with
    /// no dispatched goals, or whose nodes are all resolved, are absent from the returned map
    /// (not present with a `0` entry) — callers should treat a missing key as zero.
    pub async fn open_task_counts_for_sessions(
        &self,
        chat_external_session_ids: &[String],
    ) -> Result<std::collections::HashMap<String, i64>, StoreError> {
        let mut out = std::collections::HashMap::new();
        if chat_external_session_ids.is_empty() {
            return Ok(out);
        }
        let placeholders = (1..=chat_external_session_ids.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT ps.origin_session_id, COUNT(*)
             FROM plan_sessions ps
             JOIN plan_nodes pn
               ON pn.plan_session_id = ps.plan_session_id AND pn.version = ps.current_version
             WHERE ps.origin_session_id IN ({placeholders})
               AND pn.status IN ('pending', 'queued', 'in_progress')
             GROUP BY ps.origin_session_id"
        );
        let bound: Vec<turso::Value> = chat_external_session_ids
            .iter()
            .map(|id| turso::Value::from(id.as_str()))
            .collect();
        let mut rows = self.connection().query(&sql, bound).await?;
        while let Some(row) = rows.next().await? {
            let session_id: String = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let count: i64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            out.insert(session_id, count);
        }
        Ok(out)
    }
```

(`turso::Value::from(&str)` and the `?1,?2,...` dynamic-placeholder pattern already have precedent
in this crate — see `crates/vox-db/src/store/ops_a2a.rs`'s `NOT IN (...)` query — follow that same
shape if the exact `Value` conversion above doesn't compile as written.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-db open_task_counts_join_on_origin_session_id_and_current_version open_task_counts_exclude_superseded_plan_versions`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-db): batched open-task counts per chat session via plan_sessions.origin_session_id"
```

---

## Task 5: Wire repository tagging and archive/unarchive into the `vox-gui` Tauri command layer

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs:16-92` (`ChatSessionDto`, `chat_create_session`, `chat_list_sessions`), `:291-` (`chat_archive_session`)
- Test: `crates/vox-gui/src/commands/chat.rs` `#[cfg(test)]` module

**Note:** this task no longer creates any `plan_sessions` row and `ChatSessionDto` does not carry a
`plan_session_id` field — the task-count badge (Task 6/10) looks sessions up by their own
`session_id` against the batched query from Task 4, not by a precomputed plan-session id.

- [ ] **Step 1: Write the failing test**

Add near the existing tests in `chat.rs` (search for `#[cfg(test)]`):

```rust
#[tokio::test]
async fn chat_create_session_sets_repository_id() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();

    let dto = chat_create_session(pool, Some("Test".into())).await.unwrap();

    assert!(dto.repository_id.is_some(), "repository_id should resolve from cwd");
}

#[tokio::test]
async fn chat_archive_and_unarchive_session_round_trip() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();

    let dto = chat_create_session(pool.clone(), Some("Test".into())).await.unwrap();
    chat_archive_session(pool.clone(), dto.session_id.clone()).await.unwrap();

    let active = chat_list_sessions(pool.clone(), None, None).await.unwrap();
    assert!(!active.iter().any(|s| s.session_id == dto.session_id));

    let all = chat_list_sessions(pool.clone(), None, Some(true)).await.unwrap();
    assert!(all.iter().any(|s| s.session_id == dto.session_id));

    chat_unarchive_session(pool.clone(), dto.session_id.clone()).await.unwrap();
    let active_again = chat_list_sessions(pool, None, None).await.unwrap();
    assert!(active_again.iter().any(|s| s.session_id == dto.session_id));
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-gui chat_create_session_sets_repository_id`
Expected: compile error — `ChatSessionDto` has no `repository_id` field yet, `chat_unarchive_session` doesn't exist, `chat_list_sessions` signature mismatch.

- [ ] **Step 3: Extend `ChatSessionDto`**

In `crates/vox-gui/src/commands/chat.rs`, replace lines 16-23:

```rust
#[derive(Debug, Serialize)]
pub struct ChatSessionDto {
    pub session_id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: i64,
    pub conversation_id: i64,
    pub repository_id: Option<String>,
}
```

- [ ] **Step 4: Resolve the current repo in `chat_create_session`**

Replace `chat_create_session` (lines 51-70):

```rust
#[tauri::command]
pub async fn chat_create_session(
    pool: State<'_, GuiDbPool>,
    title: Option<String>,
) -> Result<ChatSessionDto, String> {
    let db = pool_db(&pool)?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let title = title.unwrap_or_else(|| "New chat".to_string());

    let cwd = std::env::current_dir().unwrap_or_default();
    let repo_ctx = vox_repository::discover_repository_or_fallback(&cwd);
    let repository_id = Some(repo_ctx.repository_id);

    let conv_id = db
        .chat_ensure_gui_session_with_repo(&session_id, &title, repository_id.as_deref())
        .await
        .map_err(map_db_err)?;

    Ok(ChatSessionDto {
        session_id,
        title,
        updated_at: String::new(),
        message_count: 0,
        conversation_id: conv_id,
        repository_id,
    })
}
```

- [ ] **Step 5: Update `chat_list_sessions` for the new DB signature + archived filter**

Replace `chat_list_sessions` (lines 72-92):

```rust
#[tauri::command]
pub async fn chat_list_sessions(
    pool: State<'_, GuiDbPool>,
    limit: Option<usize>,
    include_archived: Option<bool>,
) -> Result<Vec<ChatSessionDto>, String> {
    let db = pool_db(&pool)?;
    let lim = limit.unwrap_or(40);
    let rows = db
        .chat_list_gui_sessions(lim, include_archived.unwrap_or(false))
        .await
        .map_err(map_db_err)?;
    Ok(rows
        .into_iter()
        .map(
            |(conversation_id, title, session_id, updated_at, message_count, repository_id)| {
                ChatSessionDto {
                    session_id,
                    title,
                    updated_at,
                    message_count,
                    conversation_id,
                    repository_id,
                }
            },
        )
        .collect())
}
```

- [ ] **Step 6: Add `chat_unarchive_session`, next to the existing `chat_archive_session`**

Find `chat_archive_session` around line 308 and add immediately after it:

```rust
#[tauri::command]
pub async fn chat_unarchive_session(
    pool: State<'_, GuiDbPool>,
    session_id: String,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_find_gui_conversation_id(&session_id)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| format!("session {session_id} not found"))?;
    db.chat_unarchive_conversation(conv_id).await.map_err(map_db_err)
}
```

Note: `chat_find_gui_conversation_id` (Task 2 Step 4) now excludes archived rows by design — so this
specific lookup would fail to find an archived session by that path. `chat_unarchive_session` needs
the *archived* row, not an active one. Add a small archived-aware lookup instead of reusing
`chat_find_gui_conversation_id` here:

```rust
#[tauri::command]
pub async fn chat_unarchive_session(
    pool: State<'_, GuiDbPool>,
    session_id: String,
) -> Result<(), String> {
    let db = pool_db(&pool)?;
    let conv_id = db
        .chat_find_gui_conversation_id_including_archived(&session_id)
        .await
        .map_err(map_db_err)?
        .ok_or_else(|| format!("session {session_id} not found"))?;
    db.chat_unarchive_conversation(conv_id).await.map_err(map_db_err)
}
```

Add the corresponding `chat_find_gui_conversation_id_including_archived` to
`crates/vox-db/src/codex_chat.rs` right after `chat_find_gui_conversation_id` (Task 2 Step 4) —
same query, without the `AND archived_at IS NULL` clause:

```rust
    /// Same as [`Self::chat_find_gui_conversation_id`] but also finds archived rows — used only
    /// by the unarchive path, which needs to locate a conversation precisely because it's
    /// archived.
    pub async fn chat_find_gui_conversation_id_including_archived(
        &self,
        external_session_id: &str,
    ) -> Result<Option<i64>, StoreError> {
        let sid = external_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT id FROM conversations
                 WHERE origin_surface = 'gui' AND external_session_id = ?1
                 LIMIT 1",
                params![sid.as_str()],
            )
            .await?;
        let row = rows.next().await?;
        Ok(match row {
            Some(r) => Some(r.get(0).map_err(|e| StoreError::Db(e.to_string()))?),
            None => None,
        })
    }
```

Add a test for this alongside Task 2's tests in `codex_chat.rs`:

```rust
#[tokio::test]
async fn unarchive_finds_an_archived_conversation_by_id() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    let conv_id = db.chat_ensure_gui_session("sess-unarchive-1", "S").await.unwrap();
    db.chat_archive_conversation(conv_id).await.unwrap();

    let found = db.chat_find_gui_conversation_id_including_archived("sess-unarchive-1").await.unwrap();
    assert_eq!(found, Some(conv_id));
}
```

- [ ] **Step 7: Register the new command in the Tauri invoke handler**

Find where `chat_archive_session` is registered (grep `generate_handler!` or the invoke-handler list in `crates/vox-gui/src/main.rs` or `crates/vox-gui/src/lib.rs` for `chat_archive_session`) and add `chat_unarchive_session` to the same list.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p vox-gui chat_create_session_sets_repository_id chat_archive_and_unarchive_session_round_trip`
Run: `cargo test -p vox-db unarchive_finds_an_archived_conversation_by_id`
Expected: PASS

- [ ] **Step 9: Fix any other callers broken by the `ChatSessionDto`/`chat_list_sessions` signature changes, and add a same-file test for each one modified**

Run: `cargo check -p vox-gui`
Fix any remaining call sites (e.g. `secretary_confirm_task` or other code constructing `ChatSessionDto` or calling `chat_list_sessions`/`chat_ensure_gui_session`) to match the new fields/signature. Per this repo's Test-First Policy (AGENTS.md), any modified `pub fn` whose behavior changes needs an adjacent same-file test — not just a compile check — so add one for each call site actually touched here, not only the two above.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/src/commands/chat.rs crates/vox-gui/src/main.rs crates/vox-gui/src/lib.rs crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-gui): expose repository_id and archive/unarchive on chat sessions"
```

---

## Task 6: `plan_open_task_counts` Tauri command (batched)

**Files:**
- Modify: `crates/vox-gui/src/commands/plan_panel.rs`
- Test: same file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn plan_open_task_counts_batches_across_sessions() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();
    let db = pool.handle().unwrap();

    db.create_plan_session("ps-count-1", Some("chat-x"), "goal", "sequential").await.unwrap();
    db.append_plan_version("ps-count-1", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("ps-count-1", 1, "n1", "step", "[]", "{}", "pending", None).await.unwrap();

    let counts = plan_open_task_counts(pool, vec!["chat-x".to_string(), "chat-y".to_string()])
        .await
        .unwrap();
    assert_eq!(counts.get("chat-x").copied(), Some(1));
    assert_eq!(counts.get("chat-y"), None);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-gui plan_open_task_counts_batches_across_sessions`
Expected: compile error, function not found.

- [ ] **Step 3: Add the command**

Add to `crates/vox-gui/src/commands/plan_panel.rs`, near `list_plan_nodes`:

```rust
/// Batched open-task counts for the sidebar's task-count badges, one round trip for every
/// visible session instead of one `invoke` per session. Keyed by chat session id (matches
/// `ChatSessionDto::session_id`); a session absent from the returned map has zero open tasks.
#[tauri::command]
pub async fn plan_open_task_counts(
    pool: State<'_, GuiDbPool>,
    session_ids: Vec<String>,
) -> Result<std::collections::HashMap<String, i64>, String> {
    let db = pool_db(&pool)?;
    db.open_task_counts_for_sessions(&session_ids).await.map_err(map_db_err)
}
```

- [ ] **Step 4: Register the command in the invoke handler** (same location as Task 5 Step 7)

- [ ] **Step 5: Run the test**

Run: `cargo test -p vox-gui plan_open_task_counts_batches_across_sessions`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/plan_panel.rs crates/vox-gui/src/main.rs crates/vox-gui/src/lib.rs
git commit -m "feat(vox-gui): add batched plan_open_task_counts command for the sidebar task badges"
```

---

## Task 7: Extract shared session-list state into `useChatSessions`

**Files:**
- Create: `crates/vox-gui/ui/src/lib/useChatSessions.ts`
- Test: `crates/vox-gui/ui/src/lib/useChatSessions.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (remove local session CRUD and the duplicate local `ChatSession` interface; consume the hook's type instead)

**Ownership (resolved — an earlier draft of this plan left this as an open question):** after Task 9
deletes `ChatSessionRail`/`SessionsPanel`, `ChatSurface.tsx` no longer needs session CRUD locally at
all — it only needs to know the currently active session id (already threaded in via
`activeId`/`onSessionChange` props from `App.tsx`) to load that session's messages. `useChatSessions`
is therefore owned by `App.tsx` alone; `ChatSurface.tsx` does not call it.

**Why extract at all:** even with one real call site, the CRUD logic (load/create/rename/archive,
with its specific error-toast and active-session-reassignment behavior) needs to exist somewhere
outside `ChatSurface.tsx` once `Sidebar.tsx` needs to trigger the same actions — moving it to
`App.tsx` as plain local state (not a hook) was considered and rejected only because `App.tsx` is
already a very large component; a small, independently testable module is preferable there, not
because there are two consumers.

- [ ] **Step 1: Locate the exact current implementation**

Run: `grep -n "loadSessions\|createSession\|renameSession\|archiveSession" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

Read the matched region fully before proceeding. In particular, `archiveSession`
(`ChatSurface.tsx:680-690`) reassigns the active session to the next remaining one when the
archived session was active — the hook below must reproduce this, not just filter the list.

- [ ] **Step 2: Write the failing test first**

```typescript
// crates/vox-gui/ui/src/lib/useChatSessions.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useChatSessions } from './useChatSessions';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

function session(overrides: Partial<import('./useChatSessions').ChatSession>) {
  return {
    session_id: 's1', title: 'Untitled', updated_at: '', message_count: 0,
    conversation_id: 1, repository_id: 'repo-a', ...overrides,
  };
}

describe('useChatSessions', () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it('loads sessions on mount', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));
    expect(invoke).toHaveBeenCalledWith('chat_list_sessions', expect.anything());
  });

  it('creates a session and prepends it to the list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]); // initial load
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(0));

    vi.mocked(invoke).mockResolvedValueOnce(session({ session_id: 's2', title: 'New chat' }));
    await act(async () => { await result.current.createSession(); });

    expect(result.current.sessions[0].session_id).toBe('s2');
    expect(invoke).toHaveBeenCalledWith('chat_create_session', expect.anything());
  });

  it('archiving removes the session from the default (non-archived) list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    vi.mocked(invoke).mockResolvedValueOnce(undefined); // chat_archive_session
    await act(async () => { await result.current.archiveSession('s1'); });

    expect(result.current.sessions).toHaveLength(0);
    expect(invoke).toHaveBeenCalledWith('chat_archive_session', { sessionId: 's1' });
  });

  it('archiving the active session reassigns activeSessionId to the next remaining one', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' }), session({ session_id: 's2' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(2));

    vi.mocked(invoke).mockResolvedValueOnce(undefined); // chat_archive_session
    const onActiveSessionArchived = vi.fn();
    await act(async () => { await result.current.archiveSession('s1', { wasActive: true, onReassign: onActiveSessionArchived }); });

    expect(onActiveSessionArchived).toHaveBeenCalledWith('s2');
  });

  it('surfaces a create failure via the returned error rather than throwing unhandled', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]); // initial load
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(0));

    vi.mocked(invoke).mockRejectedValueOnce(new Error('backend unreachable'));
    await expect(result.current.createSession()).rejects.toThrow('backend unreachable');
    expect(result.current.sessions).toHaveLength(0); // no optimistic entry left behind
  });
});
```

- [ ] **Step 3: Run it to confirm it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/useChatSessions.test.ts`
Expected: FAIL — module `./useChatSessions` doesn't exist.

- [ ] **Step 4: Implement the hook**

```typescript
// crates/vox-gui/ui/src/lib/useChatSessions.ts
import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface ChatSession {
  session_id: string;
  title: string;
  updated_at: string;
  message_count: number;
  conversation_id: number;
  repository_id: string | null;
}

interface ArchiveOptions {
  wasActive?: boolean;
  onReassign?: (nextActiveSessionId: string) => void;
}

export function useChatSessions() {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [includeArchived, setIncludeArchived] = useState(false);

  const load = useCallback(async (opts?: { includeArchived?: boolean }) => {
    const list = await invoke<ChatSession[]>('chat_list_sessions', {
      limit: 200,
      includeArchived: opts?.includeArchived ?? includeArchived,
    });
    setSessions(list);
  }, [includeArchived]);

  useEffect(() => { load(); }, [load]);

  // No optimistic update: on failure, invoke() rejects and this function rethrows without
  // touching `sessions` state, matching ChatSurface.tsx's existing await-then-update-on-success
  // pattern (there is no rollback anywhere in the current codebase to replicate).
  const createSession = useCallback(async (title?: string) => {
    const created = await invoke<ChatSession>('chat_create_session', { title });
    setSessions(prev => [created, ...prev]);
    return created;
  }, []);

  const renameSession = useCallback(async (sessionId: string, title: string) => {
    await invoke('chat_rename_session', { sessionId, title });
    setSessions(prev => prev.map(s => s.session_id === sessionId ? { ...s, title } : s));
  }, []);

  const archiveSession = useCallback(async (sessionId: string, opts?: ArchiveOptions) => {
    await invoke('chat_archive_session', { sessionId });
    const remaining = sessions.filter(s => s.session_id !== sessionId);
    setSessions(remaining);
    if (opts?.wasActive && remaining.length > 0) {
      opts.onReassign?.(remaining[0].session_id);
    }
  }, [sessions]);

  const unarchiveSession = useCallback(async (sessionId: string) => {
    await invoke('chat_unarchive_session', { sessionId });
    await load();
  }, [load]);

  const toggleArchivedView = useCallback(async () => {
    const next = !includeArchived;
    setIncludeArchived(next);
    await load({ includeArchived: next });
  }, [includeArchived, load]);

  return {
    sessions,
    includeArchived,
    createSession,
    renameSession,
    archiveSession,
    unarchiveSession,
    toggleArchivedView,
    reload: load,
  };
}
```

- [ ] **Step 5: Run the test**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/useChatSessions.test.ts`
Expected: PASS (adjust the Tauri argument-casing assertions — `sessionId` vs `session_id` — to match whatever `ChatSurface.tsx`'s current calls actually use, found in Step 1; Tauri's default is camelCase command args, already confirmed against `ChatSurface.tsx:673,682`'s real `{ sessionId }`/`{ sessionId, title }` calls).

- [ ] **Step 6: Remove `ChatSurface.tsx`'s local session CRUD and duplicate `ChatSession` interface**

In `ChatSurface.tsx`:
- Delete the local `interface ChatSession { session_id: string; title: string; message_count: number; }` (currently at lines 102-106) — this duplicate, narrower type is a drift risk once the hook's richer version exists.
- Delete the local `loadSessions`/`createSession`/`renameSession`/`archiveSession` functions and their backing `useState<ChatSession[]>`.
- Import the type instead: `import type { ChatSession } from '../../../lib/useChatSessions';`
- `ChatSurface.tsx` no longer calls `useChatSessions()` itself (see the Ownership note above) — it
  receives whatever session-related props it still needs (at minimum `activeId`/`onSessionChange`,
  already present) from `App.tsx`. Do not thread `sessions`/`createSession`/etc. into `ChatSurface`
  unless something inside it still genuinely needs them after Task 9 removes `ChatSessionRail`.

- [ ] **Step 7: Run existing Chat surface tests**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat`
Expected: PASS (fix any test that mocked the old local functions directly instead of `useChatSessions`)

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/lib/useChatSessions.ts crates/vox-gui/ui/src/lib/useChatSessions.test.ts crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "refactor(vox-gui-ui): extract session CRUD into shared useChatSessions hook, owned by App.tsx"
```

---

## Task 8: `SessionSidebarSection` component

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/SessionSidebarSection.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/SessionSidebarSection.test.tsx`

**Corrected by adversarial review:** an earlier draft of this component destructured
`onArchiveSession`/`onUnarchiveSession`/rename props without ever calling them, and its "Show
archived" button only toggled a label with no data behind it. This version wires all of them for
real, and sorts each repo group by `updated_at` (a spec §5 requirement the earlier draft's `slice()`
silently ignored).

- [ ] **Step 1: Write the failing test**

```typescript
// crates/vox-gui/ui/src/components/layout/SessionSidebarSection.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionSidebarSection } from './SessionSidebarSection';
import type { ChatSession } from '../../lib/useChatSessions';

function session(overrides: Partial<ChatSession>): ChatSession {
  return {
    session_id: 's1', title: 'Untitled', updated_at: '2026-01-01T00:00:00Z', message_count: 0,
    conversation_id: 1, repository_id: 'vox', ...overrides,
  };
}

const noop = () => {};
const baseProps = {
  activeSessionId: null as string | null,
  taskCounts: {} as Record<string, number>,
  archivedSessions: [] as ChatSession[],
  showArchived: false,
  onSessionChange: noop,
  onCreateSession: noop,
  onRenameSession: noop as (id: string, title: string) => void,
  onArchiveSession: noop as (id: string) => void,
  onUnarchiveSession: noop as (id: string) => void,
  onToggleArchivedView: noop,
  onTaskBadgeClick: noop as (id: string) => void,
};

describe('SessionSidebarSection', () => {
  it('groups sessions by repository_id under separate headers', () => {
    const sessions = [session({ session_id: 'a', repository_id: 'vox' }), session({ session_id: 'b', repository_id: 'vox-server' })];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    expect(screen.getByText('vox')).toBeInTheDocument();
    expect(screen.getByText('vox-server')).toBeInTheDocument();
  });

  it('sessions without a repository_id fall under "Other"', () => {
    render(<SessionSidebarSection {...baseProps} sessions={[session({ repository_id: null })]} />);
    expect(screen.getByText('Other')).toBeInTheDocument();
  });

  it('sorts each repo group by updated_at descending before truncating', () => {
    const sessions = [
      session({ session_id: 'old', title: 'Old', repository_id: 'vox', updated_at: '2026-01-01T00:00:00Z' }),
      session({ session_id: 'new', title: 'New', repository_id: 'vox', updated_at: '2026-06-01T00:00:00Z' }),
    ];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs[0]).toHaveTextContent('New');
    expect(tabs[1]).toHaveTextContent('Old');
  });

  it('truncates each repo group independently at 5, with its own Show more', () => {
    const sessions = Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `Session ${i}`, repository_id: 'vox', updated_at: `2026-01-0${i + 1}T00:00:00Z` }));
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    expect(screen.getAllByRole('tab')).toHaveLength(5);
    fireEvent.click(screen.getByText('Show 2 more'));
    expect(screen.getAllByRole('tab')).toHaveLength(7);
  });

  it('expanding one repo group does not affect another', () => {
    const sessions = [
      ...Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `V${i}`, repository_id: 'vox', updated_at: `2026-01-0${i + 1}T00:00:00Z` })),
      session({ session_id: 'w0', title: 'W0', repository_id: 'vox-server' }),
    ];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    fireEvent.click(screen.getByText('Show 2 more'));
    expect(screen.getAllByRole('tab')).toHaveLength(8);
  });

  it('clicking + New session calls onCreateSession', () => {
    const onCreateSession = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[]} onCreateSession={onCreateSession} />);
    fireEvent.click(screen.getByText('+ New session'));
    expect(onCreateSession).toHaveBeenCalled();
  });

  it('clicking a task badge calls onTaskBadgeClick with the session, not onSessionChange', () => {
    const onSessionChange = vi.fn();
    const onTaskBadgeClick = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[session({ session_id: 'a' })]} taskCounts={{ a: 3 }} onSessionChange={onSessionChange} onTaskBadgeClick={onTaskBadgeClick} />);
    fireEvent.click(screen.getByText('3'));
    expect(onTaskBadgeClick).toHaveBeenCalledWith('a');
    expect(onSessionChange).not.toHaveBeenCalled();
  });

  it('renames a session via inline edit, matching the row not the whole list', () => {
    const onRenameSession = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[session({ session_id: 'a', title: 'Old title' })]} onRenameSession={onRenameSession} />);
    fireEvent.doubleClick(screen.getByText('Old title'));
    const input = screen.getByDisplayValue('Old title');
    fireEvent.change(input, { target: { value: 'New title' } });
    fireEvent.blur(input);
    expect(onRenameSession).toHaveBeenCalledWith('a', 'New title');
  });

  it('"Show archived" toggles onToggleArchivedView and, once open, renders archivedSessions with a working Unarchive action', () => {
    const onToggleArchivedView = vi.fn();
    const onUnarchiveSession = vi.fn();
    const archived = [session({ session_id: 'arch-1', title: 'Archived one', repository_id: 'vox' })];

    const { rerender } = render(<SessionSidebarSection {...baseProps} sessions={[]} onToggleArchivedView={onToggleArchivedView} />);
    fireEvent.click(screen.getByText('Show archived'));
    expect(onToggleArchivedView).toHaveBeenCalled();

    rerender(<SessionSidebarSection {...baseProps} sessions={[]} showArchived archivedSessions={archived} onUnarchiveSession={onUnarchiveSession} />);
    expect(screen.getByText('Archived one')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Unarchive'));
    expect(onUnarchiveSession).toHaveBeenCalledWith('arch-1');
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/SessionSidebarSection.test.tsx`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement the component**

```tsx
// crates/vox-gui/ui/src/components/layout/SessionSidebarSection.tsx
import { useState } from 'react';
import type { ChatSession } from '../../lib/useChatSessions';

const VISIBLE_PER_GROUP = 5;

interface Props {
  sessions: ChatSession[];
  activeSessionId: string | null;
  taskCounts: Record<string, number>;
  archivedSessions: ChatSession[];
  showArchived: boolean;
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onArchiveSession: (sessionId: string) => void;
  onUnarchiveSession: (sessionId: string) => void;
  onToggleArchivedView: () => void;
  onTaskBadgeClick: (sessionId: string) => void;
}

function groupByRepo(sessions: ChatSession[]): Map<string, ChatSession[]> {
  const groups = new Map<string, ChatSession[]>();
  for (const s of sessions) {
    const key = s.repository_id ?? 'Other';
    const list = groups.get(key) ?? [];
    list.push(s);
    groups.set(key, list);
  }
  for (const list of groups.values()) {
    list.sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
  }
  return groups;
}

function SessionRow({
  s, isActive, taskCount, onSessionChange, onRenameSession, onArchiveSession, onTaskBadgeClick, showArchive,
}: {
  s: ChatSession;
  isActive: boolean;
  taskCount: number;
  onSessionChange: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  onTaskBadgeClick: (id: string) => void;
  showArchive: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(s.title);

  if (editing) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={e => setDraft(e.target.value)}
        onBlur={() => { setEditing(false); if (draft.trim() && draft !== s.title) onRenameSession(s.session_id, draft.trim()); }}
        onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); if (e.key === 'Escape') { setDraft(s.title); setEditing(false); } }}
        className="w-full rounded px-2 py-1 text-[12px] bg-overlay-subtle"
      />
    );
  }

  return (
    <div role="tab" aria-selected={s.session_id === isActive}
         onClick={() => onSessionChange(s.session_id)}
         onDoubleClick={() => setEditing(true)}
         className="flex items-center justify-between rounded px-2 py-1 text-[12px] cursor-pointer hover:bg-overlay-hover group">
      <span className="truncate">{s.title}</span>
      <span className="flex items-center gap-1 shrink-0">
        {taskCount > 0 && (
          <span
            onClick={e => { e.stopPropagation(); onTaskBadgeClick(s.session_id); }}
            className="rounded-full bg-overlay-subtle px-1.5 text-[10px] text-text-muted"
          >
            {taskCount}
          </span>
        )}
        {showArchive && (
          <button
            type="button"
            onClick={e => { e.stopPropagation(); onArchiveSession(s.session_id); }}
            className="hidden group-hover:inline text-[10px] text-text-muted hover:text-text-primary"
          >
            Archive
          </button>
        )}
      </span>
    </div>
  );
}

function RepoGroup({
  repo, sessions, activeSessionId, taskCounts, onSessionChange, onRenameSession, onArchiveSession, onTaskBadgeClick,
}: {
  repo: string;
  sessions: ChatSession[];
  activeSessionId: string | null;
  taskCounts: Record<string, number>;
  onSessionChange: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  onTaskBadgeClick: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? sessions : sessions.slice(0, VISIBLE_PER_GROUP);
  const remaining = sessions.length - visible.length;

  return (
    <div>
      <div className="px-2 pt-1 pb-0.5 text-[10px] uppercase tracking-wide text-text-muted">{repo}</div>
      <div role="tablist" className="flex flex-col gap-0.5">
        {visible.map(s => (
          <SessionRow
            key={s.session_id}
            s={s}
            isActive={s.session_id === activeSessionId}
            taskCount={taskCounts[s.session_id] ?? 0}
            onSessionChange={onSessionChange}
            onRenameSession={onRenameSession}
            onArchiveSession={onArchiveSession}
            onTaskBadgeClick={onTaskBadgeClick}
            showArchive
          />
        ))}
      </div>
      {remaining > 0 && !expanded && (
        <button type="button" onClick={() => setExpanded(true)} className="px-2 py-1 text-[11px] text-accent-secondary">
          Show {remaining} more
        </button>
      )}
    </div>
  );
}

export function SessionSidebarSection({
  sessions, activeSessionId, taskCounts, archivedSessions, showArchived,
  onSessionChange, onCreateSession, onRenameSession, onArchiveSession, onUnarchiveSession,
  onToggleArchivedView, onTaskBadgeClick,
}: Props) {
  const groups = groupByRepo(sessions);
  const archivedGroups = groupByRepo(archivedSessions);

  return (
    <div className="flex flex-col gap-1">
      <button type="button" onClick={onCreateSession} className="px-2 py-1 text-left text-[11px] text-text-muted hover:text-text-primary">
        + New session
      </button>
      {[...groups.entries()].map(([repo, groupSessions]) => (
        <RepoGroup
          key={repo}
          repo={repo}
          sessions={groupSessions}
          activeSessionId={activeSessionId}
          taskCounts={taskCounts}
          onSessionChange={onSessionChange}
          onRenameSession={onRenameSession}
          onArchiveSession={onArchiveSession}
          onTaskBadgeClick={onTaskBadgeClick}
        />
      ))}
      <button type="button" onClick={onToggleArchivedView} className="px-2 py-1 text-left text-[10px] text-text-muted">
        {showArchived ? 'Hide archived' : 'Show archived'}
      </button>
      {showArchived && [...archivedGroups.entries()].map(([repo, groupSessions]) => (
        <div key={`archived-${repo}`} className="opacity-60">
          <div className="px-2 pt-1 pb-0.5 text-[10px] uppercase tracking-wide text-text-muted">{repo} (archived)</div>
          {groupSessions.map(s => (
            <div key={s.session_id} className="flex items-center justify-between rounded px-2 py-1 text-[12px]">
              <span className="truncate">{s.title}</span>
              <button type="button" onClick={() => onUnarchiveSession(s.session_id)} className="text-[10px] text-accent-secondary">
                Unarchive
              </button>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: Run the test**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/layout/SessionSidebarSection.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/SessionSidebarSection.tsx crates/vox-gui/ui/src/components/layout/SessionSidebarSection.test.tsx
git commit -m "feat(vox-gui-ui): add SessionSidebarSection with rename, real archive/unarchive, repo grouping"
```

---

## Task 9: Wire into `Sidebar.tsx`, retire `ChatSessionRail`/`SessionsPanel`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:67-99` (props), `:241-259` (children render branch)
- Modify: `crates/vox-gui/ui/src/App.tsx` (own `useChatSessions`, pass into `Sidebar`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (remove the `'sessions'` dockview panel and its docking dependents — see the expanded checklist below)
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx` (and its test file, if any)

**Corrected by adversarial review — the dockview removal is more involved than "delete the panel."**
Direct inspection of `ChatSurface.tsx` found the `'sessions'` panel is a real docking anchor other
panels position relative to, not just a self-contained widget:

- `CORE_PANEL_IDS` (`ChatSurface.tsx:41`) includes `'sessions'`.
- `SessionsPanel` (`ChatSurface.tsx:108`) is registered in the dockview component map
  (`ChatSurface.tsx:270`, `sessions: SessionsPanel`).
- `panelDefs.sessions` (`ChatSurface.tsx:820`) and `panelDefs.transcript.referenceChain: ['sessions']`
  (`ChatSurface.tsx:821`) — the main chat panel's reference chain points at `'sessions'`.
- The `transcript` panel is *added* with `position: { direction: 'right', referencePanel: 'sessions'
  }` (`ChatSurface.tsx:1122-1131`) — i.e. the main chat panel is docked relative to the sessions
  panel. Deleting `'sessions'` without repointing this breaks `transcript`'s layout anchor, and
  `executionRail` docks relative to `transcript` in turn, so the breakage cascades.
- `DockWorkspaceShell` persists the dockview layout to `localStorage` under `storageKeyPrefix:
  "gui.chat"`; the `onReady` handler explicitly guards `if (!event.api.getPanel('sessions'))` before
  adding it, meaning **any user with a previously-persisted layout** (i.e. everyone who has used the
  Chat surface before this ships) has a serialized layout that still references the `'sessions'`
  panel id and component. This needs an explicit, tested fallback, not just manual verification.

- [ ] **Step 1: Add sidebar props for sessions**

In `Sidebar.tsx`, extend `SidebarProps` (after line 82's `onOpenCommandPalette?`):

```typescript
  chatSessions?: ChatSession[];
  activeSessionId?: string | null;
  chatTaskCounts?: Record<string, number>;
  archivedChatSessions?: ChatSession[];
  showArchivedChatSessions?: boolean;
  onSessionChange?: (sessionId: string) => void;
  onCreateSession?: () => void;
  onRenameSession?: (sessionId: string, title: string) => void;
  onArchiveSession?: (sessionId: string) => void;
  onUnarchiveSession?: (sessionId: string) => void;
  onToggleArchivedSessions?: () => void;
  onTaskBadgeClick?: (sessionId: string) => void;
```

Add the import at the top of the file:

```typescript
import { SessionSidebarSection } from './SessionSidebarSection';
import type { ChatSession } from '../../lib/useChatSessions';
```

Destructure the new props in the `Sidebar()` function signature (after `onOpenCommandPalette,`).

- [ ] **Step 2: Render `SessionSidebarSection` for the `chat` nav key instead of the generic static children**

Replace the block at lines 241-259 (the full `{isExpanded && children && ( ... )}` block, verbatim below is the current content):

```tsx
                {isExpanded && children && (
                  <div className="ml-4 flex flex-col gap-0.5 border-l border-border-subtle pl-2">
                    {children.map(childKey => (
                      <button
                        key={childKey}
                        type="button"
                        onClick={() => onOpenTab(childKey)}
                        aria-current={view === childKey ? 'page' : undefined}
                        className={`w-full rounded-lg px-2 py-1.5 text-left font-display text-[11px] tracking-[0.1em] uppercase transition ${
                          view === childKey
                            ? 'bg-brass/10 text-brass'
                            : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
                        }`}
                      >
                        {labelForNavKey(childKey)}
                      </button>
                    ))}
                  </div>
                )}
```

Replace it with:

```tsx
                {isExpanded && key === 'chat' && chatSessions && (
                  <div className="ml-4 border-l border-border-subtle pl-2 max-h-[50vh] overflow-y-auto custom-scrollbar">
                    <SessionSidebarSection
                      sessions={chatSessions}
                      activeSessionId={activeSessionId ?? null}
                      taskCounts={chatTaskCounts ?? {}}
                      archivedSessions={archivedChatSessions ?? []}
                      showArchived={showArchivedChatSessions ?? false}
                      onSessionChange={onSessionChange ?? (() => {})}
                      onCreateSession={onCreateSession ?? (() => {})}
                      onRenameSession={onRenameSession ?? (() => {})}
                      onArchiveSession={onArchiveSession ?? (() => {})}
                      onUnarchiveSession={onUnarchiveSession ?? (() => {})}
                      onToggleArchivedView={onToggleArchivedSessions ?? (() => {})}
                      onTaskBadgeClick={onTaskBadgeClick ?? (() => {})}
                    />
                  </div>
                )}
                {isExpanded && key !== 'chat' && children && (
                  <div className="ml-4 flex flex-col gap-0.5 border-l border-border-subtle pl-2">
                    {children.map(childKey => (
                      <button
                        key={childKey}
                        type="button"
                        onClick={() => onOpenTab(childKey)}
                        aria-current={view === childKey ? 'page' : undefined}
                        className={`w-full rounded-lg px-2 py-1.5 text-left font-display text-[11px] tracking-[0.1em] uppercase transition ${
                          view === childKey
                            ? 'bg-brass/10 text-brass'
                            : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
                        }`}
                      >
                        {labelForNavKey(childKey)}
                      </button>
                    ))}
                  </div>
                )}
```

The `key !== 'chat'` branch is the original code, unchanged, moved under a narrower condition so it no longer applies to `chat`.

- [ ] **Step 3: Own the hook in `App.tsx`, wire error handling, and pass it to `Sidebar`**

In `App.tsx`, import and call the hook once near the top of the `App()` function body (alongside the other top-level state):

```typescript
import { useChatSessions } from './lib/useChatSessions';
// ...
const chatSessionsApi = useChatSessions();
const [showArchivedSessions, setShowArchivedSessions] = useState(false);
const [archivedSessions, setArchivedSessions] = useState<ChatSession[]>([]);
```

Find the existing `<Sidebar ... />` render call and add, matching the existing `pushToast`
error-handling pattern from `ChatSurface.tsx` (Task 7's revision note) rather than leaving failures
unhandled:

```tsx
chatSessions={chatSessionsApi.sessions}
activeSessionId={activeSessionId}
chatTaskCounts={chatTaskCounts}
archivedChatSessions={archivedSessions}
showArchivedChatSessions={showArchivedSessions}
onSessionChange={setActiveSessionId}
onCreateSession={() => {
  chatSessionsApi.createSession()
    .then(s => setActiveSessionId(s.session_id))
    .catch(err => pushToast({ tone: 'warn', title: 'New session failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
}}
onRenameSession={(sessionId, title) => {
  chatSessionsApi.renameSession(sessionId, title)
    .catch(err => pushToast({ tone: 'warn', title: 'Rename failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
}}
onArchiveSession={(sessionId) => {
  chatSessionsApi.archiveSession(sessionId, {
    wasActive: sessionId === activeSessionId,
    onReassign: setActiveSessionId,
  }).catch(err => pushToast({ tone: 'warn', title: 'Archive failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
}}
onUnarchiveSession={(sessionId) => {
  chatSessionsApi.unarchiveSession(sessionId)
    .then(() => invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: true }))
    .then(all => setArchivedSessions(all.filter(s => !chatSessionsApi.sessions.some(active => active.session_id === s.session_id))))
    .catch(err => pushToast({ tone: 'warn', title: 'Unarchive failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
}}
onToggleArchivedSessions={() => {
  const next = !showArchivedSessions;
  setShowArchivedSessions(next);
  if (next) {
    invoke<ChatSession[]>('chat_list_sessions', { limit: 200, includeArchived: true })
      .then(all => setArchivedSessions(all.filter(s => !chatSessionsApi.sessions.some(active => active.session_id === s.session_id))))
      .catch(err => pushToast({ tone: 'warn', title: 'Load archived sessions failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
  }
}}
onTaskBadgeClick={(sessionId) => {
  // Wired in Task 10 — sets the active plan-panel target instead of the current hardcoded null.
}}
```

`sanitizeErrorForToast` and `pushToast` already exist and are used this same way elsewhere in
`App.tsx`/`ChatSurface.tsx` — reuse them, don't reimplement.

`ChatSurface` no longer receives session CRUD props (see Task 7's Ownership note) — only whatever
subset of `activeId`/messages-related props it already had before this change.

- [ ] **Step 4: Remove the `ChatSessionRail`/`SessionsPanel` dockview panel and its docking dependents**

In `ChatSurface.tsx`:
1. Remove `'sessions'` from `CORE_PANEL_IDS` (line 41).
2. Remove the `SessionsPanel` function (line 108) and its `sessions: SessionsPanel` entry from the
   dockview component map (line 270).
3. Remove the `sessionRailNode` variable (line 698) and the `<ChatSessionRail ...>` JSX it built,
   and the `import { ChatSessionRail } from './ChatSessionRail';` (line 15).
4. Remove `panelDefs.sessions` (line 820). Change `panelDefs.transcript.referenceChain: ['sessions']`
   (line 821) to `referenceChain: []` (or drop the field if it's optional) — `transcript` no longer
   references a panel that no longer exists.
5. In the `onReady` handler (around lines 1113-1132): remove the `if (!event.api.getPanel('sessions'))
   { addPanel({ id: 'sessions', ... }) }` block entirely (lines 1113-1121). Change `transcript`'s
   `addPanel` call to drop `position: { direction: 'right', referencePanel: 'sessions' }` — `transcript`
   becomes the layout's leftmost/default panel instead of being positioned relative to `sessions`
   (omit `position` entirely, or use whatever this dockview version's convention is for "default/first
   panel" — check `IDockviewPanelProps`/`addPanel` usage elsewhere in this file for panels that don't
   take a `position`, e.g. how the very first panel added to a fresh layout is handled, and match it).
6. **Persisted-layout fallback (the localStorage risk above):** `DockWorkspaceShell`'s restore path
   reads a serialized layout that may still list `id: 'sessions', component: 'sessions'` from before
   this change. Since `'sessions'` is removed from the `CHAT_DOCK_COMPONENTS`/component map in step 2,
   restoring such a layout will ask dockview to render a component id that no longer has a
   registration. Add an explicit guard rather than relying on undocumented dockview behavior: after
   `DockWorkspaceShell` restores a layout (wherever it currently calls `event.api.fromJSON(...)` or
   equivalent — locate this via `grep -n "storageKeyPrefix\|fromJSON" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`),
   check `event.api.getPanel('sessions')` immediately after restore and, if present, call
   `event.api.getPanel('sessions')?.api.close()` to remove the stale panel from the live layout (the
   next save will persist the corrected layout without it). Write a test for this in whatever test
   file already covers `ChatSurface`'s dockview restore behavior (search for existing
   `storageKeyPrefix`/layout-restore tests before adding a new file).

- [ ] **Step 5: Delete `ChatSessionRail.tsx`**

```bash
git rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx
git rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx 2>/dev/null || true
```

- [ ] **Step 6: Run the full frontend test suite and fix breakage**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: PASS. Fix any test that still references `ChatSessionRail` or the removed dockview panel. Add a test asserting `SessionSidebarSection` renders when `isExpanded && key === 'chat' && chatSessions` is true (Step 2's new branch had no dedicated test in an earlier draft of this plan).

- [ ] **Step 7: Manual verification in the running app**

Start the dev server (`vox run` or the project's existing GUI dev-run command — check `.claude/launch.json` if present), open the app **with an existing, previously-persisted Chat dock layout** (don't test only against a fresh profile — the persisted-layout fallback from Step 4.6 is exactly what this needs to exercise), confirm the layout doesn't crash or show a broken panel. Then: expand "Chat" in the sidebar, confirm sessions grouped by repo render, "+ New session" creates one, clicking a session switches the active chat, renaming works, archiving the active session switches to another session, "Show archived" reveals real archived sessions with working Unarchive, a session with 6+ items in one repo shows "Show N more" that expands independently of other repo groups.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(vox-gui-ui): move session switching into the global sidebar, retire ChatSessionRail/SessionsPanel"
```

---

## Task 10: Wire the task badge to `PlanPanel` (fix `chatPlanSessionId: null`)

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (the `chatPlanSessionId: null` hardcode, and `chatTaskCounts` state)

**Corrected by adversarial review:** a chat session can have multiple `plan_sessions` rows (Task 4),
so there is no single stored `plan_session_id` to jump to — the badge click needs to resolve which
plan session to open at click time.

- [ ] **Step 1: Add state for which session's plan panel is open**

In `App.tsx`, near `activeSessionId`'s `useState`, add:

```typescript
const [openPlanSessionId, setOpenPlanSessionId] = useState<string | null>(null);
```

- [ ] **Step 2: Replace the hardcoded `chatPlanSessionId: null`**

Find the literal `chatPlanSessionId: null` (search `grep -n "chatPlanSessionId" crates/vox-gui/ui/src/App.tsx`) and replace with:

```typescript
chatPlanSessionId: openPlanSessionId,
```

- [ ] **Step 3: Add a Tauri command to resolve a chat session's most-recently-updated plan session, and wire the badge click to it**

Add to `crates/vox-gui/src/commands/plan_panel.rs` (near `plan_open_task_counts` from Task 6):

```rust
/// The most recently updated `plan_sessions` row linked to a chat session, if any — used to
/// pick which plan DAG the sidebar's task badge opens when a chat session has dispatched more
/// than one goal (each dispatch mints its own `plan_sessions` row; see `goal.rs`).
#[tauri::command]
pub async fn latest_plan_session_for_chat(
    pool: State<'_, GuiDbPool>,
    session_id: String,
) -> Result<Option<String>, String> {
    let db = pool_db(&pool)?;
    db.latest_plan_session_id_for_origin(&session_id).await.map_err(map_db_err)
}
```

Add the corresponding DB method to `crates/vox-db/src/store/ops_planning.rs` (near
`get_plan_session_by_id`):

```rust
    /// Most recently updated `plan_sessions.plan_session_id` for a given `origin_session_id`,
    /// or `None` if that chat session has never dispatched a goal.
    pub async fn latest_plan_session_id_for_origin(
        &self,
        origin_session_id: &str,
    ) -> Result<Option<String>, StoreError> {
        let origin = origin_session_id.to_string();
        let mut rows = self
            .conn
            .query(
                "SELECT plan_session_id FROM plan_sessions
                 WHERE origin_session_id = ?1
                 ORDER BY updated_at DESC LIMIT 1",
                params![origin.as_str()],
            )
            .await?;
        Ok(match rows.next().await? {
            Some(r) => Some(r.get::<String>(0)?),
            None => None,
        })
    }
```

Register `latest_plan_session_for_chat` in the invoke handler (same location as Task 5 Step 7).

Add a same-file test to `ops_planning.rs` and `plan_panel.rs` respectively:

```rust
// ops_planning.rs
#[tokio::test]
async fn latest_plan_session_id_for_origin_picks_the_most_recently_updated_row() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    db.create_plan_session("plan-old", Some("chat-z"), "goal one", "sequential").await.unwrap();
    db.create_plan_session("plan-new", Some("chat-z"), "goal two", "sequential").await.unwrap();
    // Touch plan-new again so its updated_at is later than plan-old's.
    db.update_plan_session_goal_text("plan-new", "goal two, revised").await.unwrap();

    let latest = db.latest_plan_session_id_for_origin("chat-z").await.unwrap();
    assert_eq!(latest.as_deref(), Some("plan-new"));
}
```

```rust
// plan_panel.rs
#[tokio::test]
async fn latest_plan_session_for_chat_returns_none_for_a_session_with_no_dispatched_goals() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();

    let result = latest_plan_session_for_chat(pool, "chat-with-no-tasks".to_string()).await.unwrap();
    assert_eq!(result, None);
}
```

- [ ] **Step 4: Fill in the `onTaskBadgeClick` handler from Task 9 Step 3**

```tsx
onTaskBadgeClick={(sessionId) => {
  invoke<string | null>('latest_plan_session_for_chat', { sessionId })
    .then(planSessionId => {
      if (planSessionId) {
        setOpenPlanSessionId(planSessionId);
        setActiveSessionId(sessionId);
      }
    })
    .catch(err => pushToast({ tone: 'warn', title: 'Open tasks failed', body: sanitizeErrorForToast(err), cause: 'backend-error' }));
}}
```

- [ ] **Step 5: Populate `chatTaskCounts` on the `Sidebar` via the batched command**

Add a small effect in `App.tsx` that fetches open-task counts for the visible sessions in one
round trip and feeds `chatTaskCounts`:

```typescript
const [chatTaskCounts, setChatTaskCounts] = useState<Record<string, number>>({});

useEffect(() => {
  let cancelled = false;
  const sessionIds = chatSessionsApi.sessions.map(s => s.session_id);
  if (sessionIds.length === 0) {
    setChatTaskCounts({});
    return;
  }
  invoke<Record<string, number>>('plan_open_task_counts', { sessionIds })
    .then(counts => { if (!cancelled) setChatTaskCounts(counts); })
    .catch(() => { if (!cancelled) setChatTaskCounts({}); });
  return () => { cancelled = true; };
}, [chatSessionsApi.sessions]);
```

Pass `chatTaskCounts={chatTaskCounts}` into `<Sidebar ... />` (already added in Task 9 Step 3).

- [ ] **Step 6: Manual verification**

Run the dev server, create a session, dispatch a real task from it (via the existing chat→task
dispatch path, not a scratch script — this exercises the real `origin_session_id` link end to end),
confirm the sidebar badge shows the count and clicking it opens `PlanPanel` scoped to that session's
plan.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx crates/vox-gui/src/commands/plan_panel.rs crates/vox-db/src/store/ops_planning.rs crates/vox-gui/src/main.rs crates/vox-gui/src/lib.rs
git commit -m "fix(vox-gui-ui): wire chatPlanSessionId from sidebar task badge via the real origin_session_id link"
```

---

## Plan-level verification

- [ ] Run the full backend suite: `cargo test -p vox-db -p vox-gui`
- [ ] Run the full frontend suite: `cd crates/vox-gui/ui && npx vitest run`
- [ ] Run `vox ci check-codex-ssot` once more to confirm the baseline digest is still consistent after all commits
- [ ] **Upgrade test:** build against a copy of a real pre-this-change `.vox` database file (not just `connect_memory()`) and confirm the app starts and archive/unarchive work — this is the scenario Task 1 exists to fix and the one most likely to be skipped in a fresh-checkout dev loop.
- [ ] Manual pass in the running app per Task 9 Step 7 and Task 10 Step 6, plus: archive a session from the sidebar, confirm it disappears and the active session reassigns; click "Show archived", confirm it reappears muted with real content; unarchive it, confirm it returns to the normal list; rename a session inline.

---

## Parallelism and execution notes (adversarial review, Phase 5)

**File-overlap map** (only disjoint writes with no cross-item dependency justify running tasks as
parallel subagents — "conceptually different" is not sufficient on its own):

- Tasks 1 → 2 → 3 → 4 all modify `crates/vox-db/src/codex_chat.rs` (2, 3, 4) or files it depends on
  (1). These **must run sequentially**, not as parallel subagents, despite each being a conceptually
  separate concern (migration, archive semantics, repo tagging, task counts) — parallel edits to the
  same file would conflict.
- Tasks 5 and 6 both depend on Tasks 1-4 being complete, and touch different files
  (`commands/chat.rs` vs. `commands/plan_panel.rs`) except for one shared touch point: both add an
  entry to the same Tauri invoke-handler registration list in `main.rs`/`lib.rs`. **Genuinely
  parallelizable except for that one shared line** — either run them sequentially, or run them in
  parallel and resolve the small registration-list merge by hand afterward.
- Task 7 (frontend hook) has no compile-time dependency on the Rust backend tasks — it can be
  developed in parallel with the Tasks 1-6 backend track, since it only needs the *planned* Tauri
  command shapes (already fully specified above), not a working backend, to write and pass its own
  unit tests (which mock `invoke`). Real integration only needs to happen before Task 9.
- Task 8 (new component) is fully disjoint from Task 7's files, but its test file does `import type
  { ChatSession } from '../../lib/useChatSessions'` — a type-only dependency on Task 7's file
  existing. In practice this means Task 7 should land first (even just the interface, ahead of full
  hook logic) before Task 8's test can typecheck; the two are not fully independent despite touching
  different files.
- Task 9 depends on both 7 and 8, and itself touches three files at once (`Sidebar.tsx`, `App.tsx`,
  `ChatSurface.tsx` — the last one *again*, overlapping Task 7's Step 6 edits to the same file).
  Task 9 should run after Task 7 is fully merged, not concurrently with it.
- Task 10 depends on Task 9 (same `App.tsx` region) and Tasks 4/6 (the batched-count/latest-plan
  commands) — sequential after both.

**Net shape:** a short, mostly-sequential chain (1→2→3→4→{5∥6}→7→8→9→10) with only two shallow,
narrow parallel opportunities (5∥6, and 7 developed ahead of the backend track). This is a small
number of tasks with a real, mostly-linear dependency chain, not a wide fan-out.

**On whether the Workflow tool fits any part of this plan's execution: it does not, and recommending
it would itself be a scope violation.** None of the three shapes that justify Workflow orchestration
are present here — there is no genuine multi-stage fan-out with a real barrier (the one barrier-like
point, Tasks 1-4 needing to land before 5/6, is a single ordering constraint handled by running tasks
in order, not a discovery-then-transform pipeline), no volume of independent similarly-shaped
sub-items large enough to need a resumable background pipeline (10 tasks, not dozens or hundreds),
and no verify-after-generate loop needing adversarial cross-checking (this review pass already served
that role once, up front, rather than as a per-task runtime loop). Execute this plan with
`superpowers:subagent-driven-development` (one fresh subagent per task, sequential per the dependency
chain above, with the 5∥6 pair optionally run as two concurrent Agent-tool calls) or
`superpowers:executing-plans` for batched inline execution — either is a better fit than Workflow for
a plan this size and shape.

---

## Changelog (adversarial review pass)

**Fixed (survived verification, applied to spec and/or plan):**
- Task 1's migration mechanism: `CREATE TABLE IF NOT EXISTS` alone does not add a column to an
  existing database, and the original plan's `CREATE INDEX` on the new column would have hard-failed
  `execute_batch()` at app startup for every upgrading user — verified directly against
  `crates/vox-db/src/store/open.rs`'s live `migrate()` code path. Fixed with an explicit,
  `PRAGMA table_info`-gated `ALTER TABLE`, plus a new test that actually exercises the
  pre-existing-database path (a `connect_memory()`-only test cannot catch this class of bug).
- The entire plan_sessions "pairing" mechanism (original Tasks 4/5/6/10) queried a fabricated
  `chatplan-{id}` id space that real dispatch code (`orchestrator/task_dispatch/submit/goal.rs`)
  never writes to — the task badge would always have read zero. Fixed by joining on the existing
  `plan_sessions.origin_session_id` column instead, batched into one query across all visible
  sessions (also resolves a separately-flagged N+1-IPC-calls finding).
- `ChatSessionRail.tsx`'s rename feature was silently dropped when Task 9 deleted it, with no
  replacement anywhere in the new `SessionSidebarSection`. Fixed — rename is now implemented inline
  in the sidebar component with a test.
- The "Show archived" toggle in the original Task 8 was a non-functional stub (flipped a label,
  fetched/rendered nothing, never called `onUnarchiveSession`). Fixed — wired to real data with a
  test that checks archived sessions actually render and Unarchive actually fires.
- Spec §6's claim that archive/rename use "optimistic UI update rolls back on error" misdescribed
  the real existing code (`ChatSurface.tsx:671-690` is await-then-update, no rollback anywhere).
  Corrected in both spec and plan; the hook now matches the real pattern instead of inventing one.
- `useChatSessions.archiveSession` didn't reassign `activeSessionId` when the archived session was
  the active one, unlike the existing `ChatSurface.tsx:685` behavior it was supposed to replace.
  Fixed.
- Task 9's App.tsx wiring had no error handling on session create/archive/etc., contradicting the
  spec's own error-handling section. Fixed with the same toast pattern used elsewhere.
- Dockview panel removal (Task 9) understated real work: `CORE_PANEL_IDS`, the component
  registration, `panelDefs.transcript`'s `referenceChain`, and `transcript`'s dock `position:
  { referencePanel: 'sessions' }` (the main chat panel's layout anchor) all needed rewiring, plus an
  explicit fallback for users with a previously-persisted dockview layout referencing the removed
  panel id — none of this was in the original plan, which only said "verify by running the app."
- Soft-archive (Task 2) only updated the sidebar's list query; `chat_find_gui_conversation_id`'s
  find-or-create path didn't check `archived_at`, so a resumed session id could silently resurrect
  and write to an archived conversation. Fixed by excluding archived rows from that lookup (with a
  separate archived-including lookup added for the unarchive path itself, which needs to find the
  archived row on purpose).
- `count_open_plan_nodes`'s version-scoping join, and the sidebar's sort-by-`updated_at`
  requirement, were both unimplemented-or-untested in ways their own test fixtures couldn't have
  caught (single-version tests only; fixtures that all shared one `updated_at` value). Fixed with
  tests that actually exercise a version bump and distinct timestamps respectively.
- Duplicate local `ChatSession` interface in `ChatSurface.tsx` (narrower than the hook's version)
  was left in place by the original Task 7, risking type drift. Fixed — removed, replaced with an
  import.

**Rejected, with reason:**
- A suggestion to add a `plan_session_id` column directly to `conversations` (as a single-source-of-
  truth alternative to deriving it via `origin_session_id`) was considered and rejected: a chat
  session can have *multiple* `plan_sessions` rows (one per dispatched goal), so a single stored
  column on `conversations` would be lossy by construction, not just a style preference — the
  existing one-to-many `origin_session_id` link is the correct shape, not an accident to fix.
- A suggestion to add server-side per-repository access scoping to `chat_list_sessions` (flagged by
  the security review as a new cross-repo information-flow path once `repository_id` carries real
  signal) was not applied as a code change here. This is a real, evidenced observation — see the
  spec's new §3 "known limitation" note — but building access-control infrastructure this codebase
  has nowhere else (no `user_id` scoping exists on any existing chat/session query) is out of scope
  for a sidebar layout feature and would itself be a YAGNI violation; documented as a limitation
  instead of silently building new infrastructure to address it.
- Backfilling `repository_id` for pre-existing sessions was considered and rejected: there is no
  reliable signal for what repo an old, untagged session was actually about, so a bulk migration
  would be guessing, not fixing. Documented as a known, permanent "Other" bucket for pre-upgrade
  history instead (spec §3).
- Reconciling `repository_id` drift across a repo directory rename/move was considered and rejected
  as out of scope — would require a repo-identity-reconciliation system this codebase doesn't have
  anywhere else. Documented as a known limitation.

**Gaps added (net-new test/step coverage, not present in either draft before this review):**
- Task 1: existing-database migration test (Step 7) — the class of test most likely to be skipped
  in a fresh-checkout dev loop, and the one that actually catches the startup-breaking bug.
  Plan-level verification also adds an explicit "upgrade a real pre-existing `.vox` file" manual
  check.
- Task 2: archived-conversation-not-resurrected-by-find-or-create test.
- Task 4: version-bump exclusion test for the batched open-task-count query.
- Task 8: sort-order test with distinct fixture timestamps; rename test; real archived-toggle test.
- Task 9: persisted-dockview-layout fallback, plus a test for the new `key === 'chat'` render
  branch (neither existed in the prior draft).

**Removed from this plan:** the original Task 11 (fixing the dead `vox_chat_sessions` localStorage
key in `Tasks/TasksView.tsx`) was removed as scope creep — it's a real, pre-existing, low-severity
bug the audit happened to notice, but it touches a third surface unrelated to the sidebar, repo
grouping, or archive semantics that is the stated goal of this plan, and doesn't block shipping any
of it. Recommended as a separate, small follow-up plan rather than bundled here.

**What remains genuinely unverifiable without running the code:** whether dockview's restore
behavior actually throws/crashes vs. silently drops an unregistered panel id when deserializing a
persisted layout (Task 9's fallback is written defensively regardless, but the exact failure mode
wasn't confirmed against dockview's source — this is a real gap in this review, not resolved here);
the exact digest value `vox ci check-codex-ssot` will print for the new baseline (Task 1 Step 4 —
inherently only knowable by running the command); whether `tauri::State::clone()` and the
`turso::Value::from(&str)` conversions used in Task 4's dynamic-`IN`-clause query compile exactly as
written versus needing a minor signature adjustment (both patterns have direct precedent elsewhere in
this codebase, cited inline, but were not compiled as part of this review).
