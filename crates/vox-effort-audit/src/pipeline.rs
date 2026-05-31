//! Top-level `run` entry point: composes range → walk → shape → hybrid → judge → emit.
//!
//! Bounded-concurrency per-commit pipeline (E3): judges fan out through a
//! `FuturesUnordered` gated by a `tokio::sync::Semaphore` sized to
//! `cfg.max_concurrent`. Findings stream into `findings.jsonl` in arrival
//! order (whichever judge completes first); the markdown report re-sorts by
//! `waste_score` internally, so stream order does not affect ranking.
//!
//! Budget strategy (token cap + dollar cap): we check `tokens_spent` and the
//! real accumulated USD `cost_spent` at *task launch* time, before pushing the
//! judge future. Once either cap is hit we mark all remaining commits Skipped
//! without dispatching them. This is pessimistic — in-flight tasks may overshoot
//! a cap by up to `max_concurrent` calls' worth — which matches the spec's
//! "best-effort" budget tolerance for S1. The dollar cap (`max_dollar_cost`) is
//! enforced only when the resolved model's pricing is known; for unknown-price
//! models we fall back to the token budget alone rather than skip everything
//! against a fabricated $0.00.
//!
//! Telemetry emission (`audit.effort.*` events from `vox-telemetry`) is
//! intentionally deferred — the event types exist (E1) but wiring them here
//! would add a side-effect channel that complicates the deterministic smoke
//! test. Follow-up: thread `vox_telemetry::emit_event` calls at run.started /
//! commit.judged / run.completed once concurrency settles.

use crate::config::EffortAuditConfig;
use crate::judge::{Judge, JudgeStatus};
use crate::output::manifest::{Manifest, RangeManifest};
use crate::output::{FindingRow, JudgeMeta};
use futures::future::FutureExt;
use futures::stream::{FuturesUnordered, StreamExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;

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
///
/// CLI bookend overrides (F1):
/// - `since_override`: when `Some`, overrides `cfg.default_since` (the `--since`
///   flag). Accepts the same forms as the TOML key (`"30 days ago"`, `"7d"`,
///   git refs, dates).
/// - `until_override`: when `Some`, overrides the default `HEAD` upper bound
///   (the `--until` flag). Same form rules as `since_override`.
///
/// `cfg.limit`, when set, caps the number of *judged* commits — commits past
/// the cap are emitted with `JudgeStatus::Skipped(LimitReached)` so the
/// `manifest.json` `commits_in_range` count still reflects the full walk.
pub async fn run(
    repo_path: &Path,
    out_dir: &Path,
    cfg: EffortAuditConfig,
    judge: Box<dyn Judge>,
    transcript_dir_override: Option<PathBuf>,
    rates: crate::pricing::ModelRates,
) -> anyhow::Result<RunSummary> {
    run_with_overrides(
        repo_path,
        out_dir,
        cfg,
        judge,
        transcript_dir_override,
        None,
        None,
        rates,
    )
    .await
}

/// Same as [`run`] but accepts explicit `--since` / `--until` overrides from
/// the CLI. Internal entry-point — keeps the public [`run`] signature stable
/// for callers that don't need to wire bookend overrides (tests, library use).
pub async fn run_with_overrides(
    repo_path: &Path,
    out_dir: &Path,
    cfg: EffortAuditConfig,
    judge: Box<dyn Judge>,
    transcript_dir_override: Option<PathBuf>,
    since_override: Option<String>,
    until_override: Option<String>,
    rates: crate::pricing::ModelRates,
) -> anyhow::Result<RunSummary> {
    // Workspace `uuid` is pinned to features ["v4", "serde"] — no v7. v4 is
    // sufficient for a per-run identifier; time-ordering isn't required since
    // `run_started` is in the manifest.
    let run_id = uuid::Uuid::new_v4().to_string();
    let started = chrono::Utc::now();

    // 1. range → walk
    let range = crate::range::resolve(
        since_override.as_deref(),
        until_override.as_deref(),
        &cfg.default_since,
    )?;
    let commits = crate::walk::iter_commits(repo_path, &range, cfg.max_diff_bytes)?;
    let total = commits.len() as u64;
    // Walker yields newest-first: capture the bookend SHAs before we consume
    // `commits` into the per-task `prepared` Vec below.
    let resolved_until_sha = commits.first().map(|c| c.sha.clone());
    let resolved_since_sha = commits.last().map(|c| c.sha.clone());

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
    // Real accumulated USD cost, tracked only when the resolved model's price
    // is known. Stays `None` for unknown-price models so we never enforce a
    // dollar budget against a fabricated $0.00 (that would skip everything).
    let mut cost_spent: Option<f64> = if rates.known { Some(0.0) } else { None };
    let max_dollar_cost = cfg.judge.max_dollar_cost;

    let mut rows_for_report: Vec<FindingRow> = Vec::with_capacity(commits.len());

    // 3. per-commit pipeline: bounded concurrency.
    //
    // Wrap the judge in an `Arc` so each spawned task can hold a shared
    // reference without taking ownership. The public API still takes
    // `Box<dyn Judge>` to avoid forcing callers to construct an `Arc`.
    let judge: Arc<dyn Judge> = Arc::from(judge);
    let judge_model_id = judge.model_id().to_string();
    let max_concurrent = cfg.max_concurrent.max(1);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let max_total_tokens = cfg.judge.max_total_tokens;

    // Precompute (shape, cost) synchronously per commit — these are cheap CPU
    // work and they read filesystem state (transcripts) that we don't want to
    // race. Pair each with an arc-ed CommitRecord so the async tasks can move
    // them without lifetime gymnastics.
    let prepared: Vec<(
        Arc<crate::walk::CommitRecord>,
        crate::shape::ShapeFeatures,
        crate::hybrid::MeasuredCost,
    )> = commits
        .into_iter()
        .map(|rec| {
            let shape = crate::shape::features(&rec);
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
            (Arc::new(rec), shape, cost)
        })
        .collect();

    let mut in_flight = FuturesUnordered::new();
    // F1: count of commits we've actually pushed at the judge (so `--limit N`
    // caps real LLM spend, not just the JSONL row count — Skipped rows are
    // free).
    let mut dispatched: u64 = 0;
    let limit_cap: Option<u64> = cfg.limit.map(|n| n as u64);

    for (rec_arc, shape, cost) in prepared.into_iter() {
        // F1: --limit cap. Once we've dispatched N judges, every remaining
        // commit becomes Skipped without firing the judge. Mirrors the
        // budget-exhausted branch below but with a distinct outcome tag so
        // the manifest / report can distinguish the two paths.
        if let Some(cap) = limit_cap
            && dispatched >= cap
        {
            let meta = JudgeMeta {
                model_id: judge_model_id.clone(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Skipped".into(),
            };
            let row = build_row(&rec_arc, &shape, &cost, &meta, None);
            writer.append(&row)?;
            rows_for_report.push(row);
            skipped += 1;
            continue;
        }
        // Budget check at dispatch time: once we've burned the configured
        // cap, every remaining commit is marked Skipped(BudgetExhausted)
        // without firing the judge. In-flight tasks may still complete and
        // push `tokens_spent` past the cap by up to `max_concurrent` calls
        // worth — acceptable slop for S1.
        if tokens_spent >= max_total_tokens {
            let meta = JudgeMeta {
                model_id: judge_model_id.clone(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Skipped".into(),
            };
            let row = build_row(&rec_arc, &shape, &cost, &meta, None);
            writer.append(&row)?;
            rows_for_report.push(row);
            skipped += 1;
            continue;
        }
        // Real dollar-budget check at dispatch time, mirroring the token cap.
        // Only enforced when pricing is known (`cost_spent` is `Some`); for
        // unknown-price models we fall back to the token budget alone rather
        // than skip everything against a fake $0.00. Same in-flight overshoot
        // tolerance as the token cap applies.
        if let Some(spent) = cost_spent
            && spent >= max_dollar_cost
        {
            let meta = JudgeMeta {
                model_id: judge_model_id.clone(),
                latency_ms: 0,
                judge_input_tokens: 0,
                judge_output_tokens: 0,
                outcome: "Skipped".into(),
            };
            let row = build_row(&rec_arc, &shape, &cost, &meta, None);
            writer.append(&row)?;
            rows_for_report.push(row);
            skipped += 1;
            continue;
        }

        let judge = Arc::clone(&judge);
        let semaphore = Arc::clone(&semaphore);
        dispatched += 1;
        in_flight.push(async move {
            // Acquire a permit; held until the future drops. `semaphore` is
            // never closed, so this `expect` is unreachable in practice.
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("effort-audit semaphore closed unexpectedly");
            let outcome = judge.judge_one(&rec_arc, &shape).await;
            (rec_arc, shape, cost, outcome)
        });

        // Drain any tasks that already finished so `findings.jsonl` reflects
        // partial progress (and so we keep `tokens_spent` fresh for the next
        // dispatch decision).
        while let Some(Some((rec_arc, shape, cost, outcome))) = in_flight.next().now_or_never() {
            consume_outcome(
                &rec_arc,
                &shape,
                &cost,
                outcome,
                &mut writer,
                &mut rows_for_report,
                &mut total_in,
                &mut total_out,
                &mut tokens_spent,
                &mut cost_spent,
                &rates,
                &mut judged,
                &mut skipped,
            )?;
        }
    }

    // Drain remaining in-flight judges.
    while let Some((rec_arc, shape, cost, outcome)) = in_flight.next().await {
        consume_outcome(
            &rec_arc,
            &shape,
            &cost,
            outcome,
            &mut writer,
            &mut rows_for_report,
            &mut total_in,
            &mut total_out,
            &mut tokens_spent,
            &mut cost_spent,
            &rates,
            &mut judged,
            &mut skipped,
        )?;
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
            resolved_since_sha,
            resolved_until_sha,
        },
        commits_in_range: total,
        commits_judged: judged,
        commits_skipped: skipped,
        judge_model_id_resolved: judge_model_id,
        judge_total_input_tokens: total_in,
        judge_total_output_tokens: total_out,
        // Real cost = accumulated judge tokens × the resolved model's registry
        // pricing (passed in by the CLI). `None` when the model's price is
        // unknown — an honest "unknown", never a fabricated $0.00.
        judge_total_cost_usd: rates.cost_usd(total_in, total_out),
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

/// Apply one completed judge outcome to the streaming writer, the in-memory
/// rows-for-report buffer, and the running tallies.
#[allow(clippy::too_many_arguments)]
fn consume_outcome(
    rec: &crate::walk::CommitRecord,
    shape: &crate::shape::ShapeFeatures,
    cost: &crate::hybrid::MeasuredCost,
    outcome: crate::judge::JudgeOutcome,
    writer: &mut crate::output::jsonl::JsonlWriter,
    rows_for_report: &mut Vec<FindingRow>,
    total_in: &mut u64,
    total_out: &mut u64,
    tokens_spent: &mut u64,
    cost_spent: &mut Option<f64>,
    rates: &crate::pricing::ModelRates,
    judged: &mut u64,
    skipped: &mut u64,
) -> anyhow::Result<()> {
    *total_in += outcome.input_tokens;
    *total_out += outcome.output_tokens;
    *tokens_spent += outcome.input_tokens + outcome.output_tokens;
    // Accumulate real per-call cost when pricing is known. `cost_spent` stays
    // `None` for unknown-price models (the dollar budget is then unenforced and
    // the manifest reports `null` cost rather than a fabricated $0.00).
    if let (Some(spent), Some(call_cost)) = (
        cost_spent.as_mut(),
        rates.cost_usd(outcome.input_tokens, outcome.output_tokens),
    ) {
        *spent += call_cost;
    }

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
    let row = build_row(rec, shape, cost, &meta, outcome.finding);
    writer.append(&row)?;
    rows_for_report.push(row);
    match outcome.status {
        JudgeStatus::Judged => *judged += 1,
        _ => *skipped += 1,
    }
    Ok(())
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
