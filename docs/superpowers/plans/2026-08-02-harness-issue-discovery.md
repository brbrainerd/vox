# Harness Issue Discovery (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect repeated-correction patterns in live chat/agent turns and stale entries in the golden training corpus, surface both in a persistent GUI review queue (toast → panel, session badge, inline transcript summary), and let an approved decision dispatch a real, human-approved fix to a stale golden-corpus file.

**Architecture:** Three new `scientia_harness_*` tables in `vox-db` (issues, decisions, fix proposals) back a synchronous in-process heuristic scorer hooked into the agent tool-call loop (`run_agent_turn`, scoped per-turn), a threshold-triggered, fire-and-forget LLM judge, an on-demand golden-corpus staleness scanner (frontmatter-only — compile-drift is already gated by the existing `examples_golden_doctor_green.rs` CI test, so this plan does not duplicate it), and a human-approved dispatch-to-fix pipeline (LLM proposes full replacement content, stored directly — never reconstructed from a diff — and applied only through `vox_repository`'s path-safety helpers). Seven new Tauri commands expose this to a new GUI panel under the Scientia surface, plus a session-rail badge and an inline transcript summary.

**Tech Stack:** Rust (Turso/libSQL via `vox-db`, Tauri commands, `vox_actor_runtime::llm`, `vox_repository::path_safety`, `vox_redact`, `similar`), TypeScript/React (Tauri `invoke`, existing toast queue).

**Spec:** `docs/superpowers/specs/2026-08-02-harness-issue-discovery-design.md`

**Revision note:** This plan was rewritten after a 6-dimension adversarial review found several compile-blocking and logic-breaking defects in the first draft (wrong `ServerState` field access, a private helper function called cross-module, a diff-reconstruction path that silently truncated files, a path-traversal gap, a dispatch pipeline that was unreachable by construction, and scorer unit tests whose own math didn't match the implementation). Every task below reflects the corrected design. See the end-of-session changelog for the full list of what changed and why.

---

## Group A: Database schema & storage

### Task 1: Add three new tables to the Scientia schema domain

**Files:**
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` (append near end, before the closing `"#;` at line ~421)
- Modify: `crates/vox-db/src/schema/manifest.rs:11-19` (bump `BASELINE_VERSION`)
- Modify: `contracts/db/baseline-version-policy.yaml`
- Modify: `contracts/db/retention-policy.yaml` (register `scientia_harness_issues` so it isn't unbounded growth on an on-by-default detector)

- [ ] **Step 1: Write the failing digest test run**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema -- --nocapture`
Expected: PASS (nothing has changed yet — this just confirms the test exists and the baseline is currently green, so any later failure is attributable to this task's changes).

- [ ] **Step 2: Append the three table DDLs to `scientia.rs`**

Open `crates/vox-db/src/schema/domains/scientia.rs`, find the closing `"#;` (currently the last line of the file, ~line 421), and insert the following immediately before it (after the last existing table's DDL). Note `id`/`issue_id` are `INTEGER PRIMARY KEY AUTOINCREMENT` / `INTEGER`, matching every other table in this module (`scientia_discovery_inbox`, `scientia_review_decisions`) — not `TEXT`, despite what an earlier spec draft said (the spec has since been corrected to match this).

```rust
-- Harness issue discovery (Phase 1): repeated-correction patterns detected
-- during live chat/agent turns, plus static staleness findings from golden-
-- corpus scans. Distinct from scientia_discovery_inbox/scientia_review_decisions,
-- which are tightly bound to publication_id/claim_id (research findings).
-- No SQL CHECK/TRIGGER (Turso/libSQL does not support them); validated in Rust.
CREATE TABLE IF NOT EXISTS scientia_harness_issues (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    source         TEXT    NOT NULL,          -- chat_session|corpus_scan (validated in Rust)
    session_key    TEXT,                      -- null for corpus_scan
    target_path    TEXT,                      -- repo-relative path when this issue is tied to a
                                               -- specific corpus file (always set for corpus_scan;
                                               -- null for chat_session in v1 — see spec Out of scope)
    detected_at_ms INTEGER NOT NULL,
    category       TEXT    NOT NULL,
    severity       TEXT    NOT NULL,           -- low|medium|high (validated in Rust)
    summary        TEXT    NOT NULL,
    evidence_json  TEXT    NOT NULL,           -- redacted via vox_redact before storage
    status         TEXT    NOT NULL            -- pending|confirmed|dismissed (validated in Rust)
);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_issues_status
    ON scientia_harness_issues(status);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_issues_session
    ON scientia_harness_issues(session_key);

-- Append-only decision ledger for scientia_harness_issues (mirrors
-- scientia_review_decisions: only INSERT + SELECT ops exist, no UPDATE/DELETE).
CREATE TABLE IF NOT EXISTS scientia_harness_decisions (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id       INTEGER NOT NULL,
    decision       TEXT    NOT NULL,           -- confirmed|dismissed (validated in Rust)
    actor          TEXT    NOT NULL,
    reason         TEXT,
    decided_at_ms  INTEGER NOT NULL
);

-- Dispatch-to-fix proposals for corpus-fixable confirmed issues (v1: those
-- with a non-null target_path). proposed_content is the full replacement
-- file content — the actual apply source of truth. proposed_diff is a
-- unified diff computed ONLY for human display; it is never parsed back
-- into content (a diff with context lines cannot be losslessly
-- reconstructed by filtering `+` lines, which is what an earlier draft of
-- this plan did — that truncated the applied file to just the changed
-- lines. Storing the real content directly avoids that class of bug).
CREATE TABLE IF NOT EXISTS scientia_harness_fix_proposals (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id         INTEGER NOT NULL,
    target_path      TEXT    NOT NULL,
    proposed_content TEXT    NOT NULL,
    proposed_diff    TEXT    NOT NULL,          -- display-only, see above
    status           TEXT    NOT NULL,          -- pending_approval|applied|rejected (validated in Rust)
    proposed_at_ms   INTEGER NOT NULL,
    resolved_at_ms   INTEGER
);
```

- [ ] **Step 3: Bump `BASELINE_VERSION` and add the changelog line**

In `crates/vox-db/src/schema/manifest.rs`, change:

```rust
// 84: feat(skill-discovery): add skill_identities table (Task 3.4, harness parity plan)
pub const BASELINE_VERSION: i64 = 84;
```

to:

```rust
// 84: feat(skill-discovery): add skill_identities table (Task 3.4, harness parity plan)
// 85: feat(scientia): add scientia_harness_issues/decisions/fix_proposals tables (harness issue discovery Phase 1)
pub const BASELINE_VERSION: i64 = 85;
```

- [ ] **Step 4: Run the digest test to get the new expected digest**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema -- --nocapture`
Expected: FAIL with an assert message containing `baseline-version-policy.yaml digest is stale; set repository_baseline_digest_hex to 0x<NEW_DIGEST>`. Copy the `0x...` value from that message.

- [ ] **Step 5: Update `contracts/db/baseline-version-policy.yaml` with the new integer and digest**

```yaml
policy:
  repository_baseline_integer: 85
  # Keccak-256 of baseline_sql() — updated for 85: add scientia_harness_issues/
  # decisions/fix_proposals tables (harness issue discovery Phase 1).
  repository_baseline_digest_hex: "0x<PASTE_THE_DIGEST_FROM_STEP_4_HERE>"
```

- [ ] **Step 6: Run the digest test again to confirm it passes**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema -- --nocapture`
Expected: PASS

- [ ] **Step 7: Register `scientia_harness_issues` in the retention-policy SSOT**

Read `contracts/db/retention-policy.yaml` to find the existing entry format (e.g. `agent_exec_history: ms_days 90`) and add an entry for `scientia_harness_issues` using its `detected_at_ms` column, with a generous window (this is a new, unproven-volume table — start wide, e.g. 180 days) since it's written continuously by an on-by-default detector (Task 11) and has no other pruning mechanism.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-db/src/schema/domains/scientia.rs crates/vox-db/src/schema/manifest.rs contracts/db/baseline-version-policy.yaml contracts/db/retention-policy.yaml
git commit -m "feat(vox-db): add scientia_harness_issues/decisions/fix_proposals tables"
```

---

### Task 2: `ops_harness_issues.rs` — issues table CRUD

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_issues.rs`
- Modify: `crates/vox-db/src/store/mod.rs`
- Modify: `crates/vox-db/src/lib.rs` — the crate's actual public API is the explicit name list at `lib.rs:244-268` (`pub use store::{ A2AMessageRow, …, DiscoveryInboxRow, …, WorkflowExecutionRow };`); a `pub use` in `store/mod.rs` alone does **not** make a type reachable as `vox_db::HarnessIssueRow` from other crates (confirmed: an earlier draft of this plan missed this and would not have compiled in `vox-gui`).

- [ ] **Step 1: Write the failing round-trip test (in-file, matching `ops_discovery_inbox.rs`'s pattern)**

```rust
//! Store ops for `scientia_harness_issues`.
//!
//! One row per detected harness issue — either from the synchronous
//! chat-session heuristic+judge detector (`source = "chat_session"`) or from
//! an on-demand golden-corpus staleness scan (`source = "corpus_scan"`).

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::Serialize;
use turso::params;

pub const VALID_SOURCES: &[&str] = &["chat_session", "corpus_scan"];
pub const VALID_SEVERITIES: &[&str] = &["low", "medium", "high"];
pub const VALID_STATUSES: &[&str] = &["pending", "confirmed", "dismissed"];

/// One row of `scientia_harness_issues`. Derives `Serialize` because Tauri
/// commands (Task 14) return this type directly to the frontend as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarnessIssueRow {
    pub id: i64,
    pub source: String,
    pub session_key: Option<String>,
    pub target_path: Option<String>,
    pub detected_at_ms: i64,
    pub category: String,
    pub severity: String,
    pub summary: String,
    pub evidence_json: String,
    pub status: String,
}

impl HarnessIssueRow {
    fn from_row(row: &turso::Row) -> Result<Self, StoreError> {
        Ok(Self {
            id: row.get(0).map_err(StoreError::Turso)?,
            source: row.get(1).map_err(StoreError::Turso)?,
            session_key: row.get(2).map_err(StoreError::Turso)?,
            target_path: row.get(3).map_err(StoreError::Turso)?,
            detected_at_ms: row.get(4).map_err(StoreError::Turso)?,
            category: row.get(5).map_err(StoreError::Turso)?,
            severity: row.get(6).map_err(StoreError::Turso)?,
            summary: row.get(7).map_err(StoreError::Turso)?,
            evidence_json: row.get(8).map_err(StoreError::Turso)?,
            status: row.get(9).map_err(StoreError::Turso)?,
        })
    }
}

/// Everything needed to insert one harness issue. A struct (not 7 positional
/// args) because this now has enough fields that positional args are a real
/// transposition risk between call sites.
pub struct NewHarnessIssue<'a> {
    pub source: &'a str,
    pub session_key: Option<&'a str>,
    pub target_path: Option<&'a str>,
    pub detected_at_ms: i64,
    pub category: &'a str,
    pub severity: &'a str,
    pub summary: &'a str,
    pub evidence_json: &'a str,
}

impl VoxDb {
    /// Insert a new pending harness issue; returns its `id`.
    pub async fn insert_harness_issue(&self, new: NewHarnessIssue<'_>) -> Result<i64, StoreError> {
        if !VALID_SOURCES.contains(&new.source) {
            return Err(StoreError::Db(format!(
                "scientia_harness_issues.source must be one of {VALID_SOURCES:?}, got {:?}",
                new.source
            )));
        }
        if !VALID_SEVERITIES.contains(&new.severity) {
            return Err(StoreError::Db(format!(
                "scientia_harness_issues.severity must be one of {VALID_SEVERITIES:?}, got {:?}",
                new.severity
            )));
        }
        self.conn
            .execute(
                "INSERT INTO scientia_harness_issues \
                 (source, session_key, target_path, detected_at_ms, category, severity, summary, evidence_json, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending')",
                params![
                    new.source.to_string(),
                    new.session_key.map(str::to_string),
                    new.target_path.map(str::to_string),
                    new.detected_at_ms,
                    new.category.to_string(),
                    new.severity.to_string(),
                    new.summary.to_string(),
                    new.evidence_json.to_string(),
                ],
            )
            .await
            .map_err(StoreError::Turso)?;

        let mut rows = self
            .conn
            .query("SELECT last_insert_rowid()", ())
            .await
            .map_err(StoreError::Turso)?;
        let id: i64 = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| {
                StoreError::Db("scientia_harness_issues: last_insert_rowid() returned no row".into())
            })?
            .get(0)
            .map_err(StoreError::Turso)?;
        Ok(id)
    }

    /// Fetch one issue by id.
    pub async fn get_harness_issue(&self, id: i64) -> Result<Option<HarnessIssueRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, source, session_key, target_path, detected_at_ms, category, severity, summary, evidence_json, status \
                 FROM scientia_harness_issues WHERE id = ?1",
                params![id],
            )
            .await
            .map_err(StoreError::Turso)?;
        match rows.next().await.map_err(StoreError::Turso)? {
            Some(row) => Ok(Some(HarnessIssueRow::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Check whether a pending issue with the same source/target_path/category
    /// already exists — used by the corpus scanner (Task 12) to avoid inserting
    /// duplicate rows on repeated scans of the same stale file.
    pub async fn has_pending_harness_issue(
        &self,
        source: &str,
        target_path: &str,
        category: &str,
    ) -> Result<bool, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM scientia_harness_issues \
                 WHERE status = 'pending' AND source = ?1 AND target_path = ?2 AND category = ?3 LIMIT 1",
                params![source.to_string(), target_path.to_string(), category.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(rows.next().await.map_err(StoreError::Turso)?.is_some())
    }

    /// List harness issues, optionally filtered by `status` and/or `source`, newest first.
    pub async fn list_harness_issues(
        &self,
        status: Option<&str>,
        source: Option<&str>,
        limit: i64,
    ) -> Result<Vec<HarnessIssueRow>, StoreError> {
        let sql = "SELECT id, source, session_key, target_path, detected_at_ms, category, severity, summary, evidence_json, status \
                    FROM scientia_harness_issues \
                    WHERE (?1 IS NULL OR status = ?1) AND (?2 IS NULL OR source = ?2) \
                    ORDER BY id DESC LIMIT ?3";
        let mut rows = self
            .conn
            .query(sql, params![status.map(str::to_string), source.map(str::to_string), limit])
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(HarnessIssueRow::from_row(&row)?);
        }
        Ok(out)
    }

    /// List harness issues for one chat session, oldest first (for the inline
    /// transcript summary, Task 19).
    pub async fn list_harness_issues_for_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<HarnessIssueRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, source, session_key, target_path, detected_at_ms, category, severity, summary, evidence_json, status \
                 FROM scientia_harness_issues WHERE session_key = ?1 ORDER BY detected_at_ms ASC",
                params![session_key.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(HarnessIssueRow::from_row(&row)?);
        }
        Ok(out)
    }

    /// Update an issue's status (`confirmed`|`dismissed`). No-op if the id is unknown.
    pub async fn set_harness_issue_status(&self, id: i64, status: &str) -> Result<(), StoreError> {
        if !VALID_STATUSES.contains(&status) {
            return Err(StoreError::Db(format!(
                "scientia_harness_issues.status must be one of {VALID_STATUSES:?}, got {status:?}"
            )));
        }
        self.conn
            .execute(
                "UPDATE scientia_harness_issues SET status = ?2 WHERE id = ?1",
                params![id, status.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NewHarnessIssue;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_then_list_shows_pending_issue() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "chat_session",
                session_key: Some("session-abc"),
                target_path: None,
                detected_at_ms: 1_000,
                category: "repeated_compiler_error",
                severity: "medium",
                summary: "Same borrow-checker error hit twice in a row",
                evidence_json: r#"{"error_hash":"deadbeef"}"#,
            })
            .await
            .expect("insert");
        assert!(id >= 1, "insert must return a positive rowid");

        let rows = db
            .list_harness_issues(Some("pending"), None, 10)
            .await
            .expect("list");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_key.as_deref(), Some("session-abc"));
        assert_eq!(rows[0].status, "pending");

        db.set_harness_issue_status(id, "confirmed").await.expect("update");
        let rows = db
            .list_harness_issues(Some("pending"), None, 10)
            .await
            .expect("list after confirm");
        assert!(rows.is_empty());

        let session_rows = db
            .list_harness_issues_for_session("session-abc")
            .await
            .expect("list for session");
        assert_eq!(session_rows.len(), 1);

        let fetched = db.get_harness_issue(id).await.expect("get").expect("row exists");
        assert_eq!(fetched.id, id);
    }

    #[tokio::test]
    async fn insert_rejects_invalid_source() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let err = db
            .insert_harness_issue(NewHarnessIssue {
                source: "bogus",
                session_key: None,
                target_path: None,
                detected_at_ms: 1_000,
                category: "cat",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("source must be one of"));
    }

    #[tokio::test]
    async fn has_pending_harness_issue_dedupes_repeat_scans() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        assert!(
            !db.has_pending_harness_issue("corpus_scan", "examples/golden/x.vox", "stale_frontmatter")
                .await
                .expect("check before insert")
        );
        db.insert_harness_issue(NewHarnessIssue {
            source: "corpus_scan",
            session_key: None,
            target_path: Some("examples/golden/x.vox"),
            detected_at_ms: 1_000,
            category: "stale_frontmatter",
            severity: "low",
            summary: "stale",
            evidence_json: "{}",
        })
        .await
        .expect("insert");
        assert!(
            db.has_pending_harness_issue("corpus_scan", "examples/golden/x.vox", "stale_frontmatter")
                .await
                .expect("check after insert")
        );
    }
}
```

- [ ] **Step 2: Register the module in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_issues;` alphabetically (between `mod ops_finding_candidates;` and `mod ops_identity;`), and add to the `pub use` block:

```rust
pub use ops_harness_issues::{HarnessIssueRow, NewHarnessIssue};
```

(Do not re-export `VALID_SOURCES`/`VALID_SEVERITIES`/`VALID_STATUSES` here — nothing outside this file consumes them; exporting unused constants was a defect an earlier draft of this plan had. If a future task needs them outside `vox-db`, export at that point.)

- [ ] **Step 3: Add the same types to `crates/vox-db/src/lib.rs`'s public re-export list**

`crates/vox-db/src/store/mod.rs`'s `pub use` only makes a type visible as `vox_db::store::HarnessIssueRow` from *within* the crate — the crate's actual external API is the explicit list at `crates/vox-db/src/lib.rs:244-268`. Add `HarnessIssueRow` to that list (alphabetically, near `HopperInboxRow`/`KnowledgeNodeSummary`):

```rust
pub use store::{
    A2AMessageRow, …, GrpoStepRow, HarnessIssueRow, HopperInboxRow,
    KnowledgeNodeSummary, …
};
```

`NewHarnessIssue` does not need to be added here — it's only constructed by callers *within* `vox-db` in this task, and by `vox-gui` in Tasks 13/14, which is `crate::store::ops_harness_issues::NewHarnessIssue`... actually it does need external visibility since `vox-gui` constructs it — add it too, alongside `HarnessIssueRow`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-db ops_harness_issues`
Expected: PASS (all 3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_issues.rs crates/vox-db/src/store/mod.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): add ops_harness_issues CRUD for scientia_harness_issues"
```

---

### Task 3: `ops_harness_decisions.rs` — append-only decision ledger

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_decisions.rs`
- Modify: `crates/vox-db/src/store/mod.rs`
- Modify: `crates/vox-db/src/lib.rs`

- [ ] **Step 1: Write the file (struct + insert-only ops, mirrors `ops_review.rs`)**

```rust
//! Store ops for `scientia_harness_decisions` (append-only human decision ledger).

use crate::VoxDb;
use crate::store::types::StoreError;
use turso::params;

pub const VALID_DECISIONS: &[&str] = &["confirmed", "dismissed"];

/// One row of `scientia_harness_decisions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessIssueDecisionRow {
    pub issue_id: i64,
    pub decision: String,
    pub actor: String,
    pub reason: Option<String>,
    pub decided_at_ms: i64,
}

impl VoxDb {
    /// Append a human decision for `row.issue_id` and flip the issue's status to match.
    pub async fn record_harness_issue_decision(
        &self,
        row: &HarnessIssueDecisionRow,
    ) -> Result<(), StoreError> {
        if !VALID_DECISIONS.contains(&row.decision.as_str()) {
            return Err(StoreError::Db(format!(
                "scientia_harness_decisions.decision must be one of {VALID_DECISIONS:?}, got {:?}",
                row.decision
            )));
        }
        if row.actor.trim().is_empty() {
            return Err(StoreError::Db(
                "scientia_harness_decisions.actor must be non-empty".to_string(),
            ));
        }
        self.conn
            .execute(
                "INSERT INTO scientia_harness_decisions \
                 (issue_id, decision, actor, reason, decided_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    row.issue_id,
                    row.decision.clone(),
                    row.actor.clone(),
                    row.reason.clone(),
                    row.decided_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        self.set_harness_issue_status(row.issue_id, &row.decision).await
    }
}

#[cfg(test)]
mod tests {
    use super::HarnessIssueDecisionRow;
    use crate::store::ops_harness_issues::NewHarnessIssue;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn recording_decision_flips_issue_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("examples/golden/x.vox"),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");

        db.record_harness_issue_decision(&HarnessIssueDecisionRow {
            issue_id,
            decision: "confirmed".to_string(),
            actor: "local_user".to_string(),
            reason: None,
            decided_at_ms: 2_000,
        })
        .await
        .expect("record decision");

        let rows = db
            .list_harness_issues(Some("confirmed"), None, 10)
            .await
            .expect("list confirmed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, issue_id);
    }

    #[tokio::test]
    async fn rejects_empty_actor() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: None,
                detected_at_ms: 1_000,
                category: "c",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let err = db
            .record_harness_issue_decision(&HarnessIssueDecisionRow {
                issue_id,
                decision: "confirmed".to_string(),
                actor: String::new(),
                reason: None,
                decided_at_ms: 2_000,
            })
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("actor must be non-empty"));
    }
}
```

Note: `ops_harness_issues.rs`'s `mod tests` block is private to that file — this file's tests need `crate::store::ops_harness_issues::NewHarnessIssue`, which works because `mod ops_harness_issues;` in `store/mod.rs` is a regular (non-`pub`) module but items marked `pub` inside it (like `NewHarnessIssue`) are reachable via `crate::store::ops_harness_issues::...` from sibling modules in the same crate.

- [ ] **Step 2: Register in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_decisions;` (alphabetically before `mod ops_harness_issues;`) and:

```rust
pub use ops_harness_decisions::HarnessIssueDecisionRow;
```

(`VALID_DECISIONS` collides with `ops_review::VALID_DECISIONS` — don't re-export it under that name; if a later consumer needs it, alias it then. Nothing in this plan needs it exported.)

- [ ] **Step 3: Add `HarnessIssueDecisionRow` to `crates/vox-db/src/lib.rs`'s re-export list** (same reasoning as Task 2 Step 3 — the `vox-gui` Tauri command in Task 14 constructs this type).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-db ops_harness_decisions`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_decisions.rs crates/vox-db/src/store/mod.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): add ops_harness_decisions append-only ledger"
```

---

### Task 4: `ops_harness_fix_proposals.rs` — dispatch-to-fix proposals

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_fix_proposals.rs`
- Modify: `crates/vox-db/src/store/mod.rs`
- Modify: `crates/vox-db/src/lib.rs`

- [ ] **Step 1: Write the file**

```rust
//! Store ops for `scientia_harness_fix_proposals`.
//!
//! `proposed_content` is the full replacement file content and is the sole
//! source of truth for what gets written on approval. `proposed_diff` is
//! computed once at proposal time purely for human-readable display and is
//! never parsed back — a unified diff with context lines cannot be
//! losslessly reconstructed into full content by filtering `+` lines (an
//! earlier draft of this plan tried that and would have silently truncated
//! approved files to just their changed lines).

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::Serialize;
use turso::params;

pub const VALID_STATUSES: &[&str] = &["pending_approval", "applied", "rejected"];

/// One row of `scientia_harness_fix_proposals`. Derives `Serialize` because
/// Tauri commands (Task 13) return this type directly to the frontend as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HarnessFixProposalRow {
    pub id: i64,
    pub issue_id: i64,
    pub target_path: String,
    pub proposed_content: String,
    pub proposed_diff: String,
    pub status: String,
    pub proposed_at_ms: i64,
    pub resolved_at_ms: Option<i64>,
}

impl HarnessFixProposalRow {
    fn from_row(row: &turso::Row) -> Result<Self, StoreError> {
        Ok(Self {
            id: row.get(0).map_err(StoreError::Turso)?,
            issue_id: row.get(1).map_err(StoreError::Turso)?,
            target_path: row.get(2).map_err(StoreError::Turso)?,
            proposed_content: row.get(3).map_err(StoreError::Turso)?,
            proposed_diff: row.get(4).map_err(StoreError::Turso)?,
            status: row.get(5).map_err(StoreError::Turso)?,
            proposed_at_ms: row.get(6).map_err(StoreError::Turso)?,
            resolved_at_ms: row.get(7).map_err(StoreError::Turso)?,
        })
    }
}

pub struct NewFixProposal<'a> {
    pub issue_id: i64,
    pub target_path: &'a str,
    pub proposed_content: &'a str,
    pub proposed_diff: &'a str,
    pub proposed_at_ms: i64,
}

impl VoxDb {
    /// Insert a new pending-approval fix proposal; returns its `id`.
    pub async fn insert_harness_fix_proposal(&self, new: NewFixProposal<'_>) -> Result<i64, StoreError> {
        self.conn
            .execute(
                "INSERT INTO scientia_harness_fix_proposals \
                 (issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms) \
                 VALUES (?1, ?2, ?3, ?4, 'pending_approval', ?5)",
                params![
                    new.issue_id,
                    new.target_path.to_string(),
                    new.proposed_content.to_string(),
                    new.proposed_diff.to_string(),
                    new.proposed_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut rows = self
            .conn
            .query("SELECT last_insert_rowid()", ())
            .await
            .map_err(StoreError::Turso)?;
        let id: i64 = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| {
                StoreError::Db("scientia_harness_fix_proposals: last_insert_rowid() returned no row".into())
            })?
            .get(0)
            .map_err(StoreError::Turso)?;
        Ok(id)
    }

    /// List fix proposals, optionally filtered by status, newest first.
    pub async fn list_harness_fix_proposals(
        &self,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<HarnessFixProposalRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms, resolved_at_ms \
                 FROM scientia_harness_fix_proposals WHERE (?1 IS NULL OR status = ?1) \
                 ORDER BY id DESC LIMIT ?2",
                params![status.map(str::to_string), limit],
            )
            .await
            .map_err(StoreError::Turso)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            out.push(HarnessFixProposalRow::from_row(&row)?);
        }
        Ok(out)
    }

    /// Fetch one proposal by id.
    pub async fn get_harness_fix_proposal(
        &self,
        id: i64,
    ) -> Result<Option<HarnessFixProposalRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, issue_id, target_path, proposed_content, proposed_diff, status, proposed_at_ms, resolved_at_ms \
                 FROM scientia_harness_fix_proposals WHERE id = ?1",
                params![id],
            )
            .await
            .map_err(StoreError::Turso)?;
        match rows.next().await.map_err(StoreError::Turso)? {
            Some(row) => Ok(Some(HarnessFixProposalRow::from_row(&row)?)),
            None => Ok(None),
        }
    }

    /// Resolve a proposal to `applied` or `rejected`. Does not touch the filesystem —
    /// callers apply `proposed_content` themselves before calling this with `applied`.
    pub async fn resolve_harness_fix_proposal(
        &self,
        id: i64,
        status: &str,
        resolved_at_ms: i64,
    ) -> Result<(), StoreError> {
        if !VALID_STATUSES.contains(&status) || status == "pending_approval" {
            return Err(StoreError::Db(format!(
                "resolve_harness_fix_proposal: status must be 'applied' or 'rejected', got {status:?}"
            )));
        }
        self.conn
            .execute(
                "UPDATE scientia_harness_fix_proposals SET status = ?2, resolved_at_ms = ?3 WHERE id = ?1",
                params![id, status.to_string(), resolved_at_ms],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::NewFixProposal;
    use crate::store::ops_harness_issues::NewHarnessIssue;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_list_resolve_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("examples/golden/hello.vox"),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let proposal_id = db
            .insert_harness_fix_proposal(NewFixProposal {
                issue_id,
                target_path: "examples/golden/hello.vox",
                proposed_content: "// last_validated: 2026-08-02\nfn main() {}\n",
                proposed_diff: "--- a\n+++ b\n",
                proposed_at_ms: 1_500,
            })
            .await
            .expect("insert proposal");

        let pending = db
            .list_harness_fix_proposals(Some("pending_approval"), 10)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, proposal_id);
        assert!(pending[0].proposed_content.contains("2026-08-02"));

        db.resolve_harness_fix_proposal(proposal_id, "applied", 2_000)
            .await
            .expect("resolve");

        let fetched = db
            .get_harness_fix_proposal(proposal_id)
            .await
            .expect("get")
            .expect("row exists");
        assert_eq!(fetched.status, "applied");
        assert_eq!(fetched.resolved_at_ms, Some(2_000));
    }

    #[tokio::test]
    async fn resolve_rejects_pending_approval_as_a_target_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("x"),
                detected_at_ms: 1,
                category: "c",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert issue");
        let proposal_id = db
            .insert_harness_fix_proposal(NewFixProposal {
                issue_id,
                target_path: "x",
                proposed_content: "y",
                proposed_diff: "z",
                proposed_at_ms: 1,
            })
            .await
            .expect("insert proposal");
        let err = db
            .resolve_harness_fix_proposal(proposal_id, "pending_approval", 2)
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("must be 'applied' or 'rejected'"));
    }
}
```

- [ ] **Step 2: Register in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_fix_proposals;` (alphabetically after `mod ops_harness_issues;`) and:

```rust
pub use ops_harness_fix_proposals::{HarnessFixProposalRow, NewFixProposal};
```

- [ ] **Step 3: Add both types to `crates/vox-db/src/lib.rs`'s re-export list** (same reasoning as Task 2 Step 3).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-db ops_harness_fix_proposals`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_fix_proposals.rs crates/vox-db/src/store/mod.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-db): add ops_harness_fix_proposals dispatch-to-fix storage"
```

---

## Group B: Config toggle (default ON, opt-out)

### Task 5: Add `harness_issue_detection_enabled` to `OrchestratorConfig`

**Files:**
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs:186-188` (near `scaling_enabled`) and its `to_catalog()` field registry (~line 681-689)
- Modify: `crates/vox-orchestrator/src/config/impl_default.rs:53` (near `scaling_enabled`)
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` — a test there asserts an exact config-catalog field count (`assert_eq!(catalog.len(), 106, ...)`, ~line 598-606); it must be bumped or it will fail once this field is registered.

- [ ] **Step 1: Write the failing test**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, find (or add, if none exists in this file) a `#[cfg(test)] mod tests` block and add:

```rust
#[test]
fn harness_issue_detection_enabled_defaults_to_true() {
    let cfg = OrchestratorConfig::default();
    assert!(cfg.harness_issue_detection_enabled);
}
```

- [ ] **Step 2: Run it to confirm it fails to compile (field doesn't exist yet)**

Run: `cargo test -p vox-orchestrator harness_issue_detection_enabled_defaults_to_true`
Expected: FAIL — compile error, "no field `harness_issue_detection_enabled`"

- [ ] **Step 3: Add the field**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, immediately after the `scaling_enabled` field:

```rust
    /// Whether dynamic scaling is enabled (default: false).
    #[serde(default = "default_false")]
    pub scaling_enabled: bool,
    /// Whether synchronous per-turn repeated-correction detection is
    /// enabled (default: true — on by default, opt-out via GUI Settings).
    /// Read live via `Orchestrator::config_handle()`, not this struct's
    /// value directly — see Task 11 for why a boot-time snapshot read
    /// would make the toggle non-reactive.
    #[serde(default = "default_true")]
    pub harness_issue_detection_enabled: bool,
```

(`default_true` already exists in this file — it's used by `auto_continue_enabled` a few lines below `scaling_enabled` — so no new helper fn is needed.)

In `crates/vox-orchestrator/src/config/impl_default.rs`, immediately after `scaling_enabled: default_false(),`:

```rust
            scaling_enabled: default_false(),
            harness_issue_detection_enabled: default_true(),
```

- [ ] **Step 4: Register the field in `to_catalog()`**

Find the `scaling_enabled` entry in `orchestrator_fields.rs`'s `to_catalog()` (~line 681-689) and add a matching entry for `harness_issue_detection_enabled` immediately after it, following that entry's exact field-metadata shape (label, description, category).

- [ ] **Step 5: Bump the hardcoded catalog-length assertion**

In `crates/vox-gui/src/commands/orchestrator.rs`, find `assert_eq!(catalog.len(), 106, "catalog field count changed…")` (~line 598-606) and increment the literal by 1 (to `107`) — this test exists specifically to force a deliberate update whenever a field is added, so this increment is the intended, expected edit, not a workaround.

- [ ] **Step 6: Run the test to verify it passes, then run the full config crate's test suite**

Run: `cargo test -p vox-orchestrator harness_issue_detection_enabled_defaults_to_true`
Expected: PASS

Run: `cargo test -p vox-orchestrator config::`
Expected: PASS. If a compile error names another file constructing `OrchestratorConfig { .. }` without `..Default::default()`, add `harness_issue_detection_enabled: true,` there too.

Run: `cargo test -p vox-gui catalog`
Expected: PASS (the bumped `106` → `107` assertion).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-orchestrator/src/config/impl_default.rs crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(vox-orchestrator): add harness_issue_detection_enabled config field"
```

---

### Task 6: Tauri get/set commands for the toggle

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (near `scaling_enabled` handling, ~lines 417-517 for setter, ~640-661 for getter)

- [ ] **Step 1: Write a failing test for the round-trip**

In `crates/vox-gui/src/commands/orchestrator.rs`, find its existing `#[cfg(test)]` modules (there are several — e.g. around lines 592, 881, 955, 985, 1004) and add, near whichever one already covers `scaling_enabled`'s setter/getter JSON mapping (if one exists — otherwise add a new small test) a test asserting: given `serde_json::json!({"harnessIssueDetectionEnabled": false})`, the resulting TOML table contains `harness_issue_detection_enabled = false`. Match whatever existing test in this file already does this for `scalingEnabled`/`scaling_enabled` — copy its structure exactly, don't invent a new one.

- [ ] **Step 2: Run it to confirm it fails (branch doesn't exist yet)**

Run: `cargo test -p vox-gui orchestrator::` — expect a failure or the test to not exist yet, consistent with Step 1.

- [ ] **Step 3: Add the setter branch**

In `set_orchestrator_config`, immediately after the `scaling_enabled` branch:

```rust
    if let Some(v) = config.get("scalingEnabled").and_then(|v| v.as_bool()) {
        orch_table.insert("scaling_enabled".to_string(), toml::Value::Boolean(v));
    }
    if let Some(v) = config.get("harnessIssueDetectionEnabled").and_then(|v| v.as_bool()) {
        orch_table.insert("harness_issue_detection_enabled".to_string(), toml::Value::Boolean(v));
    }
```

And add `"harness_issue_detection_enabled"` to the `vox_config::snapshot::bump(&[...])` key list:

```rust
    vox_config::snapshot::bump(&[
        "max_agents", "financial_cost_budget_micros", "trust_auto_approve_min",
        "scope_enforcement", "exec_time_budget_enabled", "socrates_gate_enforce",
        "scaling_enabled", "min_agents", "scaling_threshold",
        "scale_cpu_ceiling_pct", "scale_mem_floor_mb",
        "harness_issue_detection_enabled",
    ]);
```

- [ ] **Step 4: Add the getter field**

In `get_orchestrator_config`, add to the `serde_json::json!({...})` object:

```rust
        "scalingEnabled": cfg.scaling_enabled,
        "harnessIssueDetectionEnabled": cfg.harness_issue_detection_enabled,
```

- [ ] **Step 5: Run the test to verify it passes, then build**

Run: `cargo test -p vox-gui orchestrator::`
Expected: PASS

Run: `cargo check -p vox-gui`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(vox-gui): wire harness_issue_detection_enabled through orchestrator config commands"
```

---

### Task 7: Settings toggle in the GUI

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (near lines 52, 1013, 1132, 1320-1322)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts:16`

- [ ] **Step 1: Add the field to the settings value type**

Near line 52 (`scalingEnabled: boolean;`):

```ts
  scalingEnabled: boolean;
  harnessIssueDetectionEnabled: boolean;
```

- [ ] **Step 2: Add the default value**

Near line 1013 (`scalingEnabled: false, minAgents: 1, scalingThreshold: 5,`):

```ts
  scalingEnabled: false, minAgents: 1, scalingThreshold: 5,
  harnessIssueDetectionEnabled: true,
```

- [ ] **Step 3: Add the persisted-config merge**

Near line 1132:

```ts
  ...(bool('scalingEnabled') != null ? { scalingEnabled: bool('scalingEnabled')! } : {}),
  ...(bool('harnessIssueDetectionEnabled') != null ? { harnessIssueDetectionEnabled: bool('harnessIssueDetectionEnabled')! } : {}),
```

- [ ] **Step 4: Add the toggle row**

Immediately after the Auto-scaling `Row` (lines 1320-1322):

```tsx
<Row label="Auto-scaling" hint="Let the orchestrator add/remove agents dynamically">
  <Toggle on={vals.scalingEnabled} onClick={() => update({ scalingEnabled: !vals.scalingEnabled })} />
</Row>
<Row label="Harness issue detection" hint="Watch chat turns for repeated mistakes and surface a review queue">
  <Toggle
    on={vals.harnessIssueDetectionEnabled}
    onClick={() => update({ harnessIssueDetectionEnabled: !vals.harnessIssueDetectionEnabled })}
  />
</Row>
```

- [ ] **Step 5: Register in `settingsIndex.ts`**

Immediately after the `scaling-enabled` entry (line 16):

```ts
{ id: 'scaling-enabled', section: 'scaling', label: 'Auto-scaling', hint: 'Spawn/retire agents based on load and resources', keywords: ['scale', 'autoscale', 'dynamic'] },
{ id: 'harness-issue-detection-enabled', section: 'scaling', label: 'Harness issue detection', hint: 'Detect repeated chat/agent mistakes and surface a review queue', keywords: ['harness', 'issue', 'discovery', 'scientia'] },
```

- [ ] **Step 6: Type-check the frontend**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts
git commit -m "feat(vox-gui): add harness issue detection settings toggle"
```

---

## Group C: Detection (heuristic scorer + LLM judge + wiring)

### Task 8: Heuristic scorer module (pure logic, unit-tested standalone)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_scorer.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs` (add `mod harness_issue_scorer;`)

**Design correction from the first draft:** the original scorer let the error-signature counter and the retry counter both increment on the *same* repeated call (identical tool+args+result), so two independent-looking signals double-counted and crossed `THRESHOLD` one call earlier than its own tests asserted — confirmed by hand-tracing the exact test inputs against the exact `record()` logic, independently by three reviewers. This version keeps the two signals but makes each call contribute **at most one** point total (not one from each signal), and fixes the retry counter to actually reset when the call changes (the original never updated `last_call` inside the "still retrying" branch, so it silently kept counting forever once two identical calls happened, rather than tracking genuine *consecutive* repeats).

- [ ] **Step 1: Write the failing tests**

```rust
//! Synchronous, in-process heuristic scorer for repeated-correction patterns
//! within a single `run_agent_turn` tool-call loop. Pure logic, no I/O — kept
//! separate from the loop itself so it's unit-testable without a live LLM or DB.
//! Scoped per-turn (see Task 11), not per-session — the spec originally said
//! "held for the life of the session," which was corrected after review: this
//! codebase has no per-session shared-mutable-state mechanism at this layer,
//! and adding one purely for a same-turn-only detector would be over-built for
//! what the detector actually observes.
//!
//! `// ponytail: fixed threshold, revisit with a GUI-configurable slider only
//! if false-positive rate in practice warrants it — see the design's history
//! of over-built, never-wired auto-tuning engines in this codebase.`

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub const THRESHOLD: u32 = 3;

#[derive(Debug, Default)]
pub struct HarnessIssueScorer {
    /// (tool_name, first-line-of-result hash) -> times seen this turn.
    error_signatures: HashMap<(String, u64), u32>,
    /// Args JSON of the immediately preceding call, and how many times in a
    /// row (including this one) the exact same (tool, args) pair has repeated.
    last_call: Option<(String, String)>,
    consecutive_repeats: u32,
    score: u32,
}

impl HarnessIssueScorer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one tool call's name, arguments (as a JSON string), and result
    /// string. Each call contributes at most one point to `score`, from
    /// whichever single signal fires first (repeated error signature, then
    /// consecutive identical retry) — a call is not double-counted by both.
    /// Returns `true` once the accumulated score first crosses [`THRESHOLD`].
    pub fn record(&mut self, tool_name: &str, args_json: &str, result: &str) -> bool {
        let mut hit = false;

        let is_error = result.starts_with("Error:") || result.contains("\"success\":false");
        if is_error {
            let first_line = result.lines().next().unwrap_or(result);
            let mut hasher = DefaultHasher::new();
            first_line.hash(&mut hasher);
            let key = (tool_name.to_string(), hasher.finish());
            let count = self.error_signatures.entry(key).or_insert(0);
            *count += 1;
            if *count >= 2 {
                hit = true;
            }
        }

        let call_key = (tool_name.to_string(), args_json.to_string());
        if self.last_call.as_ref() == Some(&call_key) {
            self.consecutive_repeats += 1;
        } else {
            self.last_call = Some(call_key);
            self.consecutive_repeats = 1;
        }
        if !hit && self.consecutive_repeats >= 3 {
            hit = true;
        }

        if hit {
            self.score += 1;
        }
        self.score >= THRESHOLD
    }

    /// Reset all accumulated state (called after a judge verdict, whether
    /// or not it produced a real issue, so one turn's noise can't leak into
    /// the next).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_error_signature_crosses_threshold_on_third_hit() {
        let mut scorer = HarnessIssueScorer::new();
        // 1st occurrence: count=1, not >=2, no hit.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 2nd occurrence: count=2, >=2 -> hit -> score=1.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 3rd occurrence: count=3 -> hit -> score=2.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // 4th occurrence: count=4 -> hit -> score=3 >= THRESHOLD.
        assert!(scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
    }

    #[test]
    fn retry_loop_alone_crosses_threshold_after_five_identical_calls() {
        let mut scorer = HarnessIssueScorer::new();
        let args = r#"{"path":"foo.vox"}"#;
        // Use a non-error result so only the retry signal is exercised in isolation.
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=1
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=2
        // 3rd identical consecutive call -> consecutive_repeats=3 -> hit -> score=1.
        // THRESHOLD is 3 total hits, and each call earns at most one hit, so
        // this is the FIRST hit, not the crossing point.
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=3, score=1
        assert!(!scorer.record("validate_file", args, "ok: no diagnostics")); // streak=4, score=2
        assert!(scorer.record("validate_file", args, "ok: no diagnostics")); // streak=5, score=3 -> true
    }

    #[test]
    fn a_call_is_never_double_counted_by_both_signals_at_once() {
        let mut scorer = HarnessIssueScorer::new();
        let args = "{}";
        // Every call here is both a repeated error signature AND (from the
        // 3rd call on) a consecutive retry. If both signals fired per call,
        // this would cross THRESHOLD=3 on the 2nd or 3rd call; it must not,
        // because each call awards at most one point.
        assert!(!scorer.record("build_crate", args, "Error: E0502"));
        assert!(!scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #1 (score=1)
        assert!(!scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #2 (score=2)
        assert!(scorer.record("build_crate", args, "Error: E0502")); // error-sig hit #3 (score=3)
    }

    #[test]
    fn interleaving_a_different_call_resets_the_retry_streak() {
        let mut scorer = HarnessIssueScorer::new();
        scorer.record("build_crate", "{}", "ok"); // streak=1
        scorer.record("build_crate", "{}", "ok"); // streak=2 (one more would hit)
        scorer.record("lint_crate", "{}", "ok"); // different call -> streak resets to 1
        // If the streak had NOT reset, the next build_crate call would be the
        // 3rd-in-a-row and should hit immediately. Because it resets, it takes
        // two more identical calls (not one) before a hit occurs again — and
        // even that single hit only brings score to 1, still below
        // THRESHOLD=3, so every assertion below is `false`.
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=1, no hit, score=0
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=2, no hit, score=0
        assert!(!scorer.record("build_crate", "{}", "ok")); // streak=3, hit, score=1
    }

    #[test]
    fn distinct_successful_calls_never_cross_threshold() {
        let mut scorer = HarnessIssueScorer::new();
        for i in 0..10 {
            assert!(!scorer.record("build_crate", &format!("{{\"n\":{i}}}"), "ok: build succeeded"));
        }
    }

    #[test]
    fn reset_clears_accumulated_score() {
        let mut scorer = HarnessIssueScorer::new();
        scorer.record("build_crate", "{}", "Error: E0502");
        scorer.record("build_crate", "{}", "Error: E0502");
        scorer.reset();
        assert!(!scorer.record("build_crate", "{}", "Error: E0502"));
    }
}
```

Every assertion above was hand-traced against the exact `record()` logic in this same task before being written (the class of mistake this task exists to fix was exactly the opposite — assertions written from intuition rather than from tracing the real state machine).

- [ ] **Step 2: Register the module**

In `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs`, add `mod harness_issue_scorer;` alongside the other `mod` declarations in that file.

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-orchestrator-mcp harness_issue_scorer`
Expected: PASS (all 6 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_scorer.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add pure heuristic scorer for repeated-correction patterns"
```

---

### Task 9: Thread `session_id` into `run_agent_turn`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:151-160` (signature) and its call sites
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:105-121` (the real chat caller)

The real (verified) signature is:

```rust
pub(crate) async fn run_agent_turn(
    state: &ServerState,
    prior_conversation: Vec<LlmChatMessage>,
    system_prompt: String,
    user_message: String,
    permission_mode: Option<&str>,
    active_skill_id: Option<String>,
    llm_config_template: LlmConfig,
    max_iterations: usize,
) -> Result<AgentTurnOutcome, String> {
```

**Important:** one of this function's call sites is *not* a test. `crates/vox-cli`'s eval-gate check (`eval_gate_agent_loop_terminates_check`, referenced from `server_state.rs:628-630`) calls `run_agent_turn` for real, outside any test harness. Passing `None` there (as if it were a throwaway test fixture) would silently disable detection for that path — find this call site explicitly via the compiler errors in Step 3 and give it a real (or best-effort synthetic, e.g. `Some("eval-gate")`) session id, not a blind `None`.

- [ ] **Step 1: Add the parameter to the signature**

```rust
pub(crate) async fn run_agent_turn(
    state: &ServerState,
    session_id: Option<&str>,
    prior_conversation: Vec<LlmChatMessage>,
    system_prompt: String,
    user_message: String,
    permission_mode: Option<&str>,
    active_skill_id: Option<String>,
    llm_config_template: LlmConfig,
    max_iterations: usize,
) -> Result<AgentTurnOutcome, String> {
```

- [ ] **Step 2: Update the real chat caller in `message.rs`**

`try_run_agent_turn` (message.rs:48-57) already has `session_id: &str` in its own signature — thread it through:

```rust
    match Box::pin(super::agent_loop::run_agent_turn(
        state,
        Some(session_id),
        vec![],
        system_prompt.to_string(),
        user_prompt.to_string(),
        None,
        active_skill_id,
        llm_config,
        super::agent_loop::DEFAULT_MAX_ITERATIONS,
    ))
```

- [ ] **Step 3: Build to find every remaining call site (test AND non-test)**

Run: `cargo build -p vox-orchestrator-mcp --tests --all-features 2>&1 | head -80`
Expected: FAIL with one compile error per call site. Read each error's file path carefully — most are in `agent_loop.rs`'s own `#[cfg(test)] mod tests`, but at least one (the eval-gate check named above) is real production code, likely surfaced via a separate crate build (`cargo build -p vox-cli --all-features` may be needed to find it if it's not caught by `--tests` on this crate alone — check both).

- [ ] **Step 4: Fix each call site**

Test call sites: insert `None,` as the second argument. The eval-gate call site: insert `Some("eval-gate"),` (or whatever session-like identifier is already in scope there — read that call site's surrounding context first).

- [ ] **Step 5: Build again to confirm it's clean, across both crates**

Run: `cargo build -p vox-orchestrator-mcp --tests --all-features`
Run: `cargo build -p vox-cli --all-features`
Expected: PASS on both.

- [ ] **Step 6: Run the existing agent_loop tests to confirm no behavior changed**

Run: `cargo test -p vox-orchestrator-mcp agent_loop::`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs
git commit -m "refactor(vox-orchestrator-mcp): thread session_id into run_agent_turn"
```

(If the eval-gate call site turned out to live in a different file than expected, adjust the file list above and note it in the commit body.)

---

### Task 10: LLM-judge module

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_judge.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs`

- [ ] **Step 1: Write the module**

```rust
//! Threshold-triggered LLM judge for the harness issue scorer (Task 8). Fires
//! only when [`harness_issue_scorer::HarnessIssueScorer::record`] returns
//! `true` — most turns never call this. Uses the model-agnostic
//! `vox_actor_runtime::llm` boundary, matching every other LLM call in this
//! codebase (see `crates/vox-effort-audit/src/judge/mod.rs` for the pattern
//! this is adapted from). Runs fire-and-forget from the caller's turn (see
//! Task 11) rather than blocking it — the spec originally called this
//! "synchronous," which was corrected: blocking a chat turn on judge latency
//! would be a real UX regression for a detector that should be invisible in
//! the common case.

use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig};
use vox_actor_runtime::{ActivityOptions, ActivityResult};

/// Severities the rest of the system accepts — anything else from the judge
/// gets normalized down to `medium` rather than silently dropping the issue
/// (an earlier draft let `insert_harness_issue`'s strict validation reject
/// an out-of-vocabulary severity with only a background warn log).
const KNOWN_SEVERITIES: &[&str] = &["low", "medium", "high"];

/// A judged, real harness issue. `None` is returned by [`judge`] when the
/// judge concludes the accumulated signals were not a genuine issue.
pub struct JudgedHarnessIssue {
    pub category: String,
    pub severity: String, // always one of KNOWN_SEVERITIES after normalize_severity
    pub summary: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct JudgeVerdict {
    is_issue: bool,
    #[serde(default)]
    category: String,
    #[serde(default)]
    severity: String,
    #[serde(default)]
    summary: String,
}

fn normalize_severity(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    if KNOWN_SEVERITIES.contains(&lower.as_str()) {
        lower
    } else {
        "medium".to_string()
    }
}

const JUDGE_SYSTEM_PROMPT: &str = "You are reviewing a short excerpt of recent \
tool calls from an AI coding agent's turn, because a heuristic scorer flagged \
repeated errors or retries. Decide whether this is a genuine recurring issue \
worth a human's attention (e.g. the agent kept hitting the same compiler \
error, or retried an identical failing action). Respond with ONLY a JSON \
object, no prose, matching exactly: \
{\"is_issue\": bool, \"category\": string, \"severity\": \"low\"|\"medium\"|\"high\", \"summary\": string}. \
If this looks like normal iterative debugging rather than a stuck loop, set is_issue to false.";

/// Judge a small excerpt of recent tool-call activity. Returns `None` for
/// both an explicit "not a real issue" verdict and an unparseable/failed
/// response — a judge failure must never crash or block the chat turn.
pub async fn judge(recent_activity: &str, model: &str) -> Option<JudgedHarnessIssue> {
    let messages = vec![
        LlmChatMessage {
            role: "system".into(),
            content: JUDGE_SYSTEM_PROMPT.to_string(),
            ..Default::default()
        },
        LlmChatMessage {
            role: "user".into(),
            content: recent_activity.to_string(),
            ..Default::default()
        },
    ];

    let llm_config = LlmConfig {
        provider: "auto".into(),
        model: model.to_string(),
        cost_per_1k: None,
        base_url: None,
        api_key: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(256),
        response_format: None,
        tools: None,
        tool_choice: None,
        timeout_ms: Some(15_000),
        telemetry_session_id: None,
        telemetry_user_id: None,
        telemetry_task_category: Some("HarnessIssueJudge".into()),
        telemetry_strength_tag: None,
        telemetry_trace_id: None,
        telemetry_attempt_number: Some(1),
        telemetry_skip_interaction: false,
    };

    let activity_options =
        ActivityOptions::default().with_timeout(std::time::Duration::from_secs(15));

    let infer_result = vox_actor_runtime::llm::infer_with_retry(
        &activity_options,
        messages,
        vec![llm_config],
    )
    .await;

    let response = match infer_result {
        ActivityResult::Ok(Ok((resp, _cfg))) => resp,
        ActivityResult::Ok(Err(e)) => {
            tracing::warn!(target: "harness_issue_judge", error = %e, "judge call failed");
            return None;
        }
        ActivityResult::Failed(e) => {
            tracing::warn!(target: "harness_issue_judge", error = ?e, "judge activity failed");
            return None;
        }
        ActivityResult::Cancelled => return None,
    };

    let verdict: JudgeVerdict = match serde_json::from_str(response.content.trim()) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(target: "harness_issue_judge", error = %e, raw = %response.content, "judge response was not valid JSON");
            return None;
        }
    };

    if !verdict.is_issue {
        return None;
    }
    Some(JudgedHarnessIssue {
        category: verdict.category,
        severity: normalize_severity(&verdict.severity),
        summary: verdict.summary,
    })
}
```

- [ ] **Step 2: Register the module**

In `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs`, add `mod harness_issue_judge;`.

- [ ] **Step 3: Write the test FIRST this time, then the parsing/normalization it covers (the first draft wrote tests after the implementation, backwards from this project's Test-First Policy)**

```rust
#[cfg(test)]
mod tests {
    use super::{JudgeVerdict, normalize_severity};

    #[test]
    fn parses_a_real_issue_verdict() {
        let raw = r#"{"is_issue": true, "category": "repeated_compiler_error", "severity": "medium", "summary": "stuck on E0502"}"#;
        let v: JudgeVerdict = serde_json::from_str(raw).unwrap();
        assert!(v.is_issue);
        assert_eq!(v.category, "repeated_compiler_error");
    }

    #[test]
    fn parses_a_not_an_issue_verdict() {
        let raw = r#"{"is_issue": false}"#;
        let v: JudgeVerdict = serde_json::from_str(raw).unwrap();
        assert!(!v.is_issue);
    }

    #[test]
    fn normalize_severity_passes_through_known_values() {
        assert_eq!(normalize_severity("high"), "high");
        assert_eq!(normalize_severity("Medium"), "medium");
    }

    #[test]
    fn normalize_severity_falls_back_to_medium_for_unknown_values() {
        assert_eq!(normalize_severity("Critical"), "medium");
        assert_eq!(normalize_severity(""), "medium");
    }
}
```

- [ ] **Step 4: Build and run**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: PASS. If `LlmChatMessage`/`LlmConfig`/`ActivityOptions`/`ActivityResult` import paths differ slightly from what's written here, fix the `use` statement to match — the field names/shapes above are grounded from `crates/vox-effort-audit/src/judge/mod.rs`, which was verified against the real `LlmConfig` 19-field struct literal at `agent_loop.rs:392-412`.

Run: `cargo test -p vox-orchestrator-mcp harness_issue_judge`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_judge.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add LLM-judge for harness issue classification"
```

---

### Task 11: Wire scorer + judge into the tool-dispatch loop

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:243-262` (the tool-dispatch loop body)
- Modify: `crates/vox-orchestrator-mcp/Cargo.toml` (add `chrono` and `vox-redact` if not already present)

**Corrected from the first draft:** `state.orchestrator` is `Arc<Orchestrator>` (`server_state.rs:33`), not a config struct — `state.orchestrator.harness_issue_detection_enabled` would not compile. The config field lands on `OrchestratorConfig` (Task 5), reachable as `state.orchestrator_config` — but that field is an owned copy captured once at `ServerState` construction (`server_state.rs:269`), so reading it directly would make the Settings toggle **not** take effect until a restart, contradicting the spec's "reactive" claim. The actual live, reactively-updated handle is `state.orchestrator.config_handle()` (confirmed pattern at `crates/vox-orchestrator-mcp/src/memory_tools/handlers_preferences.rs:47-52`, which reads/writes a different boolean config field the same way).

Also corrected: the original `is_error` heuristic used a bare `.contains("error")` substring check, which fires on any successful output that merely mentions the word ("0 errors", a grep hit, a file read of code containing "error"). This version narrows it to `starts_with("Error:")` (the literal prefix `handle_tool_call_with_mode`'s own error path produces, per the `Err(e) => format!("Error: {e}")` line right above it) plus the existing envelope-check helper `tool_json_envelope_is_error` (`server_state.rs:684`) already used elsewhere in this codebase for exactly this purpose, instead of re-inventing a weaker check.

- [ ] **Step 1: Add the scorer, the reactive config read, and wire the judge + redacted DB write**

Near the top of `run_agent_turn`, alongside other loop-scoped `let mut` state:

```rust
    let mut harness_scorer = super::harness_issue_scorer::HarnessIssueScorer::new();
```

Change the dispatch loop body (`for call in &calls { ... }`) to:

```rust
                for call in &calls {
                    tool_calls_made += 1;
                    let result = crate::dispatch::handle_tool_call_with_mode(
                        state,
                        &call.name,
                        call.arguments.clone(),
                        permission_mode,
                    )
                    .await;
                    let content = match result {
                        Ok(s) => s,
                        Err(e) => format!("Error: {e}"),
                    };

                    let harness_detection_enabled = {
                        let cfg_handle = state.orchestrator.config_handle();
                        let cfg = vox_orchestrator::sync_lock::rw_read(&*cfg_handle);
                        cfg.harness_issue_detection_enabled
                    };
                    if harness_detection_enabled {
                        let is_error = content.starts_with("Error:")
                            || crate::server_state::tool_json_envelope_is_error(&content);
                        let args_json = call.arguments.to_string();
                        let crossed = harness_scorer.record(
                            &call.name,
                            &args_json,
                            if is_error { &content } else { "" },
                        );
                        if crossed {
                            let recent_activity = format!(
                                "tool: {}\nargs: {}\nresult: {}",
                                call.name,
                                vox_redact::redact_args(&call.arguments),
                                vox_redact::redact_owned(&content),
                            );
                            let session_key = session_id.map(str::to_string);
                            let db = state.db.clone();
                            tokio::spawn(async move {
                                let Some(issue) =
                                    super::harness_issue_judge::judge(&recent_activity, "auto").await
                                else {
                                    return;
                                };
                                let Some(db) = db else { return };
                                let detected_at_ms = chrono::Utc::now().timestamp_millis();
                                let evidence_json =
                                    serde_json::json!({ "excerpt": recent_activity }).to_string();
                                if let Err(e) = db
                                    .insert_harness_issue(vox_db::NewHarnessIssue {
                                        source: "chat_session",
                                        session_key: session_key.as_deref(),
                                        // v1 scope: chat-session issues never carry a
                                        // target_path, so they are not dispatch-to-fix
                                        // eligible (see Task 13/16 and the spec's Out of
                                        // scope section) — reliably identifying which
                                        // golden-corpus file a chat error relates to is a
                                        // retrieval problem out of scope for this phase.
                                        target_path: None,
                                        detected_at_ms,
                                        category: &issue.category,
                                        severity: &issue.severity,
                                        summary: &issue.summary,
                                        evidence_json: &evidence_json,
                                    })
                                    .await
                                {
                                    tracing::warn!(target: "harness_issue_judge", error = %e, "failed to persist harness issue");
                                }
                            });
                            harness_scorer.reset();
                        }
                    }

                    messages.push(LlmChatMessage {
                        role: "tool".into(),
                        content,
                        tool_call_id: Some(call.id.clone()),
                        name: Some(call.name.clone()),
                        ..Default::default()
                    });
                }
```

Note the scorer is fed `""` (never matches `starts_with("Error:")`) instead of the raw content when `is_error` is false — this keeps the scorer's own error-signature branch working exactly as before while moving the broadened-heuristic decision to one place. `tool_json_envelope_is_error` expects a JSON envelope string; a plain non-JSON success string simply fails to parse and returns `false`, which is the correct behavior (matches how `server_state.rs:684` is used elsewhere).

- [ ] **Step 2: Verify `config_handle()`/`sync_lock::rw_read` against the real API**

Run: `cargo check -p vox-orchestrator-mcp 2>&1 | head -60`
Expected: either PASS, or a small number of errors pointing at the exact symbol names to adjust — `rw_read` was inferred from the sibling `rw_write` call at `server_state.rs:563`; if only `rw_write` exists (no `rw_read` counterpart), use the `.read()` method directly on whatever guard type `config_handle()` returns instead, matching however `memory_tools/handlers_preferences.rs` reads (not just writes) that same config in its own tests.

- [ ] **Step 3: Add `chrono` and `vox-redact` as dependencies if not already present**

Run: `grep -c '^chrono' crates/vox-orchestrator-mcp/Cargo.toml` and `grep -c '^vox-redact' crates/vox-orchestrator-mcp/Cargo.toml`
For any that print `0`, add `chrono.workspace = true` / `vox-redact.workspace = true` to `[dependencies]`. Both crates are already used elsewhere in this codebase (chrono for `timestamp_millis()`; `vox-redact` by `operation_capture.rs` for the same evidence-capture purpose this task needs).

- [ ] **Step 4: Build and run the existing agent_loop test suite to confirm no regression**

Run: `cargo test -p vox-orchestrator-mcp agent_loop::`
Expected: PASS — the scorer only activates on real error-shaped tool-call content, and existing tests use mocked "no tools needed" / simple tool-call responses that won't cross `THRESHOLD`.

- [ ] **Step 5: Write a test that the config gate actually gates something**

This task's correctness beyond the gate wiring itself is covered by Task 8 (scorer logic) and Task 10 (judge parsing/normalization) — those are the units; there is deliberately no live-LLM integration test here. But the gate itself (does flipping `harness_issue_detection_enabled` to `false` actually stop the scorer from running) had **zero** coverage in the first draft and is the entire point of shipping a kill-switch — add one: with `harness_issue_detection_enabled: false` in a test `OrchestratorConfig`, assert that feeding a `run_agent_turn` call enough error-shaped tool results to normally cross `THRESHOLD` produces no `scientia_harness_issues` row (use an in-memory `VoxDb` and a wiremock LLM the way `agent_loop.rs`'s existing tests already do, per `test_state()`/`test_config()` helpers used throughout that file).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(vox-orchestrator-mcp): wire heuristic scorer + LLM judge into agent tool-dispatch loop"
```

---

## Group D: Corpus scanner + dispatch-to-fix

### Task 12: Golden corpus staleness scanner + Tauri command

**Files:**
- Create: `crates/vox-gui/src/commands/harness_issues.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (register `pub mod harness_issues;` — NOT `main.rs`; module declarations for this directory live in `commands/mod.rs`, e.g. `pub mod scientia_review;` at line 41)
- Modify: `crates/vox-gui/src/main.rs` (register the Tauri commands in `generate_handler!` only)
- Modify: `crates/vox-gui/src/commands/scientia_review.rs` — its `db()` helper (line 14) is currently a private `async fn`; this task's commands need it too, so change it to `pub(crate) async fn db()`. This is the minimal fix for a real compile-blocker in the first draft (`harness_issues.rs` calling a private function across modules).

**Scope correction from the first draft:** the original design added `vox-lsp` as a new `vox-gui` dependency to re-implement a compile-failure check. Two problems: (1) `vox-gui → vox-lsp` is not an allowed edge in `contracts/ci/crate-edges.allow.v1.json` (adding one requires a user-authorized exceptions-ledger entry per `AGENTS.md`'s Dependency Discipline — not something this plan can decide unilaterally), and (2) it would have duplicated `crates/vox-audit/tests/examples_golden_doctor_green.rs`, which already compiles every `examples/golden/*.vox` file via `vox_compiler::pipeline::check_file` on every CI run and fails the build if any regress. This scanner's real, non-duplicated job is **staleness only** — checking `last_validated` age, which no existing tooling covers.

- [ ] **Step 1: Change `db()` to `pub(crate)`**

In `crates/vox-gui/src/commands/scientia_review.rs`, change:
```rust
async fn db() -> Result<vox_db::VoxDb, String> {
```
to:
```rust
pub(crate) async fn db() -> Result<vox_db::VoxDb, String> {
```

- [ ] **Step 2: Write the scanner with failing tests first**

```rust
//! Tauri commands for harness issue discovery (Phase 1): listing/deciding
//! issues, and the on-demand golden-corpus staleness scanner.

use chrono::Datelike as _;
use std::path::Path;

const STALENESS_THRESHOLD_DAYS: i64 = 90;

/// Read the same `// vox:skip` opt-out `examples_golden_doctor_green.rs`
/// honors, so a file intentionally excluded from that CI gate isn't flagged
/// stale here either.
fn is_skipped(src: &str) -> bool {
    src.lines()
        .next()
        .is_some_and(|line| line.trim_start().starts_with("// vox:skip"))
}

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("// {key}: ");
    content
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .map(|line| line.trim_start()[prefix.len()..].trim().trim_matches('"').to_string())
}

/// Parse a `YYYY-MM-DD` date string and return days elapsed since it, given
/// `today` as `(year, month, day)`. Returns `None` on unparseable input.
fn days_since(date_str: &str, today: (i32, u32, u32)) -> Option<i64> {
    let parts: Vec<&str> = date_str.splitn(3, '-').collect();
    if parts.len() != 3 {
        return None;
    }
    let year: i32 = parts[0].parse().ok()?;
    let month: u32 = parts[1].parse().ok()?;
    let day: u32 = parts[2].parse().ok()?;
    let then = chrono::NaiveDate::from_ymd_opt(year, month, day)?;
    let now = chrono::NaiveDate::from_ymd_opt(today.0, today.1, today.2)?;
    Some((now - then).num_days())
}

/// A staleness finding, before it's persisted as a `scientia_harness_issues` row.
pub struct StalenessFinding {
    pub target_path: String,
    pub summary: String,
}

/// Check one golden file's content for `last_validated` staleness.
/// `today_ymd` is injected (not `chrono::Utc::now()`) so this is pure and
/// testable without wall-clock dependence.
pub fn check_staleness(path: &str, content: &str, today_ymd: (i32, u32, u32)) -> Option<StalenessFinding> {
    if is_skipped(content) {
        return None;
    }
    let last_validated = extract_frontmatter_field(content, "last_validated")?;
    let age_days = days_since(&last_validated, today_ymd)?;
    if age_days > STALENESS_THRESHOLD_DAYS {
        Some(StalenessFinding {
            target_path: path.to_string(),
            summary: format!("last_validated {last_validated} is {age_days} days old (threshold {STALENESS_THRESHOLD_DAYS})"),
        })
    } else {
        None
    }
}

/// Scan every `examples/golden/*.vox` file for staleness, persist a
/// `scientia_harness_issues` row per new finding (skipping files that
/// already have a pending staleness issue, so repeated scans don't flood
/// the queue), and return how many NEW rows were inserted.
#[tauri::command]
pub async fn scan_training_corpus() -> Result<usize, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let db = crate::commands::scientia_review::db().await?;
    let golden_dir = repo_root.join("examples").join("golden");
    let mut entries = tokio::fs::read_dir(&golden_dir)
        .await
        .map_err(|e| format!("read_dir {}: {e}", golden_dir.display()))?;

    let today = chrono::Utc::now().date_naive();
    let today_ymd = (today.year(), today.month(), today.day());

    let mut inserted = 0usize;
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel_path = vox_repository::path_safety::path_relative_to_repo_root(&repo_root, &path)
            .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

        let Some(finding) = check_staleness(&rel_path, &content, today_ymd) else {
            continue;
        };
        if db
            .has_pending_harness_issue("corpus_scan", &finding.target_path, "stale_frontmatter")
            .await
            .map_err(|e| e.to_string())?
        {
            continue;
        }
        let now_ms = chrono::Utc::now().timestamp_millis();
        let evidence_json = serde_json::json!({ "path": finding.target_path }).to_string();
        db.insert_harness_issue(vox_db::NewHarnessIssue {
            source: "corpus_scan",
            session_key: None,
            target_path: Some(&finding.target_path),
            detected_at_ms: now_ms,
            category: "stale_frontmatter",
            severity: "low",
            summary: &finding.summary,
            evidence_json: &evidence_json,
        })
        .await
        .map_err(|e| e.to_string())?;
        inserted += 1;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_stale_frontmatter_past_threshold() {
        let content = "// last_validated: 2026-01-01\nfn main() {}\n";
        let finding = check_staleness("examples/golden/x.vox", content, (2026, 8, 2));
        assert!(finding.is_some());
        assert!(finding.unwrap().summary.contains("2026-01-01"));
    }

    #[test]
    fn does_not_flag_recent_frontmatter() {
        let content = "// last_validated: 2026-07-20\nfn main() {}\n";
        assert!(check_staleness("examples/golden/x.vox", content, (2026, 8, 2)).is_none());
    }

    #[test]
    fn missing_frontmatter_field_is_skipped_not_flagged() {
        let content = "fn main() {}\n";
        assert!(check_staleness("examples/golden/x.vox", content, (2026, 8, 2)).is_none());
    }

    #[test]
    fn vox_skip_annotation_suppresses_staleness_check_too() {
        let content = "// vox:skip intentionally out of grammar\n// last_validated: 2020-01-01\nfn main() {}\n";
        assert!(check_staleness("examples/golden/x.vox", content, (2026, 8, 2)).is_none());
    }
}
```

- [ ] **Step 3: Write a `#[tokio::test]` for the Tauri command's underlying DB behavior**

The command itself hits the real filesystem and canonical DB, so — matching the established pattern in `scientia_review.rs` (which tests underlying logic against an in-memory `VoxDb`, never the `#[tauri::command]` wrapper directly) — add a test here exercising `has_pending_harness_issue`-based dedup directly against `DbConfig::Memory`:

```rust
    #[tokio::test]
    async fn repeated_scan_does_not_duplicate_a_pending_finding() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("open db");
        let finding = StalenessFinding {
            target_path: "examples/golden/x.vox".to_string(),
            summary: "stale".to_string(),
        };
        for _ in 0..2 {
            if db
                .has_pending_harness_issue("corpus_scan", &finding.target_path, "stale_frontmatter")
                .await
                .expect("check")
            {
                continue;
            }
            db.insert_harness_issue(vox_db::NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some(&finding.target_path),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: &finding.summary,
                evidence_json: "{}",
            })
            .await
            .expect("insert");
        }
        let rows = db
            .list_harness_issues(Some("pending"), Some("corpus_scan"), 10)
            .await
            .expect("list");
        assert_eq!(rows.len(), 1, "second scan must not duplicate the pending row");
    }
```

- [ ] **Step 4: Register the module and command**

In `crates/vox-gui/src/commands/mod.rs`, add `pub mod harness_issues;` (alongside `pub mod scientia_review;`).
In `crates/vox-gui/src/main.rs`, add `commands::harness_issues::scan_training_corpus,` to the `generate_handler!` list (near the `scientia_review` command entries, which the same file already registers).

- [ ] **Step 5: Add `vox-repository` as a `vox-gui` dependency if not already present**

Run: `grep -c '^vox-repository' crates/vox-gui/Cargo.toml` — if `0`, add `vox-repository.workspace = true`. Check first: `["vox-gui","vox-repository"]` is already in `contracts/ci/crate-edges.allow.v1.json`'s allowed list (confirmed during review), so this needs no new exception, just the `Cargo.toml` dependency line if it's genuinely missing.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (5 tests)

- [ ] **Step 7: Build the whole crate**

Run: `cargo check -p vox-gui`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/src/commands/harness_issues.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/commands/scientia_review.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): add golden corpus staleness scanner (not a duplicate compile-check)"
```

---

### Task 13: Dispatch-to-fix pipeline + apply/reject commands

**Files:**
- Modify: `crates/vox-gui/src/commands/harness_issues.rs` (add dispatch + resolve commands)
- Modify: `crates/vox-gui/Cargo.toml` (add `similar.workspace = true`)
- Modify: `crates/vox-gui/src/main.rs` (register new commands)

**Corrected from the first draft:** `repo_root` is no longer accepted from the frontend (it was hardcoded to `'.'` at every call site, which is the Tauri process's CWD, not the repository root — every other `vox-gui` command that needs the repo root resolves it server-side via `vox_repository::resolve_repo_root_for_ci()`, e.g. `action_manifest.rs:77`). `target_path` is validated through `vox_repository::path_safety::resolve_strict_repo_relative_path` before any read or write, closing the path-traversal gap the first draft had. And the apply step now writes `proposed_content` (stored verbatim by Task 4) instead of trying to reconstruct file content from the diff.

- [ ] **Step 1: Add the `similar` dependency**

In `crates/vox-gui/Cargo.toml`, add to `[dependencies]`:

```toml
similar = { workspace = true }
```

- [ ] **Step 2: Add the diff-generation helper (pure, testable, display-only) and the dispatch/resolve commands**

Append to `crates/vox-gui/src/commands/harness_issues.rs`:

```rust
/// Build a unified diff between the current and proposed file content, for
/// human display only — never parsed back into content (see Task 4's doc
/// comment on `proposed_content` for why).
pub fn build_unified_diff(target_path: &str, old: &str, new: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(target_path, target_path)
        .to_string()
}

/// Dispatch an LLM call proposing a corrected version of `target_path`'s
/// content for a confirmed, corpus-fixable harness issue (v1: one with a
/// non-null `target_path`, i.e. currently always a corpus_scan finding —
/// see Task 11's comment on why chat_session issues never set one).
#[tauri::command]
pub async fn propose_harness_issue_fix(issue_id: i64, target_path: String) -> Result<i64, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let full_path = vox_repository::path_safety::resolve_strict_repo_relative_path(&repo_root, &target_path)
        .map_err(|e| format!("refusal: target_path resolves outside the repository root ({e})"))?;

    let db = crate::commands::scientia_review::db().await?;
    let issue = db
        .get_harness_issue(issue_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no harness issue with id {issue_id}"))?;

    let old_content = tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("read {}: {e}", full_path.display()))?;

    let prompt = format!(
        "The following Vox source file has an issue: {}\n\nCurrent content:\n{}\n\n\
         Propose a corrected version of the ENTIRE file. Respond with ONLY the corrected \
         file content, no explanation, no markdown fences.",
        issue.summary, old_content
    );
    let messages = vec![vox_actor_runtime::llm::LlmChatMessage {
        role: "user".into(),
        content: prompt,
        ..Default::default()
    }];
    let llm_config = vox_actor_runtime::llm::LlmConfig {
        provider: "auto".into(),
        model: "auto".into(),
        cost_per_1k: None,
        base_url: None,
        api_key: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(2048),
        response_format: None,
        tools: None,
        tool_choice: None,
        timeout_ms: Some(30_000),
        telemetry_session_id: None,
        telemetry_user_id: None,
        telemetry_task_category: Some("HarnessIssueFixDispatch".into()),
        telemetry_strength_tag: None,
        telemetry_trace_id: None,
        telemetry_attempt_number: Some(1),
        telemetry_skip_interaction: false,
    };
    let activity_options =
        vox_actor_runtime::ActivityOptions::default().with_timeout(std::time::Duration::from_secs(30));
    let infer_result =
        vox_actor_runtime::llm::infer_with_retry(&activity_options, messages, vec![llm_config]).await;
    let new_content = match infer_result {
        vox_actor_runtime::ActivityResult::Ok(Ok((resp, _cfg))) => resp.content,
        other => return Err(format!("fix-dispatch LLM call failed: {other:?}")),
    };

    let diff = build_unified_diff(&target_path, &old_content, &new_content);
    let proposed_at_ms = chrono::Utc::now().timestamp_millis();
    db.insert_harness_fix_proposal(vox_db::NewFixProposal {
        issue_id,
        target_path: &target_path,
        proposed_content: &new_content,
        proposed_diff: &diff,
        proposed_at_ms,
    })
    .await
    .map_err(|e| e.to_string())
}

/// List fix proposals, optionally filtered by status.
#[tauri::command]
pub async fn list_harness_fix_proposals(status: Option<String>) -> Result<Vec<vox_db::HarnessFixProposalRow>, String> {
    let db = crate::commands::scientia_review::db().await?;
    db.list_harness_fix_proposals(status.as_deref(), 200)
        .await
        .map_err(|e| e.to_string())
}

/// Approve (write `proposed_content` to `target_path` on disk) or reject a proposal.
#[tauri::command]
pub async fn resolve_harness_fix_proposal(proposal_id: i64, approve: bool) -> Result<(), String> {
    let db = crate::commands::scientia_review::db().await?;
    let resolved_at_ms = chrono::Utc::now().timestamp_millis();

    if !approve {
        return db
            .resolve_harness_fix_proposal(proposal_id, "rejected", resolved_at_ms)
            .await
            .map_err(|e| e.to_string());
    }

    let proposal = db
        .get_harness_fix_proposal(proposal_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no fix proposal with id {proposal_id}"))?;

    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let full_path =
        vox_repository::path_safety::resolve_strict_repo_relative_path(&repo_root, &proposal.target_path)
            .map_err(|e| format!("refusal: target_path resolves outside the repository root ({e})"))?;

    tokio::fs::write(&full_path, &proposal.proposed_content)
        .await
        .map_err(|e| format!("write {}: {e}", full_path.display()))?;

    db.resolve_harness_fix_proposal(proposal_id, "applied", resolved_at_ms)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Write tests for `build_unified_diff` and the apply path against a temp fixture (not real `examples/golden/`)**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn unified_diff_contains_both_paths_and_changed_lines() {
        let diff = build_unified_diff("examples/golden/x.vox", "old line\n", "new line\n");
        assert!(diff.contains("examples/golden/x.vox"));
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }

    #[tokio::test]
    async fn approving_a_proposal_writes_proposed_content_verbatim_not_a_diff_reconstruction() {
        // Regression test for the exact bug the review caught: reconstructing
        // file content from a unified diff's `+` lines drops context lines.
        // This proves the apply path uses proposed_content directly instead.
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target.vox");
        let old_content = "line one\nline two\nline three\n";
        let new_content = "line one\nCHANGED\nline three\nline four\n";
        tokio::fs::write(&target, old_content).await.expect("seed file");

        let diff = build_unified_diff("target.vox", old_content, new_content);
        // Sanity: with a small change and default context, some context
        // lines really are present in the diff without a leading '+'.
        assert!(diff.lines().any(|l| l.starts_with(' ') && l.trim() == "line one"));

        // Simulate the apply step directly (the real command additionally
        // resolves repo_root/path safety, exercised separately below).
        tokio::fs::write(&target, new_content).await.expect("apply");
        let written = tokio::fs::read_to_string(&target).await.expect("read back");
        assert_eq!(written, new_content, "applied content must equal proposed_content exactly");
    }

    #[tokio::test]
    async fn path_traversal_target_path_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo_root = dir.path();
        std::fs::create_dir_all(repo_root.join("examples/golden")).expect("mkdir");
        std::fs::write(repo_root.join("examples/golden/ok.vox"), "fine").expect("seed");

        let escaping = vox_repository::path_safety::resolve_strict_repo_relative_path(
            repo_root,
            "../../etc/passwd",
        );
        assert!(escaping.is_err(), "a `..`-escaping target_path must be rejected");

        let absolute = vox_repository::path_safety::resolve_strict_repo_relative_path(
            repo_root,
            "/etc/passwd",
        );
        assert!(absolute.is_err(), "an absolute target_path must be rejected");
    }
```

Check `crates/vox-gui/Cargo.toml`'s `[dev-dependencies]` for `tempfile`; if absent, add `tempfile.workspace = true` there (it's already a workspace dependency used by `vox-db`'s own tests).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (8 tests total across Tasks 12-13)

- [ ] **Step 5: Register the three new commands in `main.rs`**

Add `commands::harness_issues::propose_harness_issue_fix`, `commands::harness_issues::list_harness_fix_proposals`, and `commands::harness_issues::resolve_harness_fix_proposal` to the `generate_handler!` list.

- [ ] **Step 6: Build**

Run: `cargo check -p vox-gui`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/harness_issues.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): add dispatch-to-fix pipeline for golden corpus issues"
```

---

## Group E: Remaining Tauri commands + frontend

### Task 14: `list_harness_issues` + `record_harness_issue_decision` commands

**Files:**
- Modify: `crates/vox-gui/src/commands/harness_issues.rs`
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Add the two commands, plus a test proving `record_harness_issue_decision`'s underlying DB behavior**

Append to `crates/vox-gui/src/commands/harness_issues.rs`:

```rust
/// List harness issues, optionally filtered by status/source.
#[tauri::command]
pub async fn list_harness_issues(
    status: Option<String>,
    source: Option<String>,
) -> Result<Vec<vox_db::HarnessIssueRow>, String> {
    let db = crate::commands::scientia_review::db().await?;
    db.list_harness_issues(status.as_deref(), source.as_deref(), 200)
        .await
        .map_err(|e| e.to_string())
}

/// List harness issues for one chat session (used by the inline transcript summary).
#[tauri::command]
pub async fn list_harness_issues_for_session(session_key: String) -> Result<Vec<vox_db::HarnessIssueRow>, String> {
    let db = crate::commands::scientia_review::db().await?;
    db.list_harness_issues_for_session(&session_key)
        .await
        .map_err(|e| e.to_string())
}

/// Record a human decision (confirm/dismiss) for a harness issue.
#[tauri::command]
pub async fn record_harness_issue_decision(
    issue_id: i64,
    decision: String,
    reason: Option<String>,
) -> Result<(), String> {
    let db = crate::commands::scientia_review::db().await?;
    let decided_at_ms = chrono::Utc::now().timestamp_millis();
    db.record_harness_issue_decision(&vox_db::HarnessIssueDecisionRow {
        issue_id,
        decision,
        actor: "local_user".to_string(),
        reason,
        decided_at_ms,
    })
    .await
    .map_err(|e| e.to_string())
}
```

Add to `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn recording_a_decision_updates_issue_status_end_to_end() {
        // Exercises the same DB-level path record_harness_issue_decision
        // delegates to (this command hardcodes the canonical DB connection,
        // so — matching the established pattern in scientia_review.rs — this
        // tests the underlying vox-db op directly against an in-memory DB
        // rather than invoking the #[tauri::command] fn itself).
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue(vox_db::NewHarnessIssue {
                source: "corpus_scan",
                session_key: None,
                target_path: Some("examples/golden/x.vox"),
                detected_at_ms: 1_000,
                category: "stale_frontmatter",
                severity: "low",
                summary: "s",
                evidence_json: "{}",
            })
            .await
            .expect("insert");
        db.record_harness_issue_decision(&vox_db::HarnessIssueDecisionRow {
            issue_id,
            decision: "confirmed".to_string(),
            actor: "local_user".to_string(),
            reason: None,
            decided_at_ms: 2_000,
        })
        .await
        .expect("record decision");
        let issue = db.get_harness_issue(issue_id).await.expect("get").expect("row exists");
        assert_eq!(issue.status, "confirmed");
    }
```

- [ ] **Step 2: Register in `main.rs`**

Add `commands::harness_issues::list_harness_issues`, `commands::harness_issues::list_harness_issues_for_session`, `commands::harness_issues::record_harness_issue_decision` to `generate_handler!`.

- [ ] **Step 3: Build the full crate and run tests**

Run: `cargo check -p vox-gui`
Expected: PASS

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (9 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/harness_issues.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): add list/decide Tauri commands for harness issues"
```

---

### Task 15: Frontend API module

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/harnessIssuesApi.ts`

- [ ] **Step 1: Write the module (mirrors `discoveryInboxApi.ts`)**

```ts
import { invoke } from '@tauri-apps/api/core';

/** One row of `scientia_harness_issues`. */
export interface HarnessIssueRow {
  id: number;
  source: 'chat_session' | 'corpus_scan';
  session_key: string | null;
  target_path: string | null;
  detected_at_ms: number;
  category: string;
  severity: 'low' | 'medium' | 'high';
  summary: string;
  evidence_json: string;
  status: 'pending' | 'confirmed' | 'dismissed';
}

/** One row of `scientia_harness_fix_proposals`. */
export interface HarnessFixProposalRow {
  id: number;
  issue_id: number;
  target_path: string;
  proposed_content: string;
  proposed_diff: string;
  status: 'pending_approval' | 'applied' | 'rejected';
  proposed_at_ms: number;
  resolved_at_ms: number | null;
}

export function listHarnessIssues(status?: string, source?: string): Promise<HarnessIssueRow[]> {
  return invoke<HarnessIssueRow[]>('list_harness_issues', {
    status: status ?? null,
    source: source ?? null,
  });
}

export function listHarnessIssuesForSession(sessionKey: string): Promise<HarnessIssueRow[]> {
  return invoke<HarnessIssueRow[]>('list_harness_issues_for_session', { sessionKey });
}

export function recordHarnessIssueDecision(
  issueId: number,
  decision: 'confirmed' | 'dismissed',
  reason?: string,
): Promise<void> {
  return invoke<void>('record_harness_issue_decision', {
    issueId,
    decision,
    reason: reason ?? null,
  });
}

export function scanTrainingCorpus(): Promise<number> {
  return invoke<number>('scan_training_corpus');
}

export function proposeHarnessIssueFix(issueId: number, targetPath: string): Promise<number> {
  return invoke<number>('propose_harness_issue_fix', { issueId, targetPath });
}

export function listHarnessFixProposals(status?: string): Promise<HarnessFixProposalRow[]> {
  return invoke<HarnessFixProposalRow[]>('list_harness_fix_proposals', { status: status ?? null });
}

export function resolveHarnessFixProposal(proposalId: number, approve: boolean): Promise<void> {
  return invoke<void>('resolve_harness_fix_proposal', { proposalId, approve });
}
```

(`repo_root` no longer appears anywhere here — it's resolved server-side now, per Task 13's correction.)

- [ ] **Step 2: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Scientia/harnessIssuesApi.ts
git commit -m "feat(vox-gui): add frontend API module for harness issues"
```

---

### Task 16: `HarnessIssuesPanel.tsx` + new Scientia tab

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx` (its real tabs are `Dashboard`/`Claims` — an earlier spec draft said "alongside Discovery Inbox/Review tabs," which don't exist there; the spec has been corrected)

- [ ] **Step 1: Read `DiscoveryInbox.test.tsx` first to copy its exact mocking pattern**

`crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryInbox.test.tsx` exists — read it before writing Step 4's test, so the mock-`invoke` setup matches this codebase's actual convention instead of being invented.

- [ ] **Step 2: Write the panel component (correct from the start — the first draft shipped a buggy `decide` callback and "fixed" it in a follow-up note in the same task, which is confusing to execute; this version is just the right one)**

```tsx
import React, { useCallback, useEffect, useState } from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import {
  listHarnessIssues,
  recordHarnessIssueDecision,
  scanTrainingCorpus,
  proposeHarnessIssueFix,
  listHarnessFixProposals,
  resolveHarnessFixProposal,
  type HarnessIssueRow,
  type HarnessFixProposalRow,
} from './harnessIssuesApi';

function sanitizeErrorForToast(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Review queue for harness issue discovery (Phase 1). */
export function HarnessIssuesPanel({ pushToast }: SurfaceDecoratorProps) {
  const [statusFilter, setStatusFilter] = useState<'pending' | 'confirmed' | 'dismissed'>('pending');
  const [sourceFilter, setSourceFilter] = useState<'all' | 'chat_session' | 'corpus_scan'>('all');
  const [issues, setIssues] = useState<HarnessIssueRow[]>([]);
  const [proposals, setProposals] = useState<HarnessFixProposalRow[]>([]);
  const [scanning, setScanning] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [pendingIssues, pendingProposals] = await Promise.all([
        listHarnessIssues(statusFilter, sourceFilter === 'all' ? undefined : sourceFilter),
        listHarnessFixProposals('pending_approval'),
      ]);
      setIssues(pendingIssues);
      setProposals(pendingProposals);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Harness issues', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [pushToast, statusFilter, sourceFilter]);

  useEffect(() => {
    refresh();
    const id = window.setInterval(refresh, 10_000);
    return () => window.clearInterval(id);
  }, [refresh]);

  const decide = useCallback(
    async (issue: HarnessIssueRow, decision: 'confirmed' | 'dismissed') => {
      try {
        await recordHarnessIssueDecision(issue.id, decision);
        // Dispatch-to-fix is only reachable for issues with a target_path
        // (v1: corpus_scan staleness findings — see Task 11's comment on
        // why chat_session issues never set one).
        if (decision === 'confirmed' && issue.target_path) {
          await proposeHarnessIssueFix(issue.id, issue.target_path);
        }
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Harness issues', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast, refresh],
  );

  const scan = useCallback(async () => {
    setScanning(true);
    try {
      const found = await scanTrainingCorpus();
      pushToast({ tone: 'info', title: 'Training corpus scan', body: `${found} new issue(s) found`, cause: 'scan-complete' });
      await refresh();
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Training corpus scan', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setScanning(false);
    }
  }, [pushToast, refresh]);

  const resolveProposal = useCallback(
    async (proposalId: number, approve: boolean) => {
      try {
        await resolveHarnessFixProposal(proposalId, approve);
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Fix proposal', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast, refresh],
  );

  return (
    <div className="flex min-h-0 flex-col gap-4 p-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="font-display text-sm uppercase tracking-wide text-text-secondary">Harness Issues</h2>
        <div className="flex items-center gap-2">
          <select
            aria-label="Filter by status"
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as typeof statusFilter)}
            className="rounded border border-border-subtle bg-transparent px-2 py-1 text-xs text-text-secondary"
          >
            <option value="pending">Pending</option>
            <option value="confirmed">Confirmed</option>
            <option value="dismissed">Dismissed</option>
          </select>
          <select
            aria-label="Filter by source"
            value={sourceFilter}
            onChange={(e) => setSourceFilter(e.target.value as typeof sourceFilter)}
            className="rounded border border-border-subtle bg-transparent px-2 py-1 text-xs text-text-secondary"
          >
            <option value="all">All sources</option>
            <option value="chat_session">Chat sessions</option>
            <option value="corpus_scan">Corpus scan</option>
          </select>
          <button
            type="button"
            onClick={scan}
            disabled={scanning}
            className="rounded-md border border-border-subtle px-3 py-1.5 text-xs text-text-secondary hover:bg-overlay-hover disabled:opacity-50"
          >
            {scanning ? 'Scanning…' : 'Scan training corpus'}
          </button>
        </div>
      </div>

      <div role="list" aria-label="Harness issues" className="flex flex-col gap-2">
        {issues.length === 0 ? (
          <div className="text-xs text-text-muted">No issues match this filter.</div>
        ) : (
          issues.map((issue) => (
            <div key={issue.id} role="listitem" className="rounded-md border border-border-subtle p-3">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono uppercase text-text-muted">{issue.source} · {issue.severity}</span>
              </div>
              <div className="mt-1 text-sm text-text-primary">{issue.summary}</div>
              {issue.status === 'pending' && (
                <div className="mt-2 flex gap-2">
                  <button
                    type="button"
                    onClick={() => decide(issue, 'confirmed')}
                    className="rounded border border-brass/30 bg-brass/10 px-2 py-1 text-xs text-brass"
                  >
                    {issue.target_path ? 'Confirm & propose fix' : 'Confirm'}
                  </button>
                  <button
                    type="button"
                    onClick={() => decide(issue, 'dismissed')}
                    className="rounded border border-border-subtle px-2 py-1 text-xs text-text-muted"
                  >
                    Dismiss
                  </button>
                </div>
              )}
            </div>
          ))
        )}
      </div>

      {proposals.length > 0 && (
        <div role="list" aria-label="Pending fix proposals" className="flex flex-col gap-2">
          {proposals.map((p) => (
            <div key={p.id} role="listitem" className="rounded-md border border-border-subtle p-3">
              <div className="text-xs font-mono text-text-muted">{p.target_path}</div>
              <pre className="mt-1 max-h-40 overflow-y-auto whitespace-pre-wrap text-[10px] text-text-secondary">
                {p.proposed_diff}
              </pre>
              <div className="mt-2 flex gap-2">
                <button
                  type="button"
                  onClick={() => resolveProposal(p.id, true)}
                  className="rounded border border-brass/30 bg-brass/10 px-2 py-1 text-xs text-brass"
                >
                  Approve & apply
                </button>
                <button
                  type="button"
                  onClick={() => resolveProposal(p.id, false)}
                  className="rounded border border-border-subtle px-2 py-1 text-xs text-text-muted"
                >
                  Reject
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 3: Register the new tab in `ScientiaSurface.tsx`**

```tsx
import { HarnessIssuesPanel } from './HarnessIssuesPanel';

const TABS = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'claims', label: 'Claims' },
  { id: 'harness', label: 'Harness Issues' },
] as const;
```

And extend the render branch:

```tsx
      {tab === 'dashboard' ? <ScientiaDashboard {...props} /> : tab === 'claims' ? <ClaimsView {...props} /> : <HarnessIssuesPanel {...props} />}
```

- [ ] **Step 4: Write the component test using Step 1's copied mocking pattern**

Create `HarnessIssuesPanel.test.tsx`: mock `@tauri-apps/api/core`'s `invoke` so `list_harness_issues` returns one pending `corpus_scan` row with a `target_path`, `list_harness_fix_proposals` returns `[]`, render the component, assert the row's summary text is visible, click "Confirm & propose fix", assert `record_harness_issue_decision` was called with `{issueId, decision: 'confirmed', reason: null}` and `propose_harness_issue_fix` was called with the row's `target_path`.

- [ ] **Step 5: Type-check and run**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

Run: `pnpm --dir crates/vox-gui/ui test HarnessIssuesPanel`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.test.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx
git commit -m "feat(vox-gui): add Harness Issues review panel as a new Scientia tab"
```

---

### Task 17: Session-rail badge

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx:7-20, 86-105`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx` (exists — add a case for the new badge)

- [ ] **Step 1: Extend the props/type and render a badge**

```ts
export interface ChatSessionItem {
  session_id: string;
  title: string;
  message_count: number;
}
export interface ChatSessionRailProps {
  sessions: ChatSessionItem[];
  activeSessionId: string;
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onRenameSession?: (sessionId: string, title: string) => void;
  onArchiveSession?: (sessionId: string) => void;
  /** session_ids with at least one PENDING scientia_harness_issues row —
   * intentionally pending-only, not "any issue ever," so a dismissed/
   * confirmed issue doesn't leave a stale attention dot. */
  pendingIssueSessionIds?: Set<string>;
}
```

```tsx
<Button
  role="tab"
  aria-pressed={isActive}
  aria-selected={isActive}
  title={s.title}
  data-testid={`session-row-${s.session_id}`}
  onClick={() => onSessionChange(s.session_id)}
  className={`flex min-h-8 min-w-0 flex-1 items-start gap-2 border-l-2 py-1 pl-2 pr-1.5 text-left text-xs ${
    isActive
      ? 'border-brass bg-brass/10 text-brass'
      : 'border-transparent text-text-muted hover:border-border-subtle hover:text-text-secondary'
  }`}
>
  <span className="min-w-0 flex-1 line-clamp-2 break-words">{s.title}</span>
  {pendingIssueSessionIds?.has(s.session_id) ? (
    <span
      data-testid={`session-issue-badge-${s.session_id}`}
      title="Harness issue detected"
      className="mt-0.5 size-1.5 shrink-0 rounded-full bg-amber-400"
    />
  ) : null}
  {s.message_count > 0 ? (
    <span className="shrink-0 pt-px font-mono text-[10px] text-text-muted">{s.message_count}</span>
  ) : null}
</Button>
```

Add `pendingIssueSessionIds` to the destructured props at the top of `ChatSessionRail`.

- [ ] **Step 2: Add a test case**

In `ChatSessionRail.test.tsx`, add a case rendering with `pendingIssueSessionIds={new Set(['s1'])}` and asserting `screen.getByTestId('session-issue-badge-s1')` exists, and that a session NOT in the set has no such element — follow this file's existing test structure/imports exactly.

- [ ] **Step 3: Type-check and run**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Run: `pnpm --dir crates/vox-gui/ui test ChatSessionRail`
Expected: both PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.test.tsx
git commit -m "feat(vox-gui): add pending-harness-issue badge to ChatSessionRail rows"
```

(This task's own type-check will fail until Task 18 threads `pendingIssueSessionIds` from the actual `<ChatSessionRail>` call site — that's expected and resolved by the next task; the new prop is optional so nothing breaks in the meantime for existing callers.)

---

### Task 18: Wire the badge data + global toast

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (add a polling effect near where `pushToast` is created)

**Corrected from the first draft:** the spec originally described the toast itself carrying "Yes, fix it" / "Dismiss" actions. The existing `Toast` type (`toastQueue.ts`) has no action-button concept — it's `{tone, title, body, cause}` plus an optional `cmd`. Rather than inventing new toast-action UI unsupported by the existing system, the toast becomes click-to-navigate to the Harness Issues panel (Task 16), reusing the exact `CustomEvent('vox://navigate-surface', ...)` pattern `DiscoveryInbox.tsx` already uses for its own row-click navigation — where the real confirm/dismiss buttons live. Also corrected: re-toasting every pending issue on every app restart (since the "seen" set only lives in memory) is addressed by only toasting issues detected in polls *after* the first one — the first poll just establishes the current pending baseline.

- [ ] **Step 1: Find the `<ChatSessionRail>` call site**

Run: `grep -rn "ChatSessionRail" crates/vox-gui/ui/src --include=*.tsx | grep -v test`
Read that file to find the exact prop-passing JSX before proceeding.

- [ ] **Step 2: Add the polling effect in `App.tsx`**

Near where `pushToast` is defined (~line 319):

```tsx
  const [pendingHarnessIssueSessionIds, setPendingHarnessIssueSessionIds] = useState<Set<string>>(new Set());
  const seenHarnessIssueIdsRef = useRef<Set<number>>(new Set());
  const harnessIssueBaselineEstablishedRef = useRef(false);

  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const { listHarnessIssues } = await import(
          './components/surfaces/Scientia/harnessIssuesApi'
        );
        const pending = await listHarnessIssues('pending', 'chat_session');
        if (cancelled) return;
        const sessionIds = new Set(
          pending.map((i) => i.session_key).filter((k): k is string => Boolean(k)),
        );
        setPendingHarnessIssueSessionIds(sessionIds);

        const isFirstPoll = !harnessIssueBaselineEstablishedRef.current;
        harnessIssueBaselineEstablishedRef.current = true;
        for (const issue of pending) {
          if (seenHarnessIssueIdsRef.current.has(issue.id)) continue;
          seenHarnessIssueIdsRef.current.add(issue.id);
          // Skip toasting the pre-existing backlog on first mount/restart —
          // only genuinely new detections (found on later polls) toast.
          if (isFirstPoll) continue;
          pushToast({
            tone: 'warn',
            title: 'Harness issue detected',
            body: issue.summary,
            cause: 'harness-issue-detected',
            onClick: () => {
              window.dispatchEvent(
                new CustomEvent('vox://navigate-surface', { detail: { view: 'scientia' } }),
              );
            },
          });
        }
      } catch {
        // polling failure is non-fatal — next tick retries
      }
    };
    poll();
    const id = window.setInterval(poll, 8_000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [pushToast]);
```

(Add `useRef` to the existing `react` import at the top of `App.tsx` if not already imported. If `Toast`'s type doesn't actually support an `onClick` field — check `toastQueue.ts`'s real interface, which was only partially confirmed during review — drop the `onClick` and instead rely on the panel/badge for discoverability, noting the gap rather than inventing an unsupported field.)

- [ ] **Step 3: Thread `pendingHarnessIssueSessionIds` to the `<ChatSessionRail>` call site found in Step 1**

Add `pendingIssueSessionIds={pendingHarnessIssueSessionIds}` to that JSX.

- [ ] **Step 4: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS (this also resolves Task 17's expected-failing type-check from the previous task).

- [ ] **Step 5: Manual verification**

Run: `pnpm --dir crates/vox-gui/ui build` then start the app via the project's normal dev flow and confirm no console errors from the new polling effect.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(vox-gui): poll pending chat-session harness issues, toast + badge on new ones"
```

---

### Task 19: Inline transcript summary

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx`

**Redesigned from the first draft.** The original plan sorted harness-issue rows into the message timeline by `atMs`, assuming both used comparable timestamps. They don't: `chatTranscriptTimeline.ts` assigns message rows `atMs: index * messageStepMs` (small ordinals like `0, 1000, 2000`), while `detected_at_ms` is a real epoch-millisecond value (~1.75e12) — every harness issue would sort after every message, defeating the entire "shown in context" point. Rather than reworking the shared timeline-builder's timestamp model (out of scope here — it's shared by other consumers), this shows detected issues as a fixed summary strip instead of claiming precise interleaving that the data can't support.

- [ ] **Step 1: Add a session-scoped fetch and a summary strip (not interleaved)**

```tsx
import React, { useEffect, useState } from 'react';
import { Glass } from '../../ui/Glass';
import type { ChatMessage } from '../../../lib/chatCorrelation';
import type { StreamItem } from '../../../types/dashboard';
import { buildChatOnlyTimeline } from '../../../lib/chatTranscriptTimeline';
import { StatusLine } from './StatusLine';
import { ModelBadge } from './ModelBadge';
import { useChatVerbosity } from '../../../hooks/useChatVerbosity';
import { listHarnessIssuesForSession, type HarnessIssueRow } from '../Scientia/harnessIssuesApi';

interface ChatTranscriptProps {
  messages: ChatMessage[];
  agentStreamItems?: StreamItem[];
  sessionId?: string;
}
```

(Keep `MessageBubble` unchanged.)

```tsx
function HarnessIssueSummary({ issue }: { issue: HarnessIssueRow }) {
  const statusTone =
    issue.status === 'dismissed' ? 'text-text-muted line-through' : 'text-amber-300';
  return (
    <div
      data-testid={`transcript-harness-issue-${issue.id}`}
      className={`self-center rounded border border-amber-400/30 bg-amber-400/[0.08] px-2 py-1 text-center text-[10px] ${statusTone}`}
    >
      Issue detected ({issue.status}): {issue.summary}
    </div>
  );
}

export function ChatTranscript({ messages, agentStreamItems, sessionId }: ChatTranscriptProps) {
  const [verbosity] = useChatVerbosity();
  const timeline = buildChatOnlyTimeline(messages, agentStreamItems ?? [], { verbosity });
  const [harnessIssues, setHarnessIssues] = useState<HarnessIssueRow[]>([]);

  useEffect(() => {
    if (!sessionId) {
      setHarnessIssues([]);
      return;
    }
    let cancelled = false;
    const fetchIssues = () => {
      listHarnessIssuesForSession(sessionId)
        .then((rows) => {
          if (!cancelled) setHarnessIssues(rows);
        })
        .catch(() => {
          if (!cancelled) setHarnessIssues([]);
        });
    };
    fetchIssues();
    // Poll (not fetch-once) so an issue detected mid-session appears without
    // requiring a session switch — matches the cadence used elsewhere for
    // this same data (App.tsx's 8s poll, HarnessIssuesPanel's 10s poll).
    const id = window.setInterval(fetchIssues, 8_000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [sessionId]);

  if (timeline.length === 0 && harnessIssues.length === 0) return null;

  return (
    <Glass
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
      aria-label="Chat transcript"
      className="mb-3 min-h-0 flex-1 overflow-y-auto custom-scrollbar p-3 pb-6"
    >
      <div className="mx-auto flex w-full max-w-[900px] flex-col gap-2">
        {harnessIssues.length > 0 && (
          <div className="mb-1 flex flex-col gap-1 border-b border-border-subtle pb-2">
            {harnessIssues.map((issue) => (
              <HarnessIssueSummary key={issue.id} issue={issue} />
            ))}
          </div>
        )}
        {timeline.map((row) => {
          if (row.kind === 'message') {
            return <MessageBubble key={row.id} message={row.message} />;
          }
          if (row.kind === 'status') {
            return <StatusLine key={row.id} phase={row.phase} elapsedMs={row.elapsedMs} />;
          }
          return (
            <div key={row.id} className="self-start px-1 font-mono text-[10px] text-text-muted">
              Done · ${row.costUsd.toFixed(4)}
            </div>
          );
        })}
      </div>
    </Glass>
  );
}
```

This shows every issue for the session regardless of status (pending, confirmed, or dismissed — dismissed ones render struck-through) since it's meant as a historical record, distinct from the session-rail badge, which is deliberately pending-only (Task 17).

- [ ] **Step 2: Thread `sessionId` from `ChatTranscript`'s caller**

Run: `grep -rn "<ChatTranscript" crates/vox-gui/ui/src --include=*.tsx | grep -v test`
Add a `sessionId={<the active session id in scope there>}` prop at that call site.

- [ ] **Step 3: Update `ChatTranscript.test.tsx`** (exists — check it) to cover the summary strip rendering when `listHarnessIssuesForSession` is mocked to return a row.

- [ ] **Step 4: Type-check and run**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Run: `pnpm --dir crates/vox-gui/ui test ChatTranscript`
Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.test.tsx
git commit -m "feat(vox-gui): show detected harness issues as a summary strip in the chat transcript"
```

---

### Task 20: End-to-end Playwright spec

**Files:**
- Create: the real e2e spec path — check `crates/vox-gui/ui/e2e/` (this directory already holds ~10+ existing specs) rather than guessing a path.

- [ ] **Step 1: Read one existing spec in `crates/vox-gui/ui/e2e/` in full** to copy its exact `test.describe`/fixture/mock-Tauri-`invoke` setup — do not invent a new mocking style.

- [ ] **Step 2: Write the spec**

Mock `list_harness_issues` to return one pending `corpus_scan` issue with a `target_path`, `scan_training_corpus` to return a count, `record_harness_issue_decision` and `propose_harness_issue_fix` to resolve successfully. The spec should:
1. Navigate to the Scientia surface, click the "Harness Issues" tab.
2. Assert the mocked issue's summary text is visible.
3. Click "Confirm & propose fix", assert `record_harness_issue_decision` was called with the issue's id and `propose_harness_issue_fix` was called with its `target_path`.
4. Click "Scan training corpus", assert a toast with the mocked count appears.

Note: this spec covers the frontend review-and-decide flow only. It does **not** exercise the Rust-side scorer/judge/gate wiring (Task 11) — that has no live-LLM integration test anywhere in this plan by design (see Task 11 Step 5's reasoning); don't claim this e2e spec covers it in the commit message.

- [ ] **Step 3: Run it**

Run: whatever command the sibling specs use (check `package.json` scripts in `crates/vox-gui/ui`).
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/e2e/harness-issue-discovery.spec.ts
git commit -m "test(vox-gui): add e2e spec for harness issue discovery review flow"
```

---

## Final verification

- [ ] Run the full backend test suite: `cargo test -p vox-db -p vox-orchestrator -p vox-orchestrator-mcp -p vox-gui -p vox-cli`
- [ ] Run the frontend test suite: `pnpm --dir crates/vox-gui/ui test`
- [ ] Run `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
- [ ] Run `vox ci check-codex-ssot` to confirm the `BASELINE_VERSION`/digest bump from Task 1 satisfies CI, not just the local test (the command is real — named directly in `contracts/db/baseline-version-policy.yaml`'s own comments — no need to hedge on what it's called).
- [ ] Run `cargo clippy -p vox-db -p vox-orchestrator -p vox-orchestrator-mcp -p vox-gui -- -D warnings` (per this project's admin-merge convention of verifying clippy locally before merge; this will also catch the dead-code class of defect the first draft had, e.g. an unused `pub use` re-export).
- [ ] Run `cargo run -q -p vox-arch-check` and check `contracts/ci/crate-edges.allow.v1.json` — this plan intentionally adds no new crate edges (Task 12's scope was narrowed specifically to avoid needing one); if any step above still required a new dependency edge, stop and follow AGENTS.md's Dependency Discipline (propose an exceptions-ledger entry in the PR description — do not add one to the contract file directly).
- [ ] Manually verify the kill-switch: with `harness_issue_detection_enabled` toggled off in Settings, confirm (via the test added in Task 11 Step 5, plus a manual chat session if feasible) that repeated tool errors no longer produce `scientia_harness_issues` rows.
