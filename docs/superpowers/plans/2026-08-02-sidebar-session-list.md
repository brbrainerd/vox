# Sidebar Session List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move chat-session switching out of the Chat-surface-internal dockview panel and into the global `Sidebar.tsx`, grouped by repository with independent per-repo-group overflow, wired to real per-session task lists, with Archive changed from a hard delete to a recoverable soft-archive.

**Architecture:** Three layers, bottom-up. (1) `vox-db`: add `conversations.archived_at`, start populating the already-existing `conversations.repository_id`, pair every chat session with a `plan_sessions` row. (2) `vox-gui` Tauri commands: extend `ChatSessionDto`, resolve the current repo via `vox-repository`, expose archive/unarchive/open-task-count. (3) `vox-gui/ui`: extract session-list state out of `ChatSurface.tsx` into a shared hook so both `Sidebar.tsx` and the Chat surface read the same state, build `SessionSidebarSection.tsx`, wire it under the "Chat" nav item, retire `ChatSessionRail.tsx`.

**Tech Stack:** Rust (turso async SQLite driver, Tauri commands), React + TypeScript (Vite), Vitest/React Testing Library for frontend tests, `cargo test` for backend tests.

**Spec:** [docs/superpowers/specs/2026-08-02-sidebar-session-list-design.md](../specs/2026-08-02-sidebar-session-list-design.md)

---

## Task 1: `conversations.archived_at` column + BASELINE_VERSION bump

**Files:**
- Modify: `crates/vox-db/src/schema/domains/conversations.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs:23`
- Modify: `contracts/db/baseline-version-policy.yaml`
- Test: `crates/vox-db/src/local_tests.rs`

- [ ] **Step 1: Add the column to the baseline DDL**

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

Also add an index right after the existing `idx_conversations_repository` line:

```rust
CREATE INDEX IF NOT EXISTS idx_conversations_archived ON conversations(archived_at);
```

- [ ] **Step 2: Bump `BASELINE_VERSION`**

In `crates/vox-db/src/schema/manifest.rs:23`, change:

```rust
pub const BASELINE_VERSION: i64 = 87;
```

- [ ] **Step 3: Recompute the baseline digest and update the policy file**

Run:

```bash
cargo test -p vox-db schema_baseline_digest_hex -- --nocapture
```

If no such test prints the digest directly, instead run the SSOT checker, which fails with the expected value:

```bash
vox ci check-codex-ssot
```

Read the "expected" digest from the failure output, then update `contracts/db/baseline-version-policy.yaml`:

```yaml
  repository_baseline_integer: 87
  # re-updated for 87: conversations.archived_at (soft-archive for GUI chat sessions,
  # sidebar session-list feature).
  repository_baseline_digest_hex: "0x<PASTE THE DIGEST PRINTED BY THE CHECK ABOVE>"
```

- [ ] **Step 4: Verify the SSOT check passes**

Run: `vox ci check-codex-ssot`
Expected: passes with no digest mismatch.

- [ ] **Step 5: Write a migration-safety test**

Add to `crates/vox-db/src/local_tests.rs` (find the existing `#[cfg(test)] mod` block and add a new `#[tokio::test]` alongside the others):

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

- [ ] **Step 6: Run the test**

Run: `cargo test -p vox-db conversations_archived_at_column_exists_and_defaults_null`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/src/schema/domains/conversations.rs crates/vox-db/src/schema/manifest.rs contracts/db/baseline-version-policy.yaml crates/vox-db/src/local_tests.rs
git commit -m "feat(vox-db): add conversations.archived_at column (BASELINE_VERSION 87)"
```

---

## Task 2: Real soft-archive/unarchive in `vox-db`

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs:679-693` (existing `chat_archive_conversation`)
- Test: same file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

Add near the existing tests for `codex_chat.rs` (search the file for `#[cfg(test)]`; add alongside):

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
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-db archive_conversation_is_recoverable_not_deleted`
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

- [ ] **Step 4: Update `chat_list_gui_sessions` to filter by archive state**

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
        let sql = if include_archived {
            "SELECT c.id, c.title, c.external_session_id, c.updated_at,
                    (SELECT COUNT(*) FROM conversation_messages m WHERE m.conversation_id = c.id),
                    c.repository_id
             FROM conversations c
             WHERE c.origin_surface = 'gui'
               AND c.external_session_id NOT LIKE 'bg-task-%'
             ORDER BY c.updated_at DESC
             LIMIT ?1"
        } else {
            "SELECT c.id, c.title, c.external_session_id, c.updated_at,
                    (SELECT COUNT(*) FROM conversation_messages m WHERE m.conversation_id = c.id),
                    c.repository_id
             FROM conversations c
             WHERE c.origin_surface = 'gui'
               AND c.external_session_id NOT LIKE 'bg-task-%'
               AND c.archived_at IS NULL
             ORDER BY c.updated_at DESC
             LIMIT ?1"
        };
        let mut rows = self.connection().query(sql, params![lim]).await?;
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

- [ ] **Step 5: Run the test**

Run: `cargo test -p vox-db archive_conversation_is_recoverable_not_deleted`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-db): soft-archive conversations instead of hard delete"
```

---

## Task 3: Populate `repository_id` on session create

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs:587-614` (`chat_ensure_gui_session`)
- Test: same file

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
    /// repository this session targets (see `vox_repository::compute_repository_id`
    /// for how callers derive `repository_id`).
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

## Task 4: Pair every chat session with a `plan_sessions` row + open-task count

**Files:**
- Modify: `crates/vox-db/src/codex_chat.rs` (near `chat_ensure_gui_session_with_repo`)
- Test: same file

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn create_paired_plan_session_is_idempotent_and_counts_open_nodes() {
    let db = crate::VoxDb::connect_memory().await.unwrap();
    let plan_id = db.create_paired_plan_session("chat-plan-1", "New chat").await.unwrap();

    // Idempotent: calling again with the same id doesn't error or duplicate.
    let plan_id_again = db.create_paired_plan_session("chat-plan-1", "New chat").await.unwrap();
    assert_eq!(plan_id, plan_id_again);

    // Fresh plan session has zero open nodes.
    assert_eq!(db.count_open_plan_nodes(&plan_id).await.unwrap(), 0);

    db.upsert_plan_node(&plan_id, 1, "n1", "do the thing", "[]", "{}", "pending", None)
        .await
        .unwrap();
    assert_eq!(db.count_open_plan_nodes(&plan_id).await.unwrap(), 1);

    db.upsert_plan_node(&plan_id, 1, "n1", "do the thing", "[]", "{}", "completed", None)
        .await
        .unwrap();
    assert_eq!(db.count_open_plan_nodes(&plan_id).await.unwrap(), 0);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-db create_paired_plan_session_is_idempotent_and_counts_open_nodes`
Expected: compile error, `create_paired_plan_session`/`count_open_plan_nodes` not found.

- [ ] **Step 3: Implement both methods**

Add to `crates/vox-db/src/codex_chat.rs` (anywhere inside `impl crate::VoxDb { ... }`, e.g. right after `chat_ensure_gui_session_with_repo`):

```rust
    /// Create (or return the existing) `plan_sessions` row paired 1:1 with a chat
    /// session, keyed by the chat session's own external id. Idempotent — safe to
    /// call every time a chat session is created/loaded.
    pub async fn create_paired_plan_session(
        &self,
        chat_external_session_id: &str,
        goal_text: &str,
    ) -> Result<String, StoreError> {
        let plan_session_id = format!("chatplan-{chat_external_session_id}");
        self.create_plan_session(&plan_session_id, Some(chat_external_session_id), goal_text, "chat")
            .await?;
        self.append_plan_version(&plan_session_id, 1, None, None, None).await?;
        Ok(plan_session_id)
    }

    /// Count plan nodes at the current version that are not yet resolved
    /// (pending/queued/in_progress — mirrors `vox_orchestrator::planning::types::PlanStatus`,
    /// whose variants serialize `snake_case`; "resolved" means completed/failed/cancelled/superseded).
    pub async fn count_open_plan_nodes(&self, plan_session_id: &str) -> Result<i64, StoreError> {
        let pid = plan_session_id.to_string();
        let mut rows = self
            .connection()
            .query(
                "SELECT COUNT(*) FROM plan_nodes pn
                 JOIN plan_sessions ps
                   ON ps.plan_session_id = pn.plan_session_id AND ps.current_version = pn.version
                 WHERE pn.plan_session_id = ?1
                   AND pn.status IN ('pending', 'queued', 'in_progress')",
                params![pid.as_str()],
            )
            .await?;
        let row = rows.next().await?.expect("COUNT(*) always returns one row");
        row.get(0).map_err(|e| StoreError::Db(e.to_string()))
    }
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p vox-db create_paired_plan_session_is_idempotent_and_counts_open_nodes`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/codex_chat.rs
git commit -m "feat(vox-db): pair chat sessions with a plan_sessions row, add open-task count"
```

---

## Task 5: Wire it all into the `vox-gui` Tauri command layer

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs:16-92` (`ChatSessionDto`, `chat_create_session`, `chat_list_sessions`), `:291-` (`chat_archive_session`)
- Test: `crates/vox-gui/src/commands/chat.rs` `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

Add near the existing tests in `chat.rs` (search for `#[cfg(test)]`):

```rust
#[tokio::test]
async fn chat_create_session_sets_repository_id_and_plan_session_id() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();

    let dto = chat_create_session(pool, Some("Test".into())).await.unwrap();

    assert!(dto.repository_id.is_some(), "repository_id should resolve from cwd");
    assert!(dto.plan_session_id.is_some(), "every session should get a paired plan_session_id");
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

Run: `cargo test -p vox-gui chat_create_session_sets_repository_id_and_plan_session_id`
Expected: compile error — `ChatSessionDto` has no `repository_id`/`plan_session_id` fields yet, `chat_unarchive_session` doesn't exist, `chat_list_sessions` signature mismatch.

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
    pub plan_session_id: Option<String>,
}
```

- [ ] **Step 4: Resolve the current repo and pair a plan session in `chat_create_session`**

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

    let plan_session_id = db
        .create_paired_plan_session(&session_id, &title)
        .await
        .map_err(map_db_err)?;

    Ok(ChatSessionDto {
        session_id,
        title,
        updated_at: String::new(),
        message_count: 0,
        conversation_id: conv_id,
        repository_id,
        plan_session_id: Some(plan_session_id),
    })
}
```

`RepositoryContext` (`crates/vox-repository/src/lib.rs:138`) already carries a computed `repository_id: String` field (blake3 over origin + root path), so no separate hashing call is needed — `discover_repository_or_fallback` is the one resolver call that does it all.

- [ ] **Step 5: Update `chat_list_sessions` for the new DB signature + archived filter + plan_session_id**

Replace `chat_list_sessions` (lines 72-92). Note this now needs each row's `plan_session_id`, computed the same way Task 4 does (`format!("chatplan-{external_session_id}")`) rather than a second DB round-trip:

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
                let plan_session_id = Some(format!("chatplan-{session_id}"));
                ChatSessionDto {
                    session_id,
                    title,
                    updated_at,
                    message_count,
                    conversation_id,
                    repository_id,
                    plan_session_id,
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

- [ ] **Step 7: Register the new command in the Tauri invoke handler**

Find where `chat_archive_session` is registered (grep `generate_handler!` or the invoke-handler list in `crates/vox-gui/src/main.rs` or `crates/vox-gui/src/lib.rs` for `chat_archive_session`) and add `chat_unarchive_session` to the same list.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p vox-gui chat_create_session_sets_repository_id_and_plan_session_id chat_archive_and_unarchive_session_round_trip`
Expected: PASS

- [ ] **Step 9: Fix any other callers broken by the `ChatSessionDto`/`chat_list_sessions` signature changes**

Run: `cargo check -p vox-gui`
Fix any remaining call sites (e.g. `secretary_confirm_task` or other code constructing `ChatSessionDto` or calling `chat_list_sessions`/`chat_ensure_gui_session`) to match the new fields/signature.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/src/commands/chat.rs crates/vox-gui/src/main.rs crates/vox-gui/src/lib.rs
git commit -m "feat(vox-gui): expose repository_id, plan_session_id, and unarchive on chat sessions"
```

---

## Task 6: `plan_open_task_count` Tauri command

**Files:**
- Modify: `crates/vox-gui/src/commands/plan_panel.rs`
- Test: same file's `#[cfg(test)]` module

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn plan_open_task_count_reflects_pending_nodes() {
    let app = tauri::test::mock_app();
    app.manage(GuiDbPool::connect_memory().await.expect("memory pool"));
    let pool = app.state::<GuiDbPool>();
    let db = pool.handle().unwrap();

    db.create_plan_session("ps-count-1", None, "goal", "sequential").await.unwrap();
    db.append_plan_version("ps-count-1", 1, None, None, None).await.unwrap();
    db.upsert_plan_node("ps-count-1", 1, "n1", "step", "[]", "{}", "pending", None).await.unwrap();

    let count = plan_open_task_count(pool, "ps-count-1".to_string()).await.unwrap();
    assert_eq!(count, 1);
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-gui plan_open_task_count_reflects_pending_nodes`
Expected: compile error, function not found.

- [ ] **Step 3: Add the command**

Add to `crates/vox-gui/src/commands/plan_panel.rs`, near `list_plan_nodes`:

```rust
/// Lightweight count of not-yet-resolved plan nodes for a session's current
/// version, used for the sidebar task-count badge (avoids shipping the full
/// node list just to render a number).
#[tauri::command]
pub async fn plan_open_task_count(
    pool: State<'_, GuiDbPool>,
    plan_session_id: String,
) -> Result<i64, String> {
    let db = pool_db(&pool)?;
    db.count_open_plan_nodes(&plan_session_id).await.map_err(map_db_err)
}
```

- [ ] **Step 4: Register the command in the invoke handler** (same location as Task 5 Step 7)

- [ ] **Step 5: Run the test**

Run: `cargo test -p vox-gui plan_open_task_count_reflects_pending_nodes`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/plan_panel.rs crates/vox-gui/src/main.rs crates/vox-gui/src/lib.rs
git commit -m "feat(vox-gui): add plan_open_task_count command for the sidebar task badge"
```

---

## Task 7: Extract shared session-list state into `useChatSessions`

**Files:**
- Create: `crates/vox-gui/ui/src/lib/useChatSessions.ts`
- Test: `crates/vox-gui/ui/src/lib/useChatSessions.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (remove local session CRUD, consume the hook instead)
- Modify: `crates/vox-gui/ui/src/App.tsx` (own the hook, pass down to both `Sidebar` and `ChatSurface`)

**Why:** today `ChatSurface.tsx` owns `loadSessions`/`createSession`/`renameSession`/`archiveSession` as local state, invisible to `Sidebar.tsx`. Lifting it into a hook is what lets the sidebar and the Chat surface show the same session list without duplicating fetch/CRUD logic or drifting out of sync.

- [ ] **Step 1: Locate the exact current implementation**

Run: `grep -n "loadSessions\|createSession\|renameSession\|archiveSession" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

Read the matched region fully before proceeding — the hook below must reproduce the same Tauri call shapes and error handling `ChatSurface.tsx` already has, not invent new ones.

- [ ] **Step 2: Write the failing test first**

```typescript
// crates/vox-gui/ui/src/lib/useChatSessions.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useChatSessions } from './useChatSessions';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('useChatSessions', () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it('loads sessions on mount', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([
      { session_id: 's1', title: 'Session 1', updated_at: '', message_count: 0, conversation_id: 1, repository_id: 'repo-a', plan_session_id: 'chatplan-s1' },
    ]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));
    expect(invoke).toHaveBeenCalledWith('chat_list_sessions', expect.anything());
  });

  it('creates a session and prepends it to the list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]); // initial load
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(0));

    vi.mocked(invoke).mockResolvedValueOnce({
      session_id: 's2', title: 'New chat', updated_at: '', message_count: 0, conversation_id: 2, repository_id: 'repo-a', plan_session_id: 'chatplan-s2',
    });
    await act(async () => { await result.current.createSession(); });

    expect(result.current.sessions[0].session_id).toBe('s2');
    expect(invoke).toHaveBeenCalledWith('chat_create_session', expect.anything());
  });

  it('archiving removes the session from the default (non-archived) list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([
      { session_id: 's1', title: 'Session 1', updated_at: '', message_count: 0, conversation_id: 1, repository_id: 'repo-a', plan_session_id: 'chatplan-s1' },
    ]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    vi.mocked(invoke).mockResolvedValueOnce(undefined); // chat_archive_session
    await act(async () => { await result.current.archiveSession('s1'); });

    expect(result.current.sessions).toHaveLength(0);
    expect(invoke).toHaveBeenCalledWith('chat_archive_session', { sessionId: 's1' });
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
  plan_session_id: string | null;
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

  const createSession = useCallback(async (title?: string) => {
    const created = await invoke<ChatSession>('chat_create_session', { title });
    setSessions(prev => [created, ...prev]);
    return created;
  }, []);

  const renameSession = useCallback(async (sessionId: string, title: string) => {
    await invoke('chat_rename_session', { sessionId, title });
    setSessions(prev => prev.map(s => s.session_id === sessionId ? { ...s, title } : s));
  }, []);

  const archiveSession = useCallback(async (sessionId: string) => {
    await invoke('chat_archive_session', { sessionId });
    setSessions(prev => prev.filter(s => s.session_id !== sessionId));
  }, []);

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
Expected: PASS (adjust the Tauri argument-casing assertions — `sessionId` vs `session_id` — to match whatever `ChatSurface.tsx`'s current calls actually use, found in Step 1; Tauri's default is camelCase command args).

- [ ] **Step 6: Replace `ChatSurface.tsx`'s local session CRUD with the hook**

In `ChatSurface.tsx`, remove the local `loadSessions`/`createSession`/`renameSession`/`archiveSession` functions and their backing `useState`, replacing with:

```typescript
import { useChatSessions } from '../../../lib/useChatSessions';
// ...
const { sessions, createSession, renameSession, archiveSession } = useChatSessions();
```

Thread `sessions`/`createSession`/etc. through to wherever `ChatSurface.tsx` currently passes its local versions (props into `ChatSessionRail`, if it's kept per Task 9 — otherwise this wiring is removed there in Task 9).

- [ ] **Step 7: Run existing Chat surface tests**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Chat`
Expected: PASS (fix any test that mocked the old local functions directly instead of `useChatSessions`)

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/lib/useChatSessions.ts crates/vox-gui/ui/src/lib/useChatSessions.test.ts crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "refactor(vox-gui-ui): extract session CRUD into shared useChatSessions hook"
```

---

## Task 8: `SessionSidebarSection` component

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/SessionSidebarSection.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/SessionSidebarSection.test.tsx`

- [ ] **Step 1: Write the failing test**

```typescript
// crates/vox-gui/ui/src/components/layout/SessionSidebarSection.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionSidebarSection } from './SessionSidebarSection';
import type { ChatSession } from '../../lib/useChatSessions';

function session(overrides: Partial<ChatSession>): ChatSession {
  return {
    session_id: 's1', title: 'Untitled', updated_at: '', message_count: 0,
    conversation_id: 1, repository_id: 'vox', plan_session_id: 'chatplan-s1', ...overrides,
  };
}

describe('SessionSidebarSection', () => {
  it('groups sessions by repository_id under separate headers', () => {
    const sessions = [
      session({ session_id: 'a', repository_id: 'vox' }),
      session({ session_id: 'b', repository_id: 'vox-server' }),
    ];
    render(<SessionSidebarSection sessions={sessions} activeSessionId="a" taskCounts={{}} onSessionChange={() => {}} onCreateSession={() => {}} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={() => {}} />);
    expect(screen.getByText('vox')).toBeInTheDocument();
    expect(screen.getByText('vox-server')).toBeInTheDocument();
  });

  it('sessions without a repository_id fall under "Other"', () => {
    render(<SessionSidebarSection sessions={[session({ repository_id: null })]} activeSessionId={null} taskCounts={{}} onSessionChange={() => {}} onCreateSession={() => {}} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={() => {}} />);
    expect(screen.getByText('Other')).toBeInTheDocument();
  });

  it('truncates each repo group independently at 5, with its own Show more', () => {
    const sessions = Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `Session ${i}`, repository_id: 'vox' }));
    render(<SessionSidebarSection sessions={sessions} activeSessionId={null} taskCounts={{}} onSessionChange={() => {}} onCreateSession={() => {}} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={() => {}} />);
    expect(screen.getAllByRole('tab')).toHaveLength(5);
    const showMore = screen.getByText('Show 2 more');
    fireEvent.click(showMore);
    expect(screen.getAllByRole('tab')).toHaveLength(7);
  });

  it('expanding one repo group does not affect another', () => {
    const sessions = [
      ...Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `V${i}`, repository_id: 'vox' })),
      session({ session_id: 'w0', title: 'W0', repository_id: 'vox-server' }),
    ];
    render(<SessionSidebarSection sessions={sessions} activeSessionId={null} taskCounts={{}} onSessionChange={() => {}} onCreateSession={() => {}} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={() => {}} />);
    fireEvent.click(screen.getByText('Show 2 more'));
    expect(screen.getAllByRole('tab')).toHaveLength(8); // 7 vox + 1 vox-server, still just one group's shown-count changed
  });

  it('clicking + New session calls onCreateSession', () => {
    const onCreateSession = vi.fn();
    render(<SessionSidebarSection sessions={[]} activeSessionId={null} taskCounts={{}} onSessionChange={() => {}} onCreateSession={onCreateSession} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={() => {}} />);
    fireEvent.click(screen.getByText('+ New session'));
    expect(onCreateSession).toHaveBeenCalled();
  });

  it('clicking a task badge calls onTaskBadgeClick with the session, not onSessionChange', () => {
    const onSessionChange = vi.fn();
    const onTaskBadgeClick = vi.fn();
    render(<SessionSidebarSection sessions={[session({ session_id: 'a' })]} activeSessionId={null} taskCounts={{ a: 3 }} onSessionChange={onSessionChange} onCreateSession={() => {}} onArchiveSession={() => {}} onUnarchiveSession={() => {}} onTaskBadgeClick={onTaskBadgeClick} />);
    fireEvent.click(screen.getByText('3'));
    expect(onTaskBadgeClick).toHaveBeenCalledWith('a');
    expect(onSessionChange).not.toHaveBeenCalled();
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
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onArchiveSession: (sessionId: string) => void;
  onUnarchiveSession: (sessionId: string) => void;
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
  return groups;
}

function RepoGroup({
  repo, sessions, activeSessionId, taskCounts, onSessionChange, onArchiveSession, onTaskBadgeClick,
}: {
  repo: string;
  sessions: ChatSession[];
  activeSessionId: string | null;
  taskCounts: Record<string, number>;
  onSessionChange: (id: string) => void;
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
          <div key={s.session_id} role="tab" aria-selected={s.session_id === activeSessionId}
               onClick={() => onSessionChange(s.session_id)}
               className="flex items-center justify-between rounded px-2 py-1 text-[12px] cursor-pointer hover:bg-overlay-hover">
            <span className="truncate">{s.title}</span>
            {taskCounts[s.session_id] > 0 && (
              <span
                onClick={e => { e.stopPropagation(); onTaskBadgeClick(s.session_id); }}
                className="ml-2 shrink-0 rounded-full bg-overlay-subtle px-1.5 text-[10px] text-text-muted"
              >
                {taskCounts[s.session_id]}
              </span>
            )}
          </div>
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
  sessions, activeSessionId, taskCounts, onSessionChange, onCreateSession, onArchiveSession, onUnarchiveSession, onTaskBadgeClick,
}: Props) {
  const [showArchived, setShowArchived] = useState(false);
  const groups = groupByRepo(sessions);

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
          onArchiveSession={onArchiveSession}
          onTaskBadgeClick={onTaskBadgeClick}
        />
      ))}
      <button type="button" onClick={() => setShowArchived(v => !v)} className="px-2 py-1 text-left text-[10px] text-text-muted">
        {showArchived ? 'Hide archived' : 'Show archived'}
      </button>
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
git commit -m "feat(vox-gui-ui): add SessionSidebarSection grouped-by-repo session list"
```

---

## Task 9: Wire into `Sidebar.tsx`, retire `ChatSessionRail`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:67-99` (props), `:241-250` (children render branch)
- Modify: `crates/vox-gui/ui/src/App.tsx` (own `useChatSessions`, pass into `Sidebar`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (remove the `'sessions'` dockview panel / `SessionsPanel` wrapping `ChatSessionRail`)
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx` (and its test file, if any)

**Resolves the spec's open decision point:** remove `ChatSessionRail.tsx` rather than keep a second, divergent session-switcher UI — the sidebar becomes the single place sessions are created/switched/archived.

- [ ] **Step 1: Add sidebar props for sessions**

In `Sidebar.tsx`, extend `SidebarProps` (after line 82's `onOpenCommandPalette?`):

```typescript
  chatSessions?: ChatSession[];
  activeSessionId?: string | null;
  chatTaskCounts?: Record<string, number>;
  onSessionChange?: (sessionId: string) => void;
  onCreateSession?: () => void;
  onArchiveSession?: (sessionId: string) => void;
  onUnarchiveSession?: (sessionId: string) => void;
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
                      onSessionChange={onSessionChange ?? (() => {})}
                      onCreateSession={onCreateSession ?? (() => {})}
                      onArchiveSession={onArchiveSession ?? (() => {})}
                      onUnarchiveSession={onUnarchiveSession ?? (() => {})}
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

- [ ] **Step 3: Own the hook in `App.tsx` and pass it to both consumers**

In `App.tsx`, import and call the hook once near the top of the `App()` function body (alongside the other top-level state):

```typescript
import { useChatSessions } from './lib/useChatSessions';
// ...
const chatSessionsApi = useChatSessions();
```

Find the existing `<Sidebar ... />` render call and add:

```tsx
chatSessions={chatSessionsApi.sessions}
activeSessionId={activeSessionId}
onSessionChange={setActiveSessionId}
onCreateSession={() => chatSessionsApi.createSession().then(s => setActiveSessionId(s.session_id))}
onArchiveSession={chatSessionsApi.archiveSession}
onUnarchiveSession={chatSessionsApi.unarchiveSession}
onTaskBadgeClick={(sessionId) => {
  const session = chatSessionsApi.sessions.find(s => s.session_id === sessionId);
  if (session?.plan_session_id) {
    // Wired in Task 10 — sets chatPlanSessionId instead of the current hardcoded null.
  }
}}
```

Find where `ChatSurface` is rendered and pass `chatSessionsApi` down (it currently calls `useChatSessions` itself per Task 7 — replace that internal call with props from `App.tsx`, OR leave `ChatSurface` calling its own `useChatSessions()` instance since the hook's `load()` re-fetches from the backend on every mount/toggle and both instances read the same DB state. Prefer passing props from `App.tsx` if `ChatSurface` already receives a large props object from `App.tsx` for other state — check the existing prop-drilling pattern before deciding; do not introduce a new React context for this alone).

- [ ] **Step 4: Remove the `ChatSessionRail`/`SessionsPanel` dockview panel**

In `ChatSurface.tsx`, find `sessionRailNode` and the `'sessions'` dockview panel id / `SessionsPanel` wrapper (per the audit, this is where `ChatSessionRail` is constructed). Remove that panel registration and the `sessionRailNode` variable/import. If the dockview layout persists panel ids in `localStorage`, confirm removing this panel id doesn't crash on old persisted layouts (dockview typically ignores unknown panel ids on restore — verify by running the app and loading with an existing layout before/after this change).

- [ ] **Step 5: Delete `ChatSessionRail.tsx`**

```bash
git rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx
git rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx 2>/dev/null || true
```

- [ ] **Step 6: Run the full frontend test suite and fix breakage**

Run: `cd crates/vox-gui/ui && npx vitest run`
Expected: PASS. Fix any test that still references `ChatSessionRail` or the removed dockview panel.

- [ ] **Step 7: Manual verification in the running app**

Start the dev server (`vox run` or the project's existing GUI dev-run command — check `.claude/launch.json` if present), open the app, expand "Chat" in the sidebar, confirm: sessions grouped by repo render, "+ New session" creates one, clicking a session switches the active chat, a session with 6+ items in one repo shows "Show N more" that expands independently of other repo groups.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(vox-gui-ui): move session switching into the global sidebar, retire ChatSessionRail"
```

---

## Task 10: Wire the task badge to `PlanPanel` (fix `chatPlanSessionId: null`)

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (the `chatPlanSessionId: null` hardcode, ~line 1281 per audit — line number may have shifted after Tasks 7-9's edits; locate by searching for `chatPlanSessionId`)
- Modify: same `onTaskBadgeClick` stub added in Task 9 Step 3

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

- [ ] **Step 3: Fill in the `onTaskBadgeClick` handler from Task 9 Step 3**

```tsx
onTaskBadgeClick={(sessionId) => {
  const session = chatSessionsApi.sessions.find(s => s.session_id === sessionId);
  if (session?.plan_session_id) {
    setOpenPlanSessionId(session.plan_session_id);
    setActiveSessionId(sessionId);
  }
}}
```

- [ ] **Step 4: Populate `chatTaskCounts` on the `Sidebar`**

Add a small effect in `App.tsx` that fetches open-task counts for the visible sessions and feeds `chatTaskCounts`:

```typescript
const [chatTaskCounts, setChatTaskCounts] = useState<Record<string, number>>({});

useEffect(() => {
  let cancelled = false;
  (async () => {
    const entries = await Promise.all(
      chatSessionsApi.sessions
        .filter(s => s.plan_session_id)
        .map(async s => {
          const count = await invoke<number>('plan_open_task_count', { planSessionId: s.plan_session_id });
          return [s.session_id, count] as const;
        }),
    );
    if (!cancelled) setChatTaskCounts(Object.fromEntries(entries));
  })();
  return () => { cancelled = true; };
}, [chatSessionsApi.sessions]);
```

Pass `chatTaskCounts={chatTaskCounts}` into `<Sidebar ... />`.

- [ ] **Step 5: Manual verification**

Run the dev server, create a session, add a plan node to it via whatever existing dev path creates plan nodes (or directly via the `insert_plan_node` command in a scratch script if no UI path exists yet), confirm the sidebar badge shows the count and clicking it opens `PlanPanel` scoped to that session.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "fix(vox-gui-ui): wire chatPlanSessionId from sidebar task badge instead of hardcoded null"
```

---

## Task 11: Remove the dead `vox_chat_sessions` localStorage cache

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` (the `loadSessionTitles()` reading `localStorage.getItem('vox_chat_sessions')`)

**Why:** the audit found nothing writes this key — it's dead code masquerading as working session-title lookup for the Tasks surface's session filter. Now that `useChatSessions` (Task 7) is the real source of truth, wire `TasksView` to it instead of deleting the feature outright.

- [ ] **Step 1: Locate the current dead-code read site**

Run: `grep -n "vox_chat_sessions\|loadSessionTitles" crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`

- [ ] **Step 2: Replace the localStorage read with `useChatSessions`**

Replace `loadSessionTitles()`'s localStorage read with:

```typescript
import { useChatSessions } from '../../../lib/useChatSessions';
// ...
const { sessions } = useChatSessions();
const sessionTitles = Object.fromEntries(sessions.map(s => [s.session_id, s.title]));
```

Remove the now-unused `loadSessionTitles` function and its `localStorage.getItem('vox_chat_sessions')` call.

- [ ] **Step 3: Run the Tasks surface tests**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/Tasks`
Expected: PASS (update any test that mocked `localStorage.getItem('vox_chat_sessions')` to instead mock the `chat_list_sessions` Tauri call).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx
git commit -m "fix(vox-gui-ui): wire Tasks session filter to real session data, drop dead localStorage cache"
```

---

## Plan-level verification

- [ ] Run the full backend suite: `cargo test -p vox-db -p vox-gui`
- [ ] Run the full frontend suite: `cd crates/vox-gui/ui && npx vitest run`
- [ ] Run `vox ci check-codex-ssot` once more to confirm the baseline digest is still consistent after all commits
- [ ] Manual pass in the running app per Task 9 Step 7 and Task 10 Step 5, plus: archive a session from the sidebar, confirm it disappears, click "Show archived", confirm it reappears muted, unarchive it, confirm it returns to the normal list
