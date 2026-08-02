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
                StoreError::Db(
                    "scientia_harness_issues: last_insert_rowid() returned no row".into(),
                )
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
            .query(
                sql,
                params![
                    status.map(str::to_string),
                    source.map(str::to_string),
                    limit
                ],
            )
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

        db.set_harness_issue_status(id, "confirmed")
            .await
            .expect("update");
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

        let fetched = db
            .get_harness_issue(id)
            .await
            .expect("get")
            .expect("row exists");
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
            !db.has_pending_harness_issue(
                "corpus_scan",
                "examples/golden/x.vox",
                "stale_frontmatter"
            )
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
            db.has_pending_harness_issue(
                "corpus_scan",
                "examples/golden/x.vox",
                "stale_frontmatter"
            )
            .await
            .expect("check after insert")
        );
    }
}
