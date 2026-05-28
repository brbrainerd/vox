//! `vox scientia scout` — Phase F handler.
//!
//! Wraps `vox-scientia-producers`'s [`ProducerRegistry`] in a single command
//! that surveys the current workspace, persists new candidates to
//! `scientia_finding_candidates`, and renders a report.

use anyhow::{Context, Result};
use vox_cli_core::scientia::ScoutOutput;
use vox_db::store::{FindingCandidateClass, FindingCandidateRow, InsertOutcome};
use vox_db::VoxDb;
use vox_research_events::ResearchEvent;
use vox_scientia::producers::{ProducerContext, ProducerRegistry};

/// Entry point invoked from `commands::scientia`.
pub async fn run(
    commit_window: usize,
    days_window: u32,
    output: ScoutOutput,
    candidate_class: Option<String>,
) -> Result<()> {
    let codex = VoxDb::connect_default()
        .await
        .context("connect to default Codex / VoxDb")?;

    let repo_root = std::env::current_dir().context("current_dir")?;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let session_id = format!("scout-{now_ms}");

    let ctx = ProducerContext {
        repo_root,
        commit_window,
        days_window,
        now_ms,
        session_id: session_id.clone(),
        repository_id: None,
    };

    let registry = ProducerRegistry::default_with_codex(codex.clone());
    let events = registry.run_all(&ctx).await;

    let inserted = persist_new_candidates(&codex, &events, now_ms).await?;

    let filtered: Vec<&ResearchEvent> = events
        .iter()
        .filter(|e| {
            matches_class_filter(e, candidate_class.as_deref())
        })
        .collect();

    match output {
        ScoutOutput::Table => {
            print_table(&filtered);
            eprintln!(
                "scout: {} candidate(s) shown ({} producers ran; {} new persisted)",
                filtered.len(),
                registry.len(),
                inserted,
            );
        }
        ScoutOutput::Json => print_json(&filtered)?,
    }
    Ok(())
}

/// Persist `FindingCandidateProposed` events as `scientia_finding_candidates`
/// rows. Returns the count of newly inserted rows (idempotent re-runs return 0).
async fn persist_new_candidates(
    codex: &VoxDb,
    events: &[ResearchEvent],
    now_ms: i64,
) -> Result<usize> {
    let mut inserted = 0;
    for ev in events {
        if let ResearchEvent::FindingCandidateProposed {
            finding_id,
            worthiness_score,
            ..
        } = ev
        {
            let class = class_from_finding_id(finding_id);
            let row = FindingCandidateRow {
                candidate_id: finding_id.clone(),
                candidate_class: class,
                publication_id: None,
                title_hint: None,
                internal_signals_json: "[]".into(),
                novelty_evidence_bundle_id: None,
                worthiness_decision_ref: None,
                confidence_json: Some(format!(
                    r#"{{"signal_strength":{}}}"#,
                    sane_float(*worthiness_score)
                )),
                repository_id: None,
                producer_name: producer_from_finding_id(finding_id).to_string(),
                signal_fingerprint: finding_id.clone(),
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
            };
            match codex.insert_finding_candidate(&row).await {
                Ok(InsertOutcome::Inserted) => inserted += 1,
                Ok(InsertOutcome::AlreadySeen) => {}
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "scout: insert_finding_candidate({}): {e}",
                        finding_id
                    ));
                }
            }
        }
    }
    Ok(inserted)
}

fn matches_class_filter(ev: &ResearchEvent, class_filter: Option<&str>) -> bool {
    let Some(filter) = class_filter else {
        return true;
    };
    match ev {
        ResearchEvent::FindingCandidateProposed { finding_id, .. } => {
            class_from_finding_id(finding_id).as_sql() == filter
        }
        _ => false,
    }
}

/// Map a finding_id prefix back to its `FindingCandidateClass`. Prefixes are
/// minted by producers in `vox-scientia-producers` (see the per-producer
/// `id_prefix()` / format strings).
fn class_from_finding_id(id: &str) -> FindingCandidateClass {
    if id.starts_with("algimp-") {
        FindingCandidateClass::AlgorithmicImprovement
    } else if id.starts_with("repinf-") {
        FindingCandidateClass::ReproducibilityInfra
    } else if id.starts_with("teltr-") {
        FindingCandidateClass::TelemetryTrust
    } else if id.starts_with("polgov-") {
        FindingCandidateClass::PolicyGovernance
    } else {
        FindingCandidateClass::Other
    }
}

/// Map a finding_id to its producer name. Used for the
/// `(producer_name, signal_fingerprint)` UNIQUE index in
/// `scientia_finding_candidates`.
fn producer_from_finding_id(id: &str) -> &'static str {
    // The middle token before the final sha7 distinguishes producers; for
    // now we infer from the prefix. (algimp- can come from commit_graph or
    // bench_history; the second token "bench" distinguishes the latter.)
    if id.contains("-bench-") {
        "bench_history"
    } else if id.contains("-trust-") {
        "socrates_telemetry"
    } else {
        "commit_graph"
    }
}

fn sane_float(f: f64) -> f64 {
    if f.is_finite() {
        f
    } else {
        0.0
    }
}

fn print_table(events: &[&ResearchEvent]) {
    println!(
        "{:42} {:26} {:>5}",
        "candidate-id", "class", "score"
    );
    println!("{}", "-".repeat(42 + 1 + 26 + 1 + 5));
    for ev in events {
        if let ResearchEvent::FindingCandidateProposed {
            finding_id,
            worthiness_score,
            ..
        } = ev
        {
            let class = class_from_finding_id(finding_id);
            println!(
                "{:42} {:26} {:>5.2}",
                truncate(finding_id, 42),
                class.as_sql(),
                sane_float(*worthiness_score)
            );
        }
    }
}

fn print_json(events: &[&ResearchEvent]) -> Result<()> {
    // Serialize a stripped record for stability instead of the raw enum.
    #[derive(serde::Serialize)]
    struct Row<'a> {
        candidate_id: &'a str,
        candidate_class: &'a str,
        worthiness_score: f64,
        session_id: &'a str,
    }
    let rows: Vec<Row<'_>> = events
        .iter()
        .filter_map(|e| match e {
            ResearchEvent::FindingCandidateProposed {
                finding_id,
                worthiness_score,
                session_id,
                ..
            } => Some(Row {
                candidate_id: finding_id,
                candidate_class: class_from_finding_id(finding_id).as_sql(),
                worthiness_score: sane_float(*worthiness_score),
                session_id,
            }),
            _ => None,
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finding_id_prefix_maps_back_to_class() {
        assert_eq!(
            class_from_finding_id("algimp-2026-05-15-deadbeef"),
            FindingCandidateClass::AlgorithmicImprovement
        );
        assert_eq!(
            class_from_finding_id("repinf-2026-05-15-deadbeef"),
            FindingCandidateClass::ReproducibilityInfra
        );
        assert_eq!(
            class_from_finding_id("teltr-2026-05-15-trust-abcd"),
            FindingCandidateClass::TelemetryTrust
        );
        assert_eq!(
            class_from_finding_id("polgov-2026-05-15-deadbeef"),
            FindingCandidateClass::PolicyGovernance
        );
        assert_eq!(
            class_from_finding_id("unknown"),
            FindingCandidateClass::Other
        );
    }

    #[test]
    fn producer_name_inferred_from_finding_id_subtoken() {
        assert_eq!(
            producer_from_finding_id("algimp-2026-05-15-bench-1234"),
            "bench_history"
        );
        assert_eq!(
            producer_from_finding_id("teltr-2026-05-15-trust-1234"),
            "socrates_telemetry"
        );
        assert_eq!(
            producer_from_finding_id("algimp-2026-05-15-abcdef0"),
            "commit_graph"
        );
    }

    #[test]
    fn class_filter_matches_only_requested_class() {
        let ev = ResearchEvent::FindingCandidateProposed {
            finding_id: "algimp-x".into(),
            claim_ids: vec![],
            worthiness_score: 0.5,
            session_id: "s".into(),
        };
        assert!(matches_class_filter(&ev, None));
        assert!(matches_class_filter(&ev, Some("algorithmic_improvement")));
        assert!(!matches_class_filter(&ev, Some("telemetry_trust")));
    }

    #[test]
    fn truncate_handles_long_strings() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("0123456789abc", 10), "012345678…");
    }
}
