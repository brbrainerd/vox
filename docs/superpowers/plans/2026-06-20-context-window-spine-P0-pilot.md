# Context Window Spine — P0 Pilot Implementation Plan

> **For agentic workers:** Use `superpowers:executing-plans`. Steps use `- [ ]` checkboxes.
> This is the **pathway-validation pilot** for the design at
> `docs/superpowers/specs/2026-06-20-context-window-management-design.md` (§14.1). It is
> deliberately the lowest-blast-radius vertical slice: **vox-db only, no consumers, no
> destructive GC.** Its purpose is to prove the Antigravity/Flash pathway returns useful
> answers before the larger chunks are authored.

**Goal:** Add the `context_windows` + `context_window_items` tables and read-only store
accessors (including a content-hash *reference count*) to `vox-db`, green and committed,
with the schema baseline policy kept in lockstep.

**Architecture:** Two new tables registered through the existing `SCHEMA_FRAGMENTS`
manifest; item content is stored once in the existing CAS (`objects`) via `store()` and
referenced by `content_hash`. A new `context_window_store.rs` mirrors `history_store.rs`.
Refcount is a **read-only COUNT** over live (non-trimmed) items — **no deletes in this
pilot.**

**Tech stack:** Rust, `vox-db` crate, Turso/libSQL, `tokio::test`, `DbConfig::Memory`.

---

> 🤖 **EXECUTION TARGET — READ FIRST.** This plan is written for **Gemini Flash 3.5 in
> Antigravity**. Flash has ~48% unaided in-IDE completion, **no mid-task checkpoint**, weak
> long-context recall, and a hard quota cutoff. See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.

**Operating Rules (apply to EVERY task):**
1. **Atomic + green + committed.** Tests pass before you commit. Never leave a broken tree.
   A kill between tasks must leave compiling, tested code.
2. **Verify before use.** Every `rg`/read in a Step-1 pre-flight is a **BLOCKING gate**: run
   it, paste the output, and if on-disk reality differs from this plan, **STOP and report** —
   do not guess or invent APIs.
3. **Two-strike circuit breaker.** If a step's verification fails twice, STOP and write a
   handoff note in the ledger. Do not thrash or loop.
4. **Split on overrun.** If an implement step would touch >1 file or add >1 new function,
   make one atomic green commit per sub-bullet.
5. **House rules (Rust):** build/test/lint per-crate only — `cargo build -p vox-db`,
   `cargo test -p vox-db`, `cargo clippy -p vox-db -- -D warnings`,
   `cargo fmt -p vox-db` (NEVER `cargo fmt --all`). No stubs, no `#[allow(dead_code)]`,
   no `todo!()`. Prove EFFECT: tests must INSERT and SELECT real rows, not assert on
   string shape.
6. **No unplanned shared edits.** Do not touch any file outside the per-task **Files** list
   except the two explicitly-authorized shared files in Task 1
   (`schema/manifest.rs`, `schema/domains/mod.rs`, `contracts/db/baseline-version-policy.yaml`).
   Report any other edit you believe is required instead of making it.
7. **Branch discipline.** Work ONLY on branch `claude/context-window-spine`. Every commit
   stays on it. Do not branch off, do not rebase, do not merge.

## Flash Execution Addendum (2026-06-20)

**Mandatory pre-flight (run ALL, paste output, confirm before any code):**
```
git rev-parse --abbrev-ref HEAD                                  # MUST be claude/context-window-spine
rg -n "BASELINE_VERSION" crates/vox-db/src/schema/manifest.rs    # confirm current integer (expect 79)
rg -n "history_entries" crates/vox-db/src/schema/manifest.rs     # confirm fragment registration pattern
rg -n "pub mod history" crates/vox-db/src/schema/domains/mod.rs  # confirm module decl pattern
rg -n "pub mod history_store" crates/vox-db/src/lib.rs           # confirm store module decl pattern
rg -n "repository_baseline_integer|repository_baseline_digest_hex" contracts/db/baseline-version-policy.yaml
```
If `BASELINE_VERSION` is NOT 79, or any pattern is absent, **STOP** — the plan is stale
against the tree; report the discrepancy and do not proceed.

**Context-reuse note (do not re-derive):** the CAS API is `db.store(kind, &[u8]) -> hash`
and `db.get(&hash) -> Vec<u8>` (in `crates/vox-db/src/store/ops_cas.rs`). The `objects`
table is SHARED across subsystems — **this pilot never deletes from `objects`.** Refcount is
read-only.

**Task-split table:**
| Task | Touches | Tag |
|---|---|---|
| 1 — schema + manifest + baseline policy | `schema/domains/context_windows.rs` (new), `schema/domains/mod.rs`, `schema/manifest.rs`, `contracts/db/baseline-version-policy.yaml` | **[SEQUENTIAL]** (shared manifest) |
| 2 — store accessors + refcount | `context_window_store.rs` (new), `lib.rs` (one `pub mod` line) | **[SEQUENTIAL]** (must follow Task 1; needs the tables) |

Both tasks are SEQUENTIAL; run them in order in a single worker. Do not parallelize.

---

### Task 1: Schema tables + manifest registration + baseline policy

**Files:**
- Create: `crates/vox-db/src/schema/domains/context_windows.rs`
- Modify: `crates/vox-db/src/schema/domains/mod.rs` (add one `pub mod` line, alphabetical)
- Modify: `crates/vox-db/src/schema/manifest.rs` (bump `BASELINE_VERSION`; append one `SchemaFragment`)
- Modify: `contracts/db/baseline-version-policy.yaml` (integer + digest)

- [ ] **Step 1 (pre-flight, BLOCKING):** run the Flash Execution Addendum pre-flight block
  above; paste output. Confirm `BASELINE_VERSION = 79`. If not, STOP.

- [ ] **Step 2: Create the schema fragment with a round-trip test.** New file
  `crates/vox-db/src/schema/domains/context_windows.rs`:
```rust
//! Arca schema fragment for ContextWindow spine (design 2026-06-20 §4.1).

pub const SCHEMA_CONTEXT_WINDOWS: &str = r#"
CREATE TABLE IF NOT EXISTS context_windows (
    id                TEXT PRIMARY KEY,
    repo_id           TEXT NOT NULL,
    title             TEXT,
    kind              TEXT NOT NULL,             -- 'chat'|'task'|'agent'|'a2a'|'archived'
    tier              TEXT NOT NULL DEFAULT 'hot', -- 'hot'|'warm'|'cold'|'frozen'
    parent_window_id  TEXT,
    root_window_id    TEXT NOT NULL,
    agent_id          TEXT,
    thread_id         TEXT,
    trace_id          TEXT,
    model_route       TEXT,
    git_sha_at_open   TEXT,
    git_sha_at_close  TEXT,
    token_estimate    INTEGER NOT NULL DEFAULT 0,
    pinned            INTEGER NOT NULL DEFAULT 0,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    deleted_at        INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ctxwin_repo_tier ON context_windows(repo_id, tier, updated_at);
CREATE INDEX IF NOT EXISTS idx_ctxwin_tree      ON context_windows(root_window_id, parent_window_id);

CREATE TABLE IF NOT EXISTS context_window_items (
    id             TEXT PRIMARY KEY,
    window_id      TEXT NOT NULL,
    ordinal        INTEGER NOT NULL,
    role           TEXT NOT NULL,                -- 'user'|'assistant'|'system'|'tool'
    item_kind      TEXT NOT NULL,                -- 'message'|'pin'|'attachment'|'summary'|'tool_call'
    content_hash   TEXT NOT NULL,                -- references objects(hash) in CAS
    token_estimate INTEGER NOT NULL DEFAULT 0,
    pinned         INTEGER NOT NULL DEFAULT 0,
    committed      INTEGER NOT NULL DEFAULT 0,
    redacted       INTEGER NOT NULL DEFAULT 0,
    created_at     INTEGER NOT NULL,
    trimmed_at     INTEGER
);
CREATE INDEX IF NOT EXISTS idx_ctxitem_window  ON context_window_items(window_id, ordinal);
CREATE INDEX IF NOT EXISTS idx_ctxitem_hash    ON context_window_items(content_hash);
"#;

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn context_windows_schema_round_trip() {
        let db = crate::VoxDb::connect(crate::DbConfig::Memory).await.expect("db");
        db.connection().execute(
            "INSERT INTO context_windows (id, repo_id, kind, root_window_id, token_estimate, created_at, updated_at)
             VALUES ('w1','r1','chat','w1',0,1000,1000)", ()).await.expect("insert window");
        db.connection().execute(
            "INSERT INTO context_window_items (id, window_id, ordinal, role, item_kind, content_hash, created_at)
             VALUES ('i1','w1',0,'user','message','deadbeef',1000)", ()).await.expect("insert item");
        let mut q = db.connection()
            .query("SELECT content_hash FROM context_window_items WHERE window_id='w1'", ())
            .await.expect("q");
        let row = q.next().await.expect("r").expect("row");
        assert_eq!(row.get::<String>(0).expect("hash"), "deadbeef");
    }
}
```
  > NOTE: the round-trip test runs the FULL baseline (`VoxDb::connect` applies all
  > fragments), so it only passes after Step 4 registers the fragment.

- [ ] **Step 3: Register the module.** In `crates/vox-db/src/schema/domains/mod.rs`, add
  `pub mod context_windows;` in alphabetical position (after `pub mod conversations;`).

- [ ] **Step 4: Register the fragment + bump baseline.** In
  `crates/vox-db/src/schema/manifest.rs`:
  - change `pub const BASELINE_VERSION: i64 = 79;` → `= 80;` and add a comment line above it:
    `// 80: feat(context): add context_windows + context_window_items (design 2026-06-20)`
  - append one entry to the END of the `SCHEMA_FRAGMENTS` array (after the `history_entries`
    fragment):
```rust
    SchemaFragment {
        name: "context_windows",
        sql: domains::context_windows::SCHEMA_CONTEXT_WINDOWS,
    },
```

- [ ] **Step 5: Run the round-trip test — expect PASS.**
  Run: `cargo test -p vox-db context_windows_schema_round_trip`
  Expected: PASS. If it fails on a missing table, you skipped Step 3/4 registration.

- [ ] **Step 6: Update the baseline policy digest (BLOCKING — Flash usually skips this).**
  Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema`
  Expected: **FAIL** with a panic message of the form
  `baseline-version-policy.yaml digest is stale; set repository_baseline_digest_hex to 0x<HEX>`.
  Copy that exact `0x<HEX>` value and the integer `80` into
  `contracts/db/baseline-version-policy.yaml`:
  set `repository_baseline_integer: 80` and `repository_baseline_digest_hex: 0x<HEX>`.

- [ ] **Step 7: Re-run the policy test — expect PASS.**
  Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema`
  Expected: PASS. If still failing, you pasted the wrong digest — re-read the panic. Two
  strikes → STOP and report.

- [ ] **Step 8: Full gate.**
  Run: `cargo build -p vox-db && cargo test -p vox-db && cargo clippy -p vox-db -- -D warnings && cargo fmt -p vox-db`
  Expected: all green.

- [ ] **Step 9: Commit.**
  `git commit -am "feat(vox-db): context_windows + context_window_items schema (P0 spine pilot)"`

---

### Task 2: Store accessors + read-only content-hash refcount

**Files:**
- Create: `crates/vox-db/src/context_window_store.rs`
- Modify: `crates/vox-db/src/lib.rs` (one `pub mod context_window_store;` line near `pub mod history_store;`)

- [ ] **Step 1 (pre-flight, BLOCKING):** confirm Task 1 is committed and tables exist:
  `git log --oneline -1` and
  `rg -n "context_windows" crates/vox-db/src/schema/manifest.rs`. If absent, STOP.

- [ ] **Step 2: Write the store with tests (one file, prove EFFECT).** Create
  `crates/vox-db/src/context_window_store.rs` with: a `create_window(db, id, repo_id, kind,
  root_window_id, now) -> Result<(), StoreError>`; an `add_item(db, item_id, window_id,
  ordinal, role, item_kind, content: &[u8], now) -> Result<String, StoreError>` that calls
  `db.store("ctxwin-item", content).await?` to put the blob in CAS and inserts the item row
  with the returned `content_hash`, returning the hash; a `count_hash_references(db, hash)
  -> Result<i64, StoreError>` that returns
  `SELECT COUNT(*) FROM context_window_items WHERE content_hash = ?1 AND trimmed_at IS NULL`;
  and a `mark_item_trimmed(db, item_id, now)` that sets `trimmed_at`. Mirror the
  breaker/conn pattern from `history_store.rs::add_entry_with_caps`. Include `#[tokio::test]`s
  (using `DbConfig::Memory`) that:
  - create a window, add the SAME content to two items, assert `count_hash_references == 2`
    and that the two items share one `content_hash` (dedup proven via CAS);
  - assert `db.get(&hash)` returns the original bytes (content really landed in CAS);
  - trim one item, assert `count_hash_references == 1` (refcount respects `trimmed_at`).
  > Do NOT implement any DELETE against `objects`. Refcount is read-only in this pilot.

- [ ] **Step 3: Declare the module.** In `crates/vox-db/src/lib.rs`, add
  `pub mod context_window_store;` adjacent to `pub mod history_store;`.

- [ ] **Step 4: Run the new tests — expect PASS.**
  Run: `cargo test -p vox-db context_window_store`
  Expected: PASS.

- [ ] **Step 5: Full gate.**
  Run: `cargo build -p vox-db && cargo test -p vox-db && cargo clippy -p vox-db -- -D warnings && cargo fmt -p vox-db`
  Expected: all green.

- [ ] **Step 6: Commit.**
  `git commit -am "feat(vox-db): context window store accessors + read-only hash refcount (P0)"`

---

## Done criteria (the go/no-go signal)

When both tasks are committed on `claude/context-window-spine`:

- [ ] Append ONE entry to `docs/superpowers/antigravity-handoff-ledger.md` (next `AGH-####`):
  date, plan = this file, subsystem = `vox-db / context-window spine`, target =
  `gemini-3.5-flash / antigravity`, delivered = the exact files changed (INCLUDING the 3
  shared files), `loc`, outcome (green/partial/failed), verification (the exact gate
  commands + pass/fail), any deviation from this plan, and 1–3 `prompt_lessons`.
- [ ] Post the final `git log --oneline` for the branch and the full output of the Step-5
  gate so the reviewer can confirm **effect, not shape**.

**Out of scope for P0 (do NOT implement):** the projector, tiering, GC/deletes, retrieval,
any consumer wiring, any GUI, any graphify change. Those are later chunks authored only
after this pilot's review verdict is green.
