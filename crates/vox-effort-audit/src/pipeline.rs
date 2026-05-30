//! Top-level `run` entry point: composes range → walk → shape → hybrid → judge → emit.
//!
//! Sequential per-commit pipeline for S1. Bounded concurrency lands in Task E3.
//!
//! Telemetry emission (`audit.effort.*` events from `vox-telemetry`) is
//! intentionally deferred — the event types exist (E1) but wiring them here
//! would add a side-effect channel that complicates the deterministic smoke
//! test. Follow-up: thread `vox_telemetry::emit_event` calls at run.started /
//! commit.judged / run.completed once E3 settles the concurrency story.

use crate::config::EffortAuditConfig;
use crate::judge::{Judge, JudgeStatus};
use crate::output::manifest::{Manifest, RangeManifest};
use crate::output::{FindingRow, JudgeMeta};
use std::path::{Path, PathBuf};

/// Summary returned to the caller after `run` finishes.
///
/// Used by the CLI (F1) to print a one-liner without re-reading `manifest.json`.
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub run_id: String,
    pub commits_in_range: u64,
    pub commits_judged: u64,
    pub commits_skipped: u64,
}

/// Run one `vox audit effort` pass.
///
/// Composition:
/// 1. Resolve the commit range (`range::resolve`).
/// 2. Walk commits, computing diffs + shape features.
/// 3. For each commit: resolve hybrid measured cost, check the per-run token
///    budget, call the judge, emit a `FindingRow` line to `findings.jsonl`.
/// 4. Render `report.md` + `manifest.json`.
///
/// `transcript_dir_override` lets tests point at a fixture transcript tree;
/// `None` means use `cfg.transcript_dir`.
pub async fn run(
    repo_path: &Path,
    out_dir: &Path,
    cfg: EffortAuditConfig,
    judge: Box<dyn Judge>,
    transcript_dir_override: Option<PathBuf>,
) -> anyhow::Result<RunSummary> {
    // Workspace `uuid` is pinned to features ["v4", "serde"] — no v7. v4 is
    // sufficient for a per-run identifier; time-ordering isn't required since
    // `run_started` is in the manifest.
    let run_id = uuid::Uuid::new_v4().to_string();
    let started = chrono::Utc::now();

    // 1. range → walk
    let range = crate::range::resolve(None, None, &cfg.default_since)?;
    let commits = crate::walk::iter_commits(repo_path, &range, cfg.max_diff_bytes)?;
    let total = commits.len() as u64;

    // 2. emit setup
    std::fs::create_dir_all(out_dir)?;
    let mut writer = crate::output::jsonl::JsonlWriter::create(&out_dir.join("findings.jsonl"))?;
    let transcript_dir = transcript_dir_override.unwrap_or_else(|| cfg.transcript_dir.clone());

    let mut judged = 0u64;
    let mut skipped = 0u64;
    let mut total_in = 0u64;
    let mut total_out = 0u64;
    let mut measured_count = 0u64;
    let mut tokens_spent = 0u64;

    let mut rows_for_report: Vec<FindingRow> = Vec::with_capacity(commits.len());

    // 3. per-commit pipeline (sequential; concurrency arrives in E3)
    for rec in &commits {
        let shape = crate::shape::features(rec);
        let cost = if cfg.with_transcripts {
            crate::hybrid::transcripts::resolve_for_commit(
                &transcript_dir,
                repo_path,
                rec.commit_ts,
                chrono::Duration::minutes(10),
            )
        } else {
            crate::hybrid::MeasuredCost::Unavailable
        };
        if matches!(cost, crate::hybrid::MeasuredCost::Measured { .. }) {
            measured_count += 1;
        }

        // Budget check: once we've burned the configured total, mark every
        // remaining commit as Skipped(BudgetExhausted) without calling the judge.
        if tokens_spent >= cfg.judge.max_total_tokens {
            let meta = JudgeMeta {
                model_id: judge.model_id().to_string(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Skipped".into(),
            };
            let row = build_row(rec, &shape, &cost, &meta, None);
            writer.append(&row)?;
            rows_for_report.push(row);
            skipped += 1;
            continue;
        }

        let outcome = judge.judge_one(rec, &shape).await;
        total_in += outcome.input_tokens;
        total_out += outcome.output_tokens;
        tokens_spent += outcome.input_tokens + outcome.output_tokens;

        let meta = JudgeMeta {
            model_id: outcome.model_id.clone(),
            latency_ms: outcome.latency_ms,
            judge_input_tokens: outcome.input_tokens,
            judge_output_tokens: outcome.output_tokens,
            outcome: match &outcome.status {
                JudgeStatus::Judged => "Judged".into(),
                JudgeStatus::Failed(_) => "Failed".into(),
                JudgeStatus::Skipped(_) => "Skipped".into(),
            },
        };
        let row = build_row(rec, &shape, &cost, &meta, outcome.finding);
        writer.append(&row)?;
        rows_for_report.push(row);
        match outcome.status {
            JudgeStatus::Judged => judged += 1,
            _ => skipped += 1,
        }
    }

    // 4. report + manifest
    std::fs::write(
        out_dir.join("report.md"),
        crate::output::markdown::render(&rows_for_report, cfg.report_top_n),
    )?;

    let manifest = Manifest {
        schema_version: "1.0".into(),
        run_id: run_id.clone(),
        run_started: started,
        run_completed: chrono::Utc::now(),
        vox_version: env!("CARGO_PKG_VERSION").into(),
        effort_audit_crate_version: env!("CARGO_PKG_VERSION").into(),
        range: RangeManifest {
            since: cfg.default_since.clone(),
            until: "HEAD".into(),
            // Walker yields newest-first: index 0 is the newest commit (resolved
            // `until`), the last entry is the oldest in-range commit (resolved
            // `since`).
            resolved_since_sha: commits.last().map(|c| c.sha.clone()),
            resolved_until_sha: commits.first().map(|c| c.sha.clone()),
        },
        commits_in_range: total,
        commits_judged: judged,
        commits_skipped: skipped,
        judge_model_id_resolved: judge.model_id().to_string(),
        judge_total_input_tokens: total_in,
        judge_total_output_tokens: total_out,
        // S1 leaves USD as 0.0 — cost computation lands with S3 pricing tables.
        judge_total_estimated_usd: 0.0,
        hybrid_coverage_percent: if total > 0 {
            (measured_count as f64 / total as f64) * 100.0
        } else {
            0.0
        },
    };
    crate::output::manifest::write(&out_dir.join("manifest.json"), &manifest)?;

    Ok(RunSummary {
        run_id,
        commits_in_range: total,
        commits_judged: judged,
        commits_skipped: skipped,
    })
}

fn build_row(
    rec: &crate::walk::CommitRecord,
    shape: &crate::shape::ShapeFeatures,
    cost: &crate::hybrid::MeasuredCost,
    judge: &JudgeMeta,
    finding: Option<crate::judge::schema::JudgeFinding>,
) -> FindingRow {
    FindingRow {
        schema_version: "1.0".into(),
        commit_sha: rec.sha.clone(),
        parent_sha: rec.parent_sha.clone(),
        commit_ts: rec.commit_ts,
        author_email_sha256: rec.author_email_sha256.clone(),
        // S1 does not resolve branch hints; placeholder until F1 wires `--branch`.
        branch_hint: "main".into(),
        message_first_line: rec.message.lines().next().unwrap_or("").to_string(),
        shape: shape.clone(),
        cost: cost.clone(),
        judge: judge.clone(),
        finding,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_summary_constructs_and_clones() {
        let s = RunSummary {
            run_id: "abc".into(),
            commits_in_range: 3,
            commits_judged: 2,
            commits_skipped: 1,
        };
        let s2 = s.clone();
        assert_eq!(s2.commits_in_range, 3);
        assert_eq!(s2.commits_judged, 2);
        assert_eq!(s2.commits_skipped, 1);
        // Debug derive smoke
        let dbg = format!("{s2:?}");
        assert!(dbg.contains("RunSummary"));
    }
}
