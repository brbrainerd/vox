//! Tauri commands for harness issue discovery (Phase 1): listing/deciding
//! issues, and the on-demand golden-corpus staleness scanner.

use chrono::Datelike as _;

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
        .map(|line| {
            line.trim_start()[prefix.len()..]
                .trim()
                .trim_matches('"')
                .to_string()
        })
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
pub fn check_staleness(
    path: &str,
    content: &str,
    today_ymd: (i32, u32, u32),
) -> Option<StalenessFinding> {
    if is_skipped(content) {
        return None;
    }
    let last_validated = extract_frontmatter_field(content, "last_validated")?;
    let age_days = days_since(&last_validated, today_ymd)?;
    if age_days > STALENESS_THRESHOLD_DAYS {
        Some(StalenessFinding {
            target_path: path.to_string(),
            summary: format!(
                "last_validated {last_validated} is {age_days} days old (threshold {STALENESS_THRESHOLD_DAYS})"
            ),
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
        let rel_path = vox_repository::path_relative_to_repo_root(&repo_root, &path)
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

    #[tokio::test]
    async fn repeated_scan_does_not_duplicate_a_pending_finding() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("open db");
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
        assert_eq!(
            rows.len(),
            1,
            "second scan must not duplicate the pending row"
        );
    }
}
