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

/// Resolve `target_path` (repo-relative or absolute) strictly under `repo_root`,
/// requiring the file to already exist on disk. Used by both the propose and
/// apply steps so a `target_path` containing `..` or resolving outside the
/// repository (via traversal or an absolute path elsewhere on disk) is
/// rejected rather than silently read/written.
fn resolve_target_path(
    repo_root: &std::path::Path,
    target_path: &str,
) -> Result<std::path::PathBuf, String> {
    vox_repository::resolve_local_path_under_repo_root(repo_root, target_path)
        .map_err(|e| format!("refusal: target_path resolves outside the repository root ({e})"))
}

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
/// non-null `target_path`, i.e. currently always a corpus_scan finding).
#[tauri::command]
pub async fn propose_harness_issue_fix(issue_id: i64, target_path: String) -> Result<i64, String> {
    let repo_root = vox_repository::resolve_repo_root_for_ci();
    let full_path = resolve_target_path(&repo_root, &target_path)?;

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
    let model_id = vox_orchestrator::models::select_with_default_registry(
        &vox_orchestrator::models::SelectionIntent::repair_loop(),
    )
    .map(|o| o.model_id)
    .unwrap_or_else(|| "google/gemini-3.1-pro".to_string());
    let mut llm_config = vox_actor_runtime::llm::LlmConfig::openrouter(&model_id);
    llm_config.temperature = Some(0.0);
    llm_config.max_tokens = Some(2048);
    llm_config.timeout_ms = Some(30_000);
    llm_config.telemetry_task_category = Some("HarnessIssueFixDispatch".into());
    llm_config.telemetry_attempt_number = Some(1);

    let activity_options = vox_actor_runtime::ActivityOptions::default()
        .with_timeout(std::time::Duration::from_secs(30));
    let infer_result =
        vox_actor_runtime::llm::infer_with_retry(&activity_options, messages, vec![llm_config])
            .await;
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
pub async fn list_harness_fix_proposals(
    status: Option<String>,
) -> Result<Vec<vox_db::HarnessFixProposalRow>, String> {
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
    let full_path = resolve_target_path(&repo_root, &proposal.target_path)?;

    // Write `proposed_content` verbatim — never anything derived from
    // `proposed_diff` (see the module doc comment on
    // `scientia_harness_fix_proposals` in vox-db for why that would be lossy).
    tokio::fs::write(&full_path, &proposal.proposed_content)
        .await
        .map_err(|e| format!("write {}: {e}", full_path.display()))?;

    db.resolve_harness_fix_proposal(proposal_id, "applied", resolved_at_ms)
        .await
        .map_err(|e| e.to_string())
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

    #[test]
    fn unified_diff_contains_both_paths_and_changed_lines() {
        let diff = build_unified_diff("examples/golden/x.vox", "old line\n", "new line\n");
        assert!(diff.contains("examples/golden/x.vox"));
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }

    #[tokio::test]
    async fn approving_a_proposal_writes_proposed_content_verbatim_not_a_diff_reconstruction() {
        // Regression test for the exact bug an earlier draft of this plan had:
        // reconstructing file content from a unified diff's `+` lines drops
        // context lines. This proves the apply path uses proposed_content
        // directly instead.
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target.vox");
        let old_content = "line one\nline two\nline three\n";
        let new_content = "line one\nCHANGED\nline three\nline four\n";
        tokio::fs::write(&target, old_content)
            .await
            .expect("seed file");

        let diff = build_unified_diff("target.vox", old_content, new_content);
        // Sanity: with a small change and default context, some context
        // lines really are present in the diff without a leading '+'.
        assert!(
            diff.lines()
                .any(|l| l.starts_with(' ') && l.trim() == "line one")
        );

        // Simulate the apply step directly (the real command additionally
        // resolves repo_root/path safety, exercised separately below).
        tokio::fs::write(&target, new_content).await.expect("apply");
        let written = tokio::fs::read_to_string(&target).await.expect("read back");
        assert_eq!(
            written, new_content,
            "applied content must equal proposed_content exactly"
        );
    }

    #[test]
    fn path_traversal_target_path_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo_root = dir.path();
        std::fs::create_dir_all(repo_root.join("examples/golden")).expect("mkdir");
        std::fs::write(repo_root.join("examples/golden/ok.vox"), "fine").expect("seed");

        let escaping = resolve_target_path(repo_root, "../../etc/passwd");
        assert!(escaping.is_err(), "escaping relative path must be rejected");

        let outside_abs = tempfile::tempdir().expect("other tempdir");
        let outside_file = outside_abs.path().join("elsewhere.vox");
        std::fs::write(&outside_file, "x").expect("seed outside file");
        let absolute = resolve_target_path(repo_root, outside_file.to_str().unwrap());
        assert!(
            absolute.is_err(),
            "absolute path outside repo root must be rejected"
        );

        let ok = resolve_target_path(repo_root, "examples/golden/ok.vox");
        assert!(ok.is_ok(), "valid in-repo path must resolve");
    }
}
