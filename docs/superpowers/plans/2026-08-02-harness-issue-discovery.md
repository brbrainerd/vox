# Harness Issue Discovery (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect repeated-correction patterns in live chat/agent sessions and stale/broken entries in the golden training corpus, surface both in a persistent GUI review queue (toast + session badge + inline transcript marker + panel), and let an approved decision dispatch a real, human-approved fix to the training corpus.

**Architecture:** Three new `scientia_harness_*` tables in `vox-db` (issues, decisions, fix proposals) back a synchronous in-process heuristic scorer hooked into the agent tool-call loop (`run_agent_turn`), a threshold-triggered LLM judge, an on-demand golden-corpus static scanner (`vox-lsp` HIR validation + frontmatter staleness), and a human-approved dispatch-to-fix pipeline (LLM proposes a unified diff via the `similar` crate, never auto-applied). Five new Tauri commands expose all of this to a new GUI panel under the Scientia surface, plus a session-rail badge and inline transcript marker built via read-time polling merges (not the existing fragile `cost_incurred` event-correlation pipeline).

**Tech Stack:** Rust (Turso/libSQL via `vox-db`, Tauri commands, `vox_actor_runtime::llm`, `vox-lsp`, `similar`), TypeScript/React (Tauri `invoke`, existing toast queue).

**Spec:** `docs/superpowers/specs/2026-08-02-harness-issue-discovery-design.md`

---

## Group A: Database schema & storage

### Task 1: Add three new tables to the Scientia schema domain

**Files:**
- Modify: `crates/vox-db/src/schema/domains/scientia.rs` (append near end, before the closing `"#;` at line ~421)
- Modify: `crates/vox-db/src/schema/manifest.rs:11-19` (bump `BASELINE_VERSION`)
- Modify: `contracts/db/baseline-version-policy.yaml`

- [ ] **Step 1: Write the failing digest test run**

Run: `cargo test -p vox-db baseline_policy_matches_compiled_schema -- --nocapture`
Expected: PASS (nothing has changed yet — this just confirms the test exists and the baseline is currently green, so any later failure is attributable to this task's changes).

- [ ] **Step 2: Append the three table DDLs to `scientia.rs`**

Open `crates/vox-db/src/schema/domains/scientia.rs`, find the closing `"#;` (currently the last line of the file, ~line 421), and insert the following immediately before it (after the last existing table's DDL):

```rust
-- Harness issue discovery (Phase 1): repeated-correction patterns detected
-- during live chat/agent sessions, plus static findings from golden-corpus
-- scans. Distinct from scientia_discovery_inbox/scientia_review_decisions,
-- which are tightly bound to publication_id/claim_id (research findings).
-- No SQL CHECK/TRIGGER (Turso/libSQL does not support them); validated in Rust.
CREATE TABLE IF NOT EXISTS scientia_harness_issues (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    source         TEXT    NOT NULL,          -- chat_session|corpus_scan (validated in Rust)
    session_key    TEXT,                      -- null for corpus_scan
    detected_at_ms INTEGER NOT NULL,
    category       TEXT    NOT NULL,
    severity       TEXT    NOT NULL,           -- low|medium|high (validated in Rust)
    summary        TEXT    NOT NULL,
    evidence_json  TEXT    NOT NULL,
    status         TEXT    NOT NULL            -- pending|confirmed|dismissed (validated in Rust)
);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_issues_status
    ON scientia_harness_issues(status, detected_at_ms);
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
CREATE INDEX IF NOT EXISTS idx_scientia_harness_decisions_issue
    ON scientia_harness_decisions(issue_id, decided_at_ms);

-- Dispatch-to-fix proposals for corpus-fixable confirmed issues. A proposal
-- is a unified diff against target_path; never applied without a human
-- approval that flips status to 'applied' and writes the file.
CREATE TABLE IF NOT EXISTS scientia_harness_fix_proposals (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    issue_id       INTEGER NOT NULL,
    target_path    TEXT    NOT NULL,
    proposed_diff  TEXT    NOT NULL,
    status         TEXT    NOT NULL,           -- pending_approval|applied|rejected (validated in Rust)
    proposed_at_ms INTEGER NOT NULL,
    resolved_at_ms INTEGER
);
CREATE INDEX IF NOT EXISTS idx_scientia_harness_fix_proposals_issue
    ON scientia_harness_fix_proposals(issue_id);
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

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/src/schema/domains/scientia.rs crates/vox-db/src/schema/manifest.rs contracts/db/baseline-version-policy.yaml
git commit -m "feat(vox-db): add scientia_harness_issues/decisions/fix_proposals tables"
```

---

### Task 2: `ops_harness_issues.rs` — issues table CRUD

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_issues.rs`
- Modify: `crates/vox-db/src/store/mod.rs`

- [ ] **Step 1: Write the failing round-trip test (in-file, matching `ops_discovery_inbox.rs`'s pattern)**

```rust
//! Store ops for `scientia_harness_issues`.
//!
//! One row per detected harness issue — either from the synchronous
//! chat-session heuristic+judge detector (`source = "chat_session"`) or from
//! an on-demand golden-corpus scan (`source = "corpus_scan"`).

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
            detected_at_ms: row.get(3).map_err(StoreError::Turso)?,
            category: row.get(4).map_err(StoreError::Turso)?,
            severity: row.get(5).map_err(StoreError::Turso)?,
            summary: row.get(6).map_err(StoreError::Turso)?,
            evidence_json: row.get(7).map_err(StoreError::Turso)?,
            status: row.get(8).map_err(StoreError::Turso)?,
        })
    }
}

impl VoxDb {
    /// Insert a new pending harness issue; returns its `id`.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_harness_issue(
        &self,
        source: &str,
        session_key: Option<&str>,
        detected_at_ms: i64,
        category: &str,
        severity: &str,
        summary: &str,
        evidence_json: &str,
    ) -> Result<i64, StoreError> {
        if !VALID_SOURCES.contains(&source) {
            return Err(StoreError::Db(format!(
                "scientia_harness_issues.source must be one of {VALID_SOURCES:?}, got {source:?}"
            )));
        }
        if !VALID_SEVERITIES.contains(&severity) {
            return Err(StoreError::Db(format!(
                "scientia_harness_issues.severity must be one of {VALID_SEVERITIES:?}, got {severity:?}"
            )));
        }
        self.conn
            .execute(
                "INSERT INTO scientia_harness_issues \
                 (source, session_key, detected_at_ms, category, severity, summary, evidence_json, status) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
                params![
                    source.to_string(),
                    session_key.map(str::to_string),
                    detected_at_ms,
                    category.to_string(),
                    severity.to_string(),
                    summary.to_string(),
                    evidence_json.to_string(),
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

    /// List harness issues, optionally filtered by `status` and/or `source`, newest first.
    pub async fn list_harness_issues(
        &self,
        status: Option<&str>,
        source: Option<&str>,
        limit: i64,
    ) -> Result<Vec<HarnessIssueRow>, StoreError> {
        let sql = "SELECT id, source, session_key, detected_at_ms, category, severity, summary, evidence_json, status \
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

    /// List harness issues for one chat session, oldest first (for inline transcript merge).
    pub async fn list_harness_issues_for_session(
        &self,
        session_key: &str,
    ) -> Result<Vec<HarnessIssueRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, source, session_key, detected_at_ms, category, severity, summary, evidence_json, status \
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
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_then_list_shows_pending_issue() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let id = db
            .insert_harness_issue(
                "chat_session",
                Some("session-abc"),
                1_000,
                "repeated_compiler_error",
                "medium",
                "Same borrow-checker error hit twice in a row",
                r#"{"error_hash":"deadbeef"}"#,
            )
            .await
            .expect("insert");
        assert!(id >= 1);

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
    }

    #[tokio::test]
    async fn insert_rejects_invalid_source() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let err = db
            .insert_harness_issue("bogus", None, 1_000, "cat", "low", "s", "{}")
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("source must be one of"));
    }
}
```

- [ ] **Step 2: Register the module in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_issues;` alphabetically (between `mod ops_finding_candidates;` and `mod ops_identity;`), and add to the `pub use` block:

```rust
pub use ops_harness_issues::{HarnessIssueRow, VALID_SEVERITIES, VALID_SOURCES, VALID_STATUSES as VALID_HARNESS_ISSUE_STATUSES};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-db ops_harness_issues`
Expected: PASS (both tests)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_issues.rs crates/vox-db/src/store/mod.rs
git commit -m "feat(vox-db): add ops_harness_issues CRUD for scientia_harness_issues"
```

---

### Task 3: `ops_harness_decisions.rs` — append-only decision ledger

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_decisions.rs`
- Modify: `crates/vox-db/src/store/mod.rs`

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
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn recording_decision_flips_issue_status() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue("corpus_scan", None, 1_000, "stale_frontmatter", "low", "s", "{}")
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
            .insert_harness_issue("corpus_scan", None, 1_000, "c", "low", "s", "{}")
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

- [ ] **Step 2: Register in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_decisions;` (alphabetically before `mod ops_harness_issues;`) and:

```rust
pub use ops_harness_decisions::{HarnessIssueDecisionRow, VALID_DECISIONS as VALID_HARNESS_DECISIONS};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-db ops_harness_decisions`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_decisions.rs crates/vox-db/src/store/mod.rs
git commit -m "feat(vox-db): add ops_harness_decisions append-only ledger"
```

---

### Task 4: `ops_harness_fix_proposals.rs` — dispatch-to-fix proposals

**Files:**
- Create: `crates/vox-db/src/store/ops_harness_fix_proposals.rs`
- Modify: `crates/vox-db/src/store/mod.rs`

- [ ] **Step 1: Write the file**

```rust
//! Store ops for `scientia_harness_fix_proposals`.
//!
//! A proposal is a unified diff against `target_path`, produced by a dispatched
//! model call. Never applied to disk automatically — `status` starts at
//! `pending_approval` and only a human-approved `resolve` (Task 14) flips it.

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
            proposed_diff: row.get(3).map_err(StoreError::Turso)?,
            status: row.get(4).map_err(StoreError::Turso)?,
            proposed_at_ms: row.get(5).map_err(StoreError::Turso)?,
            resolved_at_ms: row.get(6).map_err(StoreError::Turso)?,
        })
    }
}

impl VoxDb {
    /// Insert a new pending-approval fix proposal; returns its `id`.
    pub async fn insert_harness_fix_proposal(
        &self,
        issue_id: i64,
        target_path: &str,
        proposed_diff: &str,
        proposed_at_ms: i64,
    ) -> Result<i64, StoreError> {
        self.conn
            .execute(
                "INSERT INTO scientia_harness_fix_proposals \
                 (issue_id, target_path, proposed_diff, status, proposed_at_ms) \
                 VALUES (?1, ?2, ?3, 'pending_approval', ?4)",
                params![issue_id, target_path.to_string(), proposed_diff.to_string(), proposed_at_ms],
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
                "SELECT id, issue_id, target_path, proposed_diff, status, proposed_at_ms, resolved_at_ms \
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
                "SELECT id, issue_id, target_path, proposed_diff, status, proposed_at_ms, resolved_at_ms \
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
    /// callers apply the diff themselves before calling this with `applied`.
    pub async fn resolve_harness_fix_proposal(
        &self,
        id: i64,
        status: &str,
        resolved_at_ms: i64,
    ) -> Result<(), StoreError> {
        if status != "applied" && status != "rejected" {
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
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn insert_list_resolve_round_trip() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let issue_id = db
            .insert_harness_issue("corpus_scan", None, 1_000, "stale_frontmatter", "low", "s", "{}")
            .await
            .expect("insert issue");
        let proposal_id = db
            .insert_harness_fix_proposal(issue_id, "examples/golden/hello.vox", "--- a\n+++ b\n", 1_500)
            .await
            .expect("insert proposal");

        let pending = db
            .list_harness_fix_proposals(Some("pending_approval"), 10)
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, proposal_id);

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
}
```

- [ ] **Step 2: Register in `crates/vox-db/src/store/mod.rs`**

Add `mod ops_harness_fix_proposals;` (alphabetically after `mod ops_harness_issues;`) and:

```rust
pub use ops_harness_fix_proposals::HarnessFixProposalRow;
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-db ops_harness_fix_proposals`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-db/src/store/ops_harness_fix_proposals.rs crates/vox-db/src/store/mod.rs
git commit -m "feat(vox-db): add ops_harness_fix_proposals dispatch-to-fix storage"
```

---

## Group B: Config toggle (default ON, opt-out)

### Task 5: Add `harness_issue_detection_enabled` to `OrchestratorConfig`

**Files:**
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs:186-188` (near `scaling_enabled`)
- Modify: `crates/vox-orchestrator/src/config/impl_default.rs:53` (near `scaling_enabled`)

- [ ] **Step 1: Write the failing test**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, find the `#[cfg(test)]` module in this file (or the config crate's default-values test if one already asserts `scaling_enabled` defaults) and add:

```rust
#[test]
fn harness_issue_detection_enabled_defaults_to_true() {
    let cfg = OrchestratorConfig::default();
    assert!(cfg.harness_issue_detection_enabled);
}
```

If no `#[cfg(test)] mod tests` block exists in this file, add one at the end:

```rust
#[cfg(test)]
mod harness_issue_detection_default_tests {
    use super::OrchestratorConfig;

    #[test]
    fn harness_issue_detection_enabled_defaults_to_true() {
        let cfg = OrchestratorConfig::default();
        assert!(cfg.harness_issue_detection_enabled);
    }
}
```

- [ ] **Step 2: Run it to confirm it fails to compile (field doesn't exist yet)**

Run: `cargo test -p vox-orchestrator harness_issue_detection_enabled_defaults_to_true`
Expected: FAIL — compile error, "no field `harness_issue_detection_enabled`"

- [ ] **Step 3: Add the field**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, immediately after the `scaling_enabled` field (line 187-188):

```rust
    /// Whether dynamic scaling is enabled (default: false).
    #[serde(default = "default_false")]
    pub scaling_enabled: bool,
    /// Whether synchronous chat-session repeated-correction detection is
    /// enabled (default: true — on by default, opt-out via GUI Settings).
    #[serde(default = "default_true")]
    pub harness_issue_detection_enabled: bool,
```

In `crates/vox-orchestrator/src/config/impl_default.rs`, immediately after `scaling_enabled: default_false(),` (line 53):

```rust
            scaling_enabled: default_false(),
            harness_issue_detection_enabled: default_true(),
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-orchestrator harness_issue_detection_enabled_defaults_to_true`
Expected: PASS

- [ ] **Step 5: Run the full config crate's test suite to catch any other exhaustive-struct-literal sites**

Run: `cargo test -p vox-orchestrator config::`
Expected: PASS. If a compile error names another file constructing `OrchestratorConfig { .. }` without `..Default::default()`, add `harness_issue_detection_enabled: true,` (or the appropriate value) there too.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-orchestrator/src/config/impl_default.rs
git commit -m "feat(vox-orchestrator): add harness_issue_detection_enabled config field"
```

---

### Task 6: Tauri get/set commands for the toggle

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs` (near `scaling_enabled` handling, ~lines 417-517 for setter, ~640-661 for getter)

- [ ] **Step 1: Add the setter branch**

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

- [ ] **Step 2: Add the getter field**

In `get_orchestrator_config`, add to the `serde_json::json!({...})` object:

```rust
        "scalingEnabled": cfg.scaling_enabled,
        "harnessIssueDetectionEnabled": cfg.harness_issue_detection_enabled,
```

- [ ] **Step 3: Build to confirm it compiles**

Run: `cargo check -p vox-gui`
Expected: PASS (no errors)

- [ ] **Step 4: Commit**

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
<Row label="Harness issue detection" hint="Watch chat sessions for repeated mistakes and surface a review queue">
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

- [ ] **Step 1: Write the failing tests**

```rust
//! Synchronous, in-process heuristic scorer for repeated-correction patterns
//! within a single `run_agent_turn` tool-call loop. Pure logic, no I/O — kept
//! separate from the loop itself so it's unit-testable without a live LLM or DB.
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
    /// (tool_name, first-line-of-result hash) -> consecutive-seen count.
    error_signatures: HashMap<(String, u64), u32>,
    /// (tool_name, args JSON) -> consecutive-call count.
    retries: HashMap<(String, String), u32>,
    last_call: Option<(String, String)>,
    score: u32,
}

impl HarnessIssueScorer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one tool call's name, arguments (as a JSON string), and result string.
    /// Returns `true` once the accumulated score first crosses [`THRESHOLD`].
    pub fn record(&mut self, tool_name: &str, args_json: &str, result: &str) -> bool {
        let is_error = result.starts_with("Error:") || result.contains("\"success\":false") || result.contains("error");

        if is_error {
            let first_line = result.lines().next().unwrap_or(result);
            let mut hasher = DefaultHasher::new();
            first_line.hash(&mut hasher);
            let key = (tool_name.to_string(), hasher.finish());
            let count = self.error_signatures.entry(key).or_insert(0);
            *count += 1;
            if *count >= 2 {
                self.score += 1;
            }
        }

        let call_key = (tool_name.to_string(), args_json.to_string());
        if self.last_call.as_ref() == Some(&call_key) {
            let count = self.retries.entry(call_key).or_insert(1);
            *count += 1;
            if *count >= 3 {
                self.score += 1;
            }
        } else {
            self.last_call = Some(call_key);
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
    fn repeated_error_signature_crosses_threshold() {
        let mut scorer = HarnessIssueScorer::new();
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        // Two occurrences of the same signature = +1 score; need THRESHOLD=3 hits.
        assert!(!scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
        assert!(scorer.record("build_crate", "{}", "Error: E0502 cannot borrow"));
    }

    #[test]
    fn retry_loop_crosses_threshold() {
        let mut scorer = HarnessIssueScorer::new();
        let args = r#"{"path":"foo.vox"}"#;
        assert!(!scorer.record("validate_file", args, "Error: parse failed"));
        assert!(!scorer.record("validate_file", args, "Error: parse failed"));
        assert!(!scorer.record("validate_file", args, "Error: parse failed"));
        // 3rd identical consecutive call bumps retries score by 1 (on top of the
        // 2 error-signature hits already counted above), crossing THRESHOLD=3.
        assert!(scorer.record("validate_file", args, "Error: parse failed"));
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

- [ ] **Step 2: Register the module**

In `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs`, add `mod harness_issue_scorer;` alongside the other `mod` declarations in that file (e.g. near `mod agent_loop;` / `mod message;` — match whatever ordering convention the file already uses).

- [ ] **Step 3: Run the tests**

Run: `cargo test -p vox-orchestrator-mcp harness_issue_scorer`
Expected: PASS (all 4 tests)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_scorer.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add pure heuristic scorer for repeated-correction patterns"
```

---

### Task 9: Thread `session_id` into `run_agent_turn`

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:151-160` (signature) and its 4 test call sites (~415, 566, 607, 662, 713)
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs:105-117` (the one real caller)

- [ ] **Step 1: Add the parameter to the signature**

In `agent_loop.rs`, change the `run_agent_turn` signature (line 151-160ish) to add `session_id: Option<&str>` right after `state`:

```rust
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_agent_turn(
    state: &ServerState,
    session_id: Option<&str>,
    prior_conversation: Vec<LlmChatMessage>,
    system_prompt: String,
    user_message: String,
    permission_mode: Option<PermissionMode>,
    active_skill_id: Option<String>,
    llm_config_template: LlmConfig,
    max_iterations: u32,
    // ... existing return type, unchanged — do not alter it.
) {
```

Only insert the new `session_id: Option<&str>` parameter as the second argument; leave every other parameter name, type, and the function's actual return type exactly as they already are in the file. The `{` above stands in for "whatever the real return arrow and type already say" — read the current signature first and edit it in place rather than replacing the whole thing.

- [ ] **Step 2: Update the real caller in `message.rs`**

At line 105-117, add `Some(session_id)` as the second argument (the function already has `session_id: &str` in scope from its own signature at line 52):

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

- [ ] **Step 3: Try to build to find every remaining call site**

Run: `cargo build -p vox-orchestrator-mcp --tests 2>&1 | head -50`
Expected: FAIL with 4 compile errors, one per test call site in `agent_loop.rs` (lines ~415, ~566, ~607, ~662, ~713 — exact line numbers will have shifted after Step 1's edit; the compiler errors give the authoritative locations).

- [ ] **Step 4: Fix each test call site**

For each compiler-reported call site, insert `None,` as the second argument (test call sites have no real session identity):

```rust
        let outcome = run_agent_turn(
            &state,
            None,
            vec![],
            // ...(rest unchanged)
```

- [ ] **Step 5: Build again to confirm it's clean**

Run: `cargo build -p vox-orchestrator-mcp --tests`
Expected: PASS

- [ ] **Step 6: Run the existing agent_loop tests to confirm no behavior changed**

Run: `cargo test -p vox-orchestrator-mcp agent_loop::`
Expected: PASS (same tests as before, now compiling with the new parameter)

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs
git commit -m "refactor(vox-orchestrator-mcp): thread session_id into run_agent_turn"
```

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
//! this is adapted from).

use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig};
use vox_actor_runtime::{ActivityOptions, ActivityResult};

/// A judged, real harness issue. `None` is returned by [`judge`] when the
/// judge concludes the accumulated signals were not a genuine issue.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct JudgedHarnessIssue {
    pub category: String,
    pub severity: String, // low|medium|high
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

const JUDGE_SYSTEM_PROMPT: &str = "You are reviewing a short excerpt of recent \
tool calls from an AI coding agent's turn, because a heuristic scorer flagged \
repeated errors or retries. Decide whether this is a genuine recurring issue \
worth a human's attention (e.g. the agent kept hitting the same compiler \
error, or retried an identical failing action). Respond with ONLY a JSON \
object, no prose, matching exactly: \
{\"is_issue\": bool, \"category\": string, \"severity\": \"low\"|\"medium\"|\"high\", \"summary\": string}. \
If this looks like normal iterative debugging rather than a stuck loop, set is_issue to false.";

/// Judge a small excerpt of recent tool-call activity. Returns `Ok(None)` for
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
        severity: verdict.severity,
        summary: verdict.summary,
    })
}
```

- [ ] **Step 2: Register the module**

In `crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs`, add `mod harness_issue_judge;`.

- [ ] **Step 3: Build**

Run: `cargo build -p vox-orchestrator-mcp`
Expected: PASS. If `LlmChatMessage`/`LlmConfig`/`ActivityOptions`/`ActivityResult` import paths differ slightly from what's written here, fix the `use` statement to match — the exact re-export path may need a one-line adjustment; the field names/shapes above are grounded from `crates/vox-effort-audit/src/judge/mod.rs`.

- [ ] **Step 4: Write and run a unit test for JSON parsing (no live LLM call)**

Add to the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::JudgeVerdict;

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
}
```

Run: `cargo test -p vox-orchestrator-mcp harness_issue_judge`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/harness_issue_judge.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add LLM-judge for harness issue classification"
```

---

### Task 11: Wire scorer + judge into the tool-dispatch loop

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs:243-262` (the tool-dispatch loop body)

- [ ] **Step 1: Add the scorer, gate on config, and wire the judge + DB write**

Immediately before the tool-dispatch loop (the `for call in &calls { ... }` block at line 243), initialize the scorer once per `run_agent_turn` invocation:

```rust
    let mut harness_scorer = super::harness_issue_scorer::HarnessIssueScorer::new();
```

(Place this near the top of the function, alongside other loop-scoped `let mut` state — e.g. next to wherever `tool_calls_made` is declared.)

Then change the dispatch loop body to score each result and, on threshold crossing, run the judge and persist an issue:

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

                    if state.orchestrator.harness_issue_detection_enabled {
                        let args_json = call.arguments.to_string();
                        let crossed = harness_scorer.record(&call.name, &args_json, &content);
                        if crossed {
                            let recent_activity =
                                format!("tool: {}\nargs: {}\nresult: {}", call.name, args_json, content);
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
                                    .insert_harness_issue(
                                        "chat_session",
                                        session_key.as_deref(),
                                        detected_at_ms,
                                        &issue.category,
                                        &issue.severity,
                                        &issue.summary,
                                        &evidence_json,
                                    )
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

The judge call hardcodes `"auto"` as the model, matching `provider: "auto"` in `LlmConfig` — the model registry resolves the actual vendor/model, same as every other `provider: "auto"` call site in this codebase.

- [ ] **Step 2: Verify `state.orchestrator` and `state.db` field access match `ServerState`'s real shape**

Run: `cargo check -p vox-orchestrator-mcp 2>&1 | head -60`
Expected: either PASS, or a small number of field-access errors. If `state.orchestrator` is not directly a struct with `.harness_issue_detection_enabled` (e.g. it might be wrapped in a snapshot accessor), fix the access expression to match — search `crates/vox-orchestrator-mcp/src/server_state.rs` for the `orchestrator` field's exact type and how other code in this file reads a boolean flag off it (e.g. `context_fill_ratio` at `message.rs:70` reads `&state.orchestrator` directly, which is the pattern this step assumes).

- [ ] **Step 3: Add `chrono` as a dependency if not already present**

Run: `grep -c '^chrono' crates/vox-orchestrator-mcp/Cargo.toml`
If `0`, add `chrono.workspace = true` to the `[dependencies]` section (chrono is already a workspace dependency used elsewhere in this codebase for `timestamp_millis()`-style calls).

- [ ] **Step 4: Build and run the existing agent_loop test suite to confirm no regression**

Run: `cargo test -p vox-orchestrator-mcp agent_loop::`
Expected: PASS — the scorer only activates on real tool-call content matching error patterns, and existing tests use mocked "no tools needed" / simple tool-call responses that won't cross `THRESHOLD`.

- [ ] **Step 5: Write an integration-style test proving the wiring fires the judge (using a fake judge is out of scope — instead assert the scorer's `record` return value is consulted per iteration by adding a debug counter, OR skip a live-judge test here and rely on Task 8's scorer unit tests plus Task 10's parser unit tests as the coverage for this wiring; the actual end-to-end path is covered by Task 19's Playwright spec).**

No new test file for this step — this task's correctness is covered by: Task 8 (scorer logic), Task 10 (judge parsing), and Task 19 (e2e). Document this explicitly in the commit message.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(vox-orchestrator-mcp): wire heuristic scorer + LLM judge into agent tool-dispatch loop

Coverage: scorer logic (harness_issue_scorer tests), judge JSON parsing
(harness_issue_judge tests), end-to-end (Task 19 Playwright spec)."
```

---

## Group D: Corpus scanner + dispatch-to-fix

### Task 12: Golden corpus static scanner + Tauri command

**Files:**
- Create: `crates/vox-gui/src/commands/harness_issues.rs`
- Modify: `crates/vox-gui/Cargo.toml` (add `vox-lsp.workspace = true`)
- Modify: `crates/vox-gui/src/main.rs` (register commands + `mod`)

- [ ] **Step 1: Add the `vox-lsp` dependency**

In `crates/vox-gui/Cargo.toml`, add to `[dependencies]` (alongside the existing `vox-db = { workspace = true }` at line 28):

```toml
vox-lsp = { workspace = true }
```

- [ ] **Step 2: Write the scanner + Tauri command with a failing test**

```rust
//! Tauri commands for harness issue discovery (Phase 1): listing/deciding
//! issues, and the on-demand golden-corpus static scanner.

use chrono::Datelike as _;
use std::path::Path;

/// One finding from a golden-corpus scan, before it's persisted as a
/// `scientia_harness_issues` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusFinding {
    pub target_path: String,
    pub category: String,
    pub severity: String,
    pub summary: String,
}

const STALENESS_THRESHOLD_DAYS: i64 = 365;

/// Scan a single `.vox` file's content for frontmatter staleness and compile
/// failures. `today_ymd` is injected (not `chrono::Utc::now()`) so this is
/// pure and testable without wall-clock dependence.
pub fn scan_golden_file(path: &str, content: &str, today_ymd: (i32, u32, u32)) -> Vec<CorpusFinding> {
    let mut findings = Vec::new();

    if let Some(last_validated) = extract_frontmatter_field(content, "last_validated") {
        if let Some(age_days) = days_since(&last_validated, today_ymd) {
            if age_days > STALENESS_THRESHOLD_DAYS {
                findings.push(CorpusFinding {
                    target_path: path.to_string(),
                    category: "stale_frontmatter".to_string(),
                    severity: "low".to_string(),
                    summary: format!("last_validated {last_validated} is {age_days} days old"),
                });
            }
        }
    }

    let diagnostics = vox_lsp::validate_document_with_hir(content);
    let has_error = diagnostics
        .iter()
        .any(|d| d.severity == Some(tower_lsp_server::ls_types::DiagnosticSeverity::ERROR));
    if has_error {
        findings.push(CorpusFinding {
            target_path: path.to_string(),
            category: "compile_failure".to_string(),
            severity: "high".to_string(),
            summary: format!("{} HIR diagnostic(s) at error severity", diagnostics.len()),
        });
    }

    findings
}

fn extract_frontmatter_field(content: &str, key: &str) -> Option<String> {
    let prefix = format!("// {key}: ");
    content
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .map(|line| line.trim_start()[prefix.len()..].trim().to_string())
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

/// Scan every `examples/golden/*.vox` file under `repo_root`, persist a
/// `scientia_harness_issues` row per finding, and return how many were found.
#[tauri::command]
pub async fn scan_training_corpus(repo_root: String) -> Result<usize, String> {
    let db = crate::commands::scientia_review::db().await?;
    let golden_dir = Path::new(&repo_root).join("examples").join("golden");
    let mut entries = tokio::fs::read_dir(&golden_dir)
        .await
        .map_err(|e| format!("read_dir {}: {e}", golden_dir.display()))?;

    let today = chrono::Utc::now().date_naive();
    let today_ymd = (today.year(), today.month(), today.day());

    let mut count = 0usize;
    while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let rel_path = path
            .strip_prefix(&repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for finding in scan_golden_file(&rel_path, &content, today_ymd) {
            let now_ms = chrono::Utc::now().timestamp_millis();
            let evidence_json = serde_json::json!({ "path": finding.target_path }).to_string();
            db.insert_harness_issue(
                "corpus_scan",
                None,
                now_ms,
                &finding.category,
                &finding.severity,
                &finding.summary,
                &evidence_json,
            )
            .await
            .map_err(|e| e.to_string())?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_stale_frontmatter() {
        let content = "// last_validated: 2020-01-01\nfn main() {}\n";
        let findings = scan_golden_file("examples/golden/x.vox", content, (2026, 8, 2));
        assert!(findings.iter().any(|f| f.category == "stale_frontmatter"));
    }

    #[test]
    fn does_not_flag_recent_frontmatter() {
        let content = "// last_validated: 2026-07-01\nfn main() {}\n";
        let findings = scan_golden_file("examples/golden/x.vox", content, (2026, 8, 2));
        assert!(!findings.iter().any(|f| f.category == "stale_frontmatter"));
    }

    #[test]
    fn missing_frontmatter_field_is_skipped_not_flagged() {
        let content = "fn main() {}\n";
        let findings = scan_golden_file("examples/golden/x.vox", content, (2026, 8, 2));
        assert!(!findings.iter().any(|f| f.category == "stale_frontmatter"));
    }
}
```

- [ ] **Step 3: Register the module and command**

In `crates/vox-gui/src/main.rs`, add `mod harness_issues;` to the `commands` module declarations, and add `commands::harness_issues::scan_training_corpus,` to the `generate_handler!` list (near the `scientia_review` command entries found in Task 15).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (3 tests)

- [ ] **Step 5: Build the whole crate**

Run: `cargo check -p vox-gui`
Expected: PASS. If `tower_lsp_server::ls_types::DiagnosticSeverity` isn't the correct import path in this crate's dependency graph, adjust the `use` to whatever path `vox-lsp`'s public API actually re-exports `DiagnosticSeverity` under (check `crates/vox-lsp/src/lib.rs`'s own imports for the canonical path, already confirmed as `tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, ...}` at `crates/vox-lsp/src/lib.rs:546` — if that's a test-only import, find the non-test equivalent near the top of the file).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/harness_issues.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): add golden corpus static scanner (staleness + compile-check)"
```

---

### Task 13: Dispatch-to-fix pipeline + apply/reject commands

**Files:**
- Modify: `crates/vox-gui/src/commands/harness_issues.rs` (add dispatch + resolve commands)
- Modify: `crates/vox-gui/Cargo.toml` (add `similar.workspace = true`)
- Modify: `crates/vox-gui/src/main.rs` (register new commands)

- [ ] **Step 1: Add the `similar` dependency**

In `crates/vox-gui/Cargo.toml`, add to `[dependencies]`:

```toml
similar = { workspace = true }
```

- [ ] **Step 2: Add the diff-generation helper (pure, testable) and the dispatch/resolve commands**

Append to `crates/vox-gui/src/commands/harness_issues.rs`:

```rust
/// Build a unified diff between the current and proposed file content.
pub fn build_unified_diff(target_path: &str, old: &str, new: &str) -> String {
    similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(target_path, target_path)
        .to_string()
}

/// Dispatch an LLM call proposing a corrected version of `target_path`'s
/// content for a confirmed, corpus-fixable harness issue. Stores the result
/// as a `pending_approval` fix proposal — never writes to disk.
#[tauri::command]
pub async fn propose_harness_issue_fix(
    issue_id: i64,
    repo_root: String,
    target_path: String,
) -> Result<i64, String> {
    let db = crate::commands::scientia_review::db().await?;
    let full_path = std::path::Path::new(&repo_root).join(&target_path);
    let old_content = tokio::fs::read_to_string(&full_path)
        .await
        .map_err(|e| format!("read {}: {e}", full_path.display()))?;

    let issues = db
        .list_harness_issues(None, None, 500)
        .await
        .map_err(|e| e.to_string())?;
    let issue = issues
        .into_iter()
        .find(|i| i.id == issue_id)
        .ok_or_else(|| format!("no harness issue with id {issue_id}"))?;

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
    db.insert_harness_fix_proposal(issue_id, &target_path, &diff, proposed_at_ms)
        .await
        .map_err(|e| e.to_string())
}

/// List fix proposals, optionally filtered by status.
#[tauri::command]
pub async fn list_harness_fix_proposals(
    status: Option<String>,
) -> Result<Vec<vox_db::HarnessFixProposalRow>, String> {
    let db = crate::commands::scientia_review::db().await?;
    db.list_harness_fix_proposals(status.as_deref(), 200)
        .await
        .map_err(|e| e.to_string())
}

/// Approve (apply the diff to `target_path` on disk) or reject a proposal.
#[tauri::command]
pub async fn resolve_harness_fix_proposal(
    proposal_id: i64,
    repo_root: String,
    approve: bool,
) -> Result<(), String> {
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

    // Applying the *diff* (not just re-fetching a "new content" we didn't keep)
    // requires the original content: re-read the current file and apply the
    // stored unified diff's hunks. Simpler and equally safe for v1: the
    // proposal's diff was generated from a full-file rewrite (Task 13 Step 2),
    // so re-derive the new content by reading the diff's `+` lines.
    let new_content: String = proposal
        .proposed_diff
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| format!("{}\n", &l[1..]))
        .collect();

    let full_path = std::path::Path::new(&repo_root).join(&proposal.target_path);
    tokio::fs::write(&full_path, new_content)
        .await
        .map_err(|e| format!("write {}: {e}", full_path.display()))?;

    db.resolve_harness_fix_proposal(proposal_id, "applied", resolved_at_ms)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Write a test for `build_unified_diff`**

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn unified_diff_contains_both_paths_and_changed_lines() {
        let diff = build_unified_diff("examples/golden/x.vox", "old line\n", "new line\n");
        assert!(diff.contains("examples/golden/x.vox"));
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (4 tests now)

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

- [ ] **Step 1: Add the two commands**

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

/// List harness issues for one chat session (used by the inline transcript merge).
#[tauri::command]
pub async fn list_harness_issues_for_session(
    session_key: String,
) -> Result<Vec<vox_db::HarnessIssueRow>, String> {
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

- [ ] **Step 2: Register in `main.rs`**

Add `commands::harness_issues::list_harness_issues`, `commands::harness_issues::list_harness_issues_for_session`, `commands::harness_issues::record_harness_issue_decision` to `generate_handler!`.

- [ ] **Step 3: Build the full crate**

Run: `cargo check -p vox-gui`
Expected: PASS

- [ ] **Step 4: Run the full harness_issues test module one more time**

Run: `cargo test -p vox-gui harness_issues::tests`
Expected: PASS (same 4 tests — this task adds no new pure logic to test, only thin DB-passthrough commands)

- [ ] **Step 5: Commit**

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

export function scanTrainingCorpus(repoRoot: string): Promise<number> {
  return invoke<number>('scan_training_corpus', { repoRoot });
}

export function proposeHarnessIssueFix(
  issueId: number,
  repoRoot: string,
  targetPath: string,
): Promise<number> {
  return invoke<number>('propose_harness_issue_fix', {
    issueId,
    repoRoot,
    targetPath,
  });
}

export function listHarnessFixProposals(status?: string): Promise<HarnessFixProposalRow[]> {
  return invoke<HarnessFixProposalRow[]>('list_harness_fix_proposals', { status: status ?? null });
}

export function resolveHarnessFixProposal(
  proposalId: number,
  repoRoot: string,
  approve: boolean,
): Promise<void> {
  return invoke<void>('resolve_harness_fix_proposal', { proposalId, repoRoot, approve });
}
```

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
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx`

- [ ] **Step 1: Write the panel component (pattern-matched off `DiscoveryInbox.tsx`)**

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
  const [issues, setIssues] = useState<HarnessIssueRow[]>([]);
  const [proposals, setProposals] = useState<HarnessFixProposalRow[]>([]);
  const [scanning, setScanning] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [pendingIssues, pendingProposals] = await Promise.all([
        listHarnessIssues('pending'),
        listHarnessFixProposals('pending_approval'),
      ]);
      setIssues(pendingIssues);
      setProposals(pendingProposals);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Harness issues', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = window.setInterval(refresh, 10_000);
    return () => window.clearInterval(id);
  }, [refresh]);

  const decide = useCallback(
    async (issueId: number, decision: 'confirmed' | 'dismissed') => {
      try {
        await recordHarnessIssueDecision(issueId, decision);
        if (decision === 'confirmed') {
          await proposeHarnessIssueFix(issueId, '.', 'examples/golden/placeholder.vox');
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
      const found = await scanTrainingCorpus('.');
      pushToast({ tone: 'info', title: 'Training corpus scan', body: `${found} issue(s) found`, cause: 'scan-complete' });
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
        await resolveHarnessFixProposal(proposalId, '.', approve);
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Fix proposal', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast, refresh],
  );

  return (
    <div className="flex min-h-0 flex-col gap-4 p-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-sm uppercase tracking-wide text-text-secondary">Harness Issues</h2>
        <button
          type="button"
          onClick={scan}
          disabled={scanning}
          className="rounded-md border border-border-subtle px-3 py-1.5 text-xs text-text-secondary hover:bg-overlay-hover disabled:opacity-50"
        >
          {scanning ? 'Scanning…' : 'Scan training corpus'}
        </button>
      </div>

      <div role="list" aria-label="Pending harness issues" className="flex flex-col gap-2">
        {issues.length === 0 ? (
          <div className="text-xs text-text-muted">No pending issues.</div>
        ) : (
          issues.map((issue) => (
            <div key={issue.id} role="listitem" className="rounded-md border border-border-subtle p-3">
              <div className="flex items-center justify-between text-xs">
                <span className="font-mono uppercase text-text-muted">{issue.source} · {issue.severity}</span>
              </div>
              <div className="mt-1 text-sm text-text-primary">{issue.summary}</div>
              <div className="mt-2 flex gap-2">
                <button
                  type="button"
                  onClick={() => decide(issue.id, 'confirmed')}
                  className="rounded border border-brass/30 bg-brass/10 px-2 py-1 text-xs text-brass"
                >
                  Confirm & propose fix
                </button>
                <button
                  type="button"
                  onClick={() => decide(issue.id, 'dismissed')}
                  className="rounded border border-border-subtle px-2 py-1 text-xs text-text-muted"
                >
                  Dismiss
                </button>
              </div>
            </div>
          ))
        )}
      </div>

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
    </div>
  );
}
```

Note on the `proposeHarnessIssueFix(issueId, '.', 'examples/golden/placeholder.vox')` call in `decide`: the target path should come from the issue's `evidence_json.path` for `corpus_scan`-sourced issues, not a placeholder. Fix this before Step 3: change `decide` to parse `issue.evidence_json` and only call `proposeHarnessIssueFix` when a `path` field is present:

```tsx
  const decide = useCallback(
    async (issue: HarnessIssueRow, decision: 'confirmed' | 'dismissed') => {
      try {
        await recordHarnessIssueDecision(issue.id, decision);
        if (decision === 'confirmed') {
          const evidence = JSON.parse(issue.evidence_json) as { path?: string };
          if (evidence.path) {
            await proposeHarnessIssueFix(issue.id, '.', evidence.path);
          }
        }
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Harness issues', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      }
    },
    [pushToast, refresh],
  );
```

And update the two call sites (`decide(issue.id, 'confirmed')` / `decide(issue.id, 'dismissed')`) to `decide(issue, 'confirmed')` / `decide(issue, 'dismissed')`.

- [ ] **Step 2: Register the new tab in `ScientiaSurface.tsx`**

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

- [ ] **Step 3: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Write a component test**

Create `crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.test.tsx`, following whatever testing pattern `DiscoveryInbox.test.tsx` uses (mock `@tauri-apps/api/core`'s `invoke`, render, assert list items appear). Since this plan doesn't have `DiscoveryInbox.test.tsx`'s exact content grounded, the implementing engineer should open that file first and copy its mocking setup verbatim, swapping in `harnessIssuesApi` invoke command names (`list_harness_issues`, `list_harness_fix_proposals`) and asserting the rendered summary text appears.

Run: `pnpm --dir crates/vox-gui/ui test HarnessIssuesPanel`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/HarnessIssuesPanel.test.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx
git commit -m "feat(vox-gui): add Harness Issues review panel as a new Scientia tab"
```

---

### Task 17: Session-rail badge

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx:7-20, 86-105`

- [ ] **Step 1: Extend the props/type and render a badge**

Change the interfaces (lines 7-20):

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
  /** session_ids with at least one pending scientia_harness_issues row. */
  pendingIssueSessionIds?: Set<string>;
}
```

Change the row JSX (lines 86-105) to accept the new prop and render a dot:

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

And destructure the new prop where the component's function signature is (find `export function ChatSessionRail(...)` at the top of the render function and add `pendingIssueSessionIds` to its destructured props).

- [ ] **Step 2: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: FAIL if `pendingIssueSessionIds` isn't threaded from wherever `<ChatSessionRail ... />` is actually rendered (this plan's grounding did not find that call site with certainty — see Task 18).

- [ ] **Step 3: Commit this task's change alone** (Task 18 supplies the caller-side wiring; this task is safe to land independently since the new prop is optional)

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSessionRail.tsx
git commit -m "feat(vox-gui): add pending-harness-issue badge to ChatSessionRail rows"
```

---

### Task 18: Wire the badge data + global toast polling

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (add a polling hook near the root, where `pushToast` is created — line ~319)

- [ ] **Step 1: Find the `<ChatSessionRail` call site**

Run: `grep -rn "ChatSessionRail" crates/vox-gui/ui/src --include=*.tsx | grep -v test`

This must return the file that renders `<ChatSessionRail sessions={...} ... />` — read that file to find the exact prop-passing JSX before proceeding.

- [ ] **Step 2: Add a polling hook in `App.tsx`**

Near where `pushToast` is defined (line ~319), add a `useEffect` that polls pending chat-session-sourced harness issues, diffs against previously-seen ids, toasts on new ones, and derives the `Set<string>` for the session badge:

```tsx
  const [pendingHarnessIssueSessionIds, setPendingHarnessIssueSessionIds] = useState<Set<string>>(new Set());
  const seenHarnessIssueIdsRef = useRef<Set<number>>(new Set());

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
        for (const issue of pending) {
          if (!seenHarnessIssueIdsRef.current.has(issue.id)) {
            seenHarnessIssueIdsRef.current.add(issue.id);
            pushToast({
              tone: 'warn',
              title: 'Harness issue detected',
              body: issue.summary,
              cause: 'harness-issue-detected',
            });
          }
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

(Add `useRef` to the existing `react` import at the top of `App.tsx` if not already imported.)

- [ ] **Step 3: Thread `pendingHarnessIssueSessionIds` to the `<ChatSessionRail>` call site found in Step 1**

Add `pendingIssueSessionIds={pendingHarnessIssueSessionIds}` to that JSX.

- [ ] **Step 4: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

- [ ] **Step 5: Manual verification**

Run: `pnpm --dir crates/vox-gui/ui build` then start the app via the project's normal dev flow and confirm no console errors from the new polling effect (there is no live backend data yet to see a real toast, but the polling call should complete without throwing).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(vox-gui): poll pending chat-session harness issues, toast + badge on new ones"
```

---

### Task 19: Inline transcript marker (read-time merge)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx`

- [ ] **Step 1: Add a session-scoped fetch + splice into the render**

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
    listHarnessIssuesForSession(sessionId)
      .then((rows) => {
        if (!cancelled) setHarnessIssues(rows);
      })
      .catch(() => {
        if (!cancelled) setHarnessIssues([]);
      });
    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  if (timeline.length === 0 && harnessIssues.length === 0) return null;

  type MergedRow =
    | { kind: 'timeline'; atMs: number; row: (typeof timeline)[number] }
    | { kind: 'harness-issue'; atMs: number; issue: HarnessIssueRow };

  const merged: MergedRow[] = [
    ...timeline.map((row) => ({ kind: 'timeline' as const, atMs: row.atMs ?? 0, row })),
    ...harnessIssues.map((issue) => ({
      kind: 'harness-issue' as const,
      atMs: issue.detected_at_ms,
      issue,
    })),
  ].sort((a, b) => a.atMs - b.atMs);

  return (
    <Glass
      role="log"
      aria-live="polite"
      aria-relevant="additions text"
      aria-label="Chat transcript"
      className="mb-3 min-h-0 flex-1 overflow-y-auto custom-scrollbar p-3 pb-6"
    >
      <div className="mx-auto flex w-full max-w-[900px] flex-col gap-2">
        {merged.map((m) => {
          if (m.kind === 'harness-issue') {
            return (
              <div
                key={`harness-issue-${m.issue.id}`}
                data-testid={`transcript-harness-issue-${m.issue.id}`}
                className="self-center rounded border border-amber-400/30 bg-amber-400/[0.08] px-2 py-1 text-center text-[10px] text-amber-300"
              >
                Issue detected: {m.issue.summary}
              </div>
            );
          }
          const row = m.row;
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

- [ ] **Step 2: Thread `sessionId` from `ChatTranscript`'s caller**

Run: `grep -rn "<ChatTranscript" crates/vox-gui/ui/src --include=*.tsx | grep -v test`

Add a `sessionId={<the active session id in scope there>}` prop at that call site — the exact variable name depends on that parent component, which must be read first.

- [ ] **Step 3: Type-check**

Run: `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatTranscript.tsx
git commit -m "feat(vox-gui): splice detected harness issues into the chat transcript by timestamp"
```

---

### Task 20: End-to-end Playwright spec

**Files:**
- Create: `crates/vox-gui/ui/e2e/harness-issue-discovery.spec.ts` (or wherever existing `gui-playwright-smoke` specs live — check `crates/vox-gui/ui` for the existing e2e directory convention before creating this path)

- [ ] **Step 1: Locate the existing e2e convention**

Run: `find crates/vox-gui/ui -iname "*.spec.ts" | head -5` (or `Get-ChildItem -Recurse -Filter *.spec.ts crates/vox-gui/ui` on Windows) to find the real directory and one existing spec to copy the `test.describe`/fixture/mock-Tauri-invoke setup from.

- [ ] **Step 2: Write the spec**

Using whatever mocking pattern the existing specs use for `@tauri-apps/api/core`'s `invoke` (mock `list_harness_issues` to return one pending issue, `scan_training_corpus` to return a count, `record_harness_issue_decision` to resolve), write a spec that:
1. Navigates to the Scientia surface, clicks the "Harness Issues" tab.
2. Asserts the mocked pending issue's summary text is visible.
3. Clicks "Confirm & propose fix", asserts `record_harness_issue_decision` and `propose_harness_issue_fix` were called with the right issue id.
4. Clicks "Scan training corpus", asserts a toast with the mocked count appears.

Follow the exact assertion/mock style from the file found in Step 1 rather than inventing a new one — this keeps the spec consistent with `gui-playwright-smoke` conventions referenced in the design.

- [ ] **Step 3: Run it**

Run: whatever command the existing e2e specs use (check `package.json` scripts in `crates/vox-gui/ui`, likely `pnpm --dir crates/vox-gui/ui test:e2e -- harness-issue-discovery`)
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/e2e/harness-issue-discovery.spec.ts
git commit -m "test(vox-gui): add e2e spec for harness issue discovery review flow"
```

---

## Final verification

- [ ] Run the full backend test suite: `cargo test -p vox-db -p vox-orchestrator -p vox-orchestrator-mcp -p vox-gui`
- [ ] Run the frontend test suite: `pnpm --dir crates/vox-gui/ui test`
- [ ] Run `pnpm --dir crates/vox-gui/ui exec tsc --noEmit`
- [ ] Run `vox ci check-codex-ssot` (or whatever the project's schema-SSOT gate command is — check `AGENTS.md`) to confirm the `BASELINE_VERSION`/digest bump from Task 1 satisfies CI, not just the local test.
- [ ] Run `cargo clippy -p vox-db -p vox-orchestrator -p vox-orchestrator-mcp -p vox-gui -- -D warnings` (per this project's admin-merge convention of verifying clippy locally before merge).
