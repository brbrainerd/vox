//! [`CorpusFeedbackJsonlSink`] — append-only JSONL recorder for CR-L8.
//!
//! Filters the global `TelemetryEvent` stream down to the four CR-L8 variants
//! (`LintFinding`, `LintAutofix`, `RepairAttempt`, `RepairOutcome`) and appends
//! one JSON-line per event to
//! `<corpus-feedback-events-root>/<YYYY-MM-DD>.jsonl`.
//!
//! The `vox audit corpus-feedback` subcommand
//! (`crates/vox-audit/src/subcommands/corpus_feedback.rs`) reads these files
//! via `vox_audit::recorder::load_events_from_dir` and aggregates them into
//! `contracts/reports/corpus-feedback/<quarter>.json` per CR-L8 spec.
//!
//! **Path resolution.** The sink root is:
//!
//! 1. `$VOX_CORPUS_FEEDBACK_EVENTS_DIR` if set and non-empty, else
//! 2. `<cwd>/contracts/reports/corpus-feedback-events/` (matches the path the
//!    aggregator looks for under workspace root).
//!
//! **Disable switch.** Setting `$VOX_CORPUS_FEEDBACK_EVENTS_DIR=disabled`
//! (literal string) suppresses the sink entirely; useful for CI runs that
//! don't want side-effects in the corpus tree.
//!
//! Council-ratified 2026-05-15 (CR-L8 P2.1c — the last link wiring the
//! diagnostic→repair→corpus loop end-to-end).

use std::path::PathBuf;
use std::sync::Mutex;

use vox_telemetry::{TelemetryEvent, TelemetryRecorder};

/// Default directory under the current working directory.
const DEFAULT_RELATIVE_DIR: &str = "contracts/reports/corpus-feedback-events";

/// Env-var override for the events directory.
const EVENTS_DIR_ENV: &str = "VOX_CORPUS_FEEDBACK_EVENTS_DIR";

/// Sentinel value that, when set as the env-var, disables the sink entirely.
const DISABLED_SENTINEL: &str = "disabled";

/// Resolve the events root from env or CWD-relative default. Returns `None`
/// when the sentinel disables the sink.
pub fn resolve_events_root() -> Option<PathBuf> {
    match std::env::var(EVENTS_DIR_ENV) {
        Ok(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                cwd_default()
            } else if trimmed.eq_ignore_ascii_case(DISABLED_SENTINEL) {
                None
            } else {
                Some(PathBuf::from(trimmed))
            }
        }
        Err(_) => cwd_default(),
    }
}

fn cwd_default() -> Option<PathBuf> {
    std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join(DEFAULT_RELATIVE_DIR))
}

/// Compute `YYYY-QN` for the current UTC moment (e.g., `2026-Q2` for May).
/// Matches the convention in
/// `crates/vox-audit/src/subcommands/corpus_feedback.rs` so events and
/// substantive reports share a quarter index.
fn current_quarter_yyyy_qn() -> String {
    let now = chrono::Utc::now();
    let year = now.format("%Y");
    let month_one_based: u32 = now.format("%m").to_string().parse().unwrap_or(1);
    let quarter = (month_one_based.saturating_sub(1)) / 3 + 1;
    format!("{year}-Q{quarter}")
}

/// True iff this event variant participates in CR-L8 aggregation.
fn is_corpus_feedback_event(event: &TelemetryEvent) -> bool {
    matches!(
        event,
        TelemetryEvent::LintFinding(_)
            | TelemetryEvent::LintAutofix(_)
            | TelemetryEvent::RepairAttempt(_)
            | TelemetryEvent::RepairOutcome(_)
    )
}

/// `TelemetryRecorder` sink that appends CR-L8 events to a daily JSONL file.
pub struct CorpusFeedbackJsonlSink {
    root: PathBuf,
    /// Serializes appends across threads to prevent interleaved writes (one
    /// process; cross-process is best-effort and tolerates malformed lines
    /// per `vox_audit::recorder::load_events_from_jsonl`).
    lock: Mutex<()>,
}

impl CorpusFeedbackJsonlSink {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn write_event(&self, event: &TelemetryEvent) {
        let Ok(line) = serde_json::to_string(event) else {
            tracing::debug!("CorpusFeedbackJsonlSink: serde_json::to_string failed");
            return;
        };
        let Ok(_guard) = self.lock.lock() else {
            tracing::debug!("CorpusFeedbackJsonlSink: lock poisoned");
            return;
        };
        if let Err(err) = std::fs::create_dir_all(&self.root) {
            tracing::debug!(?err, "CorpusFeedbackJsonlSink: create_dir_all failed");
            return;
        }
        // A13 (ratified 2026-05-15): per-quarter filename keeps file count
        // bounded. The CR-L8 aggregator writes its substantive report at the
        // same quarter granularity (`<YYYY-QN>.json`), so events sit alongside
        // their report. Per-day rotation would create ~30× more JSONL files
        // over a single quarter; this matters for sinks that survive months.
        let file_name = format!("{}.jsonl", current_quarter_yyyy_qn());
        let path = self.root.join(file_name);
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        else {
            tracing::debug!("CorpusFeedbackJsonlSink: open failed at {}", path.display());
            return;
        };
        use std::io::Write;
        if let Err(err) = writeln!(file, "{line}") {
            tracing::debug!(?err, "CorpusFeedbackJsonlSink: writeln failed");
        }
    }
}

impl TelemetryRecorder for CorpusFeedbackJsonlSink {
    fn record(&self, event: &TelemetryEvent) {
        if !is_corpus_feedback_event(event) {
            return;
        }
        self.write_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_telemetry::{
        LintFindingEvent, ModelCallEvent, RepairOutcomeEvent,
    };

    fn lint_event() -> TelemetryEvent {
        TelemetryEvent::LintFinding(LintFindingEvent {
            rule_id: "rule/x".into(),
            diagnostic_id: None,
            severity: "warning".into(),
            relative_path: "x.vox".into(),
            line: 1,
            autofix_available: false,
            confidence: None,
            repository_id: None,
        })
    }

    fn repair_outcome_event() -> TelemetryEvent {
        TelemetryEvent::RepairOutcome(RepairOutcomeEvent {
            final_state: "success".into(),
            attempts_used: 1,
            attempts_budget: 3,
            total_cost_usd: 0.0,
            total_duration_ms: 100,
            residual_diagnostics: 0,
            note: None,
            repository_id: None,
        })
    }

    fn model_call_event() -> TelemetryEvent {
        TelemetryEvent::ModelCall(ModelCallEvent {
            model: "x".into(),
            provider: "x".into(),
            route_profile: None,
            prompt_tokens: 0,
            completion_tokens: 0,
            cache_read_input_tokens: None,
            cache_creation_input_tokens: None,
            latency_ms: 0,
            cost_usd: 0.0,
            cost_source: "x".into(),
            error_class: None,
            retry_attempt: 0,
            task_id: None,
            parent_task_id: None,
            trace_id: None,
            caller_agent_id: None,
        })
    }

    #[test]
    fn is_corpus_feedback_event_recognizes_the_four_cr_l8_variants() {
        assert!(is_corpus_feedback_event(&lint_event()));
        assert!(is_corpus_feedback_event(&repair_outcome_event()));
        // ModelCall is NOT a CR-L8 variant.
        assert!(!is_corpus_feedback_event(&model_call_event()));
    }

    #[test]
    fn sink_appends_cr_l8_events_only_to_jsonl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = CorpusFeedbackJsonlSink::new(tmp.path());

        sink.record(&lint_event());
        sink.record(&model_call_event()); // filtered out
        sink.record(&repair_outcome_event());

        // A13: per-quarter filename (was per-day).
        let path = tmp.path().join(format!("{}.jsonl", current_quarter_yyyy_qn()));
        assert!(path.exists(), "JSONL should exist at {}", path.display());

        let contents = std::fs::read_to_string(&path).expect("read");
        let line_count = contents.lines().filter(|l| !l.trim().is_empty()).count();
        assert_eq!(line_count, 2, "two CR-L8 events expected; got {line_count}");
        assert!(contents.contains("lint_finding"));
        assert!(contents.contains("repair_outcome"));
        assert!(!contents.contains("model_call"));
    }

    #[test]
    fn current_quarter_yyyy_qn_returns_well_formed_label() {
        let q = current_quarter_yyyy_qn();
        // Format must be YYYY-Q{1..4}.
        assert!(
            q.starts_with("20") && q.contains("-Q"),
            "expected `YYYY-QN` shape; got {q}"
        );
        let q_num: u32 = q
            .rsplit_once("-Q")
            .map(|(_, n)| n.parse().unwrap_or(0))
            .unwrap_or(0);
        assert!(
            (1..=4).contains(&q_num),
            "quarter number must be in 1..=4; got {q_num}"
        );
    }

    #[test]
    fn sink_creates_directory_lazily() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("deeply").join("nested");
        let sink = CorpusFeedbackJsonlSink::new(&nested);
        sink.record(&lint_event());
        assert!(nested.exists(), "nested dir should be created lazily");
    }

    #[test]
    fn disabled_sentinel_resolves_to_none() {
        // SAFETY: tests are single-threaded with respect to this env var; we
        // restore it after the assertion.
        let prior = std::env::var(EVENTS_DIR_ENV).ok();
        // SAFETY: see above.
        unsafe { std::env::set_var(EVENTS_DIR_ENV, "disabled") };
        let resolved = resolve_events_root();
        assert!(resolved.is_none(), "sentinel `disabled` must disable the sink");
        // SAFETY: see above.
        unsafe {
            match prior {
                Some(v) => std::env::set_var(EVENTS_DIR_ENV, v),
                None => std::env::remove_var(EVENTS_DIR_ENV),
            }
        }
    }

    #[test]
    fn explicit_env_path_overrides_cwd_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prior = std::env::var(EVENTS_DIR_ENV).ok();
        // SAFETY: see above.
        unsafe { std::env::set_var(EVENTS_DIR_ENV, tmp.path().to_str().unwrap()) };
        let resolved = resolve_events_root().expect("resolved");
        assert_eq!(resolved, tmp.path());
        // SAFETY: see above.
        unsafe {
            match prior {
                Some(v) => std::env::set_var(EVENTS_DIR_ENV, v),
                None => std::env::remove_var(EVENTS_DIR_ENV),
            }
        }
    }
}
