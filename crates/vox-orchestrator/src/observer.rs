//! Observer sub-system for the Vox orchestrator (Wave 2, Tasks 48–65).
//!
//! The `Observer` evaluates file-level language invariants in real-time as agents
//! write code. It reports structural health (`ObservationReport`) and recommends
//! follow-up actions (`ObserverAction`) aligned with the MENS reward function.
//!
//! # Design
//! - `observe_file` / `observe_rust_file` — one-shot reports on a path
//! - `compute_action` — maps an `ObservationReport` to an `ObserverAction`
//! - `summarize` — aggregates a history window into `ObservationSummary`
//! - All public types are `Send + Sync` and cheaply cloneable

use std::collections::VecDeque;
use std::path::Path;
use std::sync::{Arc, Mutex};

use vox_db::store::types::{ObservationReport, ObserverAction};

/// Observation-level policy for controlling when the observer escalates.
#[derive(Debug, Clone)]
pub struct ObserverPolicy {
    /// Tolerated LSP error count before triggering `TriggerReplan`.
    pub max_lsp_errors: usize,
    /// Minimum parse rate (0.0–1.0) before triggering `RequestMoreEvidence`.
    pub min_parse_rate: f32,
    /// Minimum construct coverage (0.0–1.0) below which we emit a negative example.
    pub min_construct_coverage: f32,
    /// Escalate to human once the LSP error count exceeds this threshold.
    pub human_escalation_threshold: usize,
}

impl Default for ObserverPolicy {
    fn default() -> Self {
        Self {
            max_lsp_errors: 5,
            min_parse_rate: 0.80,
            min_construct_coverage: 0.60,
            human_escalation_threshold: 15,
        }
    }
}

/// Aggregated summary of an observation window for a task.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ObservationSummary {
    /// Task identifier this summary covers.
    pub task_id: String,
    /// Number of individual observations collected.
    pub observation_count: usize,
    /// Mean LSP error count across observations.
    pub mean_lsp_errors: f64,
    /// Mean parse rate across observations (0.0–1.0).
    pub mean_parse_rate: f32,
    /// Mean construct coverage across observations (0.0–1.0).
    pub mean_construct_coverage: f32,
    /// Most frequently recommended action.
    pub dominant_action: ObserverAction,
    /// Whether any observation triggered escalation.
    pub had_escalation: bool,
}

/// Ring-buffer of recent observation reports for a single task.
///
/// Capped at [`MAX_HISTORY`] to prevent unbounded memory growth.
const MAX_HISTORY: usize = 20;

/// The Observer evaluates language-level invariants for files being edited by agents.
///
/// It is intentionally stateless per-file (no cross-session memory) and only
/// accumulates `observation_history` for the current task window.
#[derive(Debug, Clone)]
pub struct Observer {
    policy: ObserverPolicy,
    /// Bounded ring buffer of recent observations (Task 58: capped at 20).
    history: Arc<Mutex<VecDeque<ObservationReport>>>,
}

impl Observer {
    /// Create a new observer with the given policy.
    pub fn new(policy: ObserverPolicy) -> Self {
        Self {
            policy,
            history: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_HISTORY))),
        }
    }

    /// Create an observer with the default policy.
    pub fn with_default_policy() -> Self {
        Self::new(ObserverPolicy::default())
    }

    /// Observe a file at `path` and produce an `ObservationReport`.
    ///
    /// This performs a lightweight structural analysis — no LLM is invoked.
    /// For Rust files, delegates to `observe_rust_file` for richer diagnostics.
    pub fn observe_file(&self, session_id: &str, task_id: &str, path: &Path) -> ObservationReport {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let report = match ext {
            "rs" => self.observe_rust_file(session_id, task_id, path),
            _ => self.observe_generic_file(session_id, task_id, path),
        };
        self.push_history(report.clone());
        report
    }

    /// Rust-specific structural check: parse errors, `todo!()`/`unimplemented!()` count,
    /// and construct coverage heuristic from line patterns.
    pub fn observe_rust_file(
        &self,
        session_id: &str,
        task_id: &str,
        path: &Path,
    ) -> ObservationReport {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = source.lines().collect();
        let total = lines.len().max(1);

        // Count TOESTUB patterns (todo!/unimplemented!/panic!)
        let toestub_lines = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                t.contains("todo!()") || t.contains("unimplemented!()") || t.contains("// TODO")
            })
            .count();

        // Parse rate: ratio of non-empty lines that don't look like dangling braces or toestubs
        let nonempty_total = lines.iter().filter(|l| !l.trim().is_empty()).count().max(1);
        let valid_lines = lines
            .iter()
            .filter(|l| {
                let t = l.trim();
                // Brace-only lines are normal in Rust; excluding them collapsed parse_rate on
                // well-formed sources (many `}` lines) and triggered spurious RequestMoreEvidence.
                !t.is_empty() && !t.contains("todo!()")
            })
            .count();
        let parse_rate = (valid_lines as f32 / nonempty_total as f32).clamp(0.0, 1.0);

        // Construct coverage: presence of fn/struct/impl/enum/trait/type keywords
        let construct_lines = lines
            .iter()
            .filter(|l| {
                let t = l.trim_start();
                t.starts_with("pub fn ")
                    || t.starts_with("fn ")
                    || t.starts_with("pub struct ")
                    || t.starts_with("struct ")
                    || t.starts_with("impl ")
                    || t.starts_with("pub enum ")
                    || t.starts_with("enum ")
                    || t.starts_with("pub trait ")
                    || t.starts_with("trait ")
                    || t.starts_with("pub type ")
                    || t.starts_with("type ")
            })
            .count();
        let construct_coverage = if total >= 5 {
            (construct_lines as f32 / (total as f32 / 10.0).max(1.0)).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let lsp_error_count = toestub_lines;

        let recommended_action =
            self.compute_action_raw(lsp_error_count, parse_rate, construct_coverage);

        let metadata = serde_json::json!({
            "ext": "rs",
            "toestub_lines": toestub_lines,
            "total_lines": total,
            "construct_lines": construct_lines,
        });

        ObservationReport {
            session_id: vox_db::DbSessionId::new(session_id),
            task_id: vox_db::DbTaskId::new(task_id),
            observed_at: chrono::Utc::now(),
            file_path: path.display().to_string(),
            lsp_error_count,
            parse_rate,
            construct_coverage,
            recommended_action,
            metadata_json: Some(metadata.to_string()),
        }
    }

    fn observe_generic_file(
        &self,
        session_id: &str,
        task_id: &str,
        path: &Path,
    ) -> ObservationReport {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = source.lines().collect();
        let total = lines.len().max(1);
        let nonempty = lines.iter().filter(|l| !l.trim().is_empty()).count();
        let parse_rate = (nonempty as f32 / total as f32).clamp(0.0, 1.0);
        let construct_coverage = parse_rate; // no language-specific analysis
        let lsp_error_count = 0;

        let recommended_action =
            self.compute_action_raw(lsp_error_count, parse_rate, construct_coverage);

        ObservationReport {
            session_id: vox_db::DbSessionId::new(session_id),
            task_id: vox_db::DbTaskId::new(task_id),
            observed_at: chrono::Utc::now(),
            file_path: path.display().to_string(),
            lsp_error_count,
            parse_rate,
            construct_coverage,
            recommended_action,
            metadata_json: None,
        }
    }

    /// Compute the recommended `ObserverAction` from a complete `ObservationReport`
    /// using the current policy (Task 57).
    pub fn compute_action(&self, report: &ObservationReport) -> ObserverAction {
        self.compute_action_raw(
            report.lsp_error_count,
            report.parse_rate,
            report.construct_coverage,
        )
    }

    fn compute_action_raw(
        &self,
        lsp_error_count: usize,
        parse_rate: f32,
        construct_coverage: f32,
    ) -> ObserverAction {
        // Priority order: most severe → least severe
        if lsp_error_count >= self.policy.human_escalation_threshold {
            return ObserverAction::EscalateToHuman;
        }
        if lsp_error_count >= self.policy.max_lsp_errors {
            return ObserverAction::TriggerReplan;
        }
        if parse_rate < self.policy.min_parse_rate {
            return ObserverAction::RequestMoreEvidence;
        }
        if construct_coverage < self.policy.min_construct_coverage {
            return ObserverAction::EmitNegativeExample;
        }
        ObserverAction::Continue
    }

    /// Push a report into the bounded history ring (Task 58: max 20 entries).
    fn push_history(&self, report: ObservationReport) {
        let mut hist = self.history.lock().expect("observer history lock");
        if hist.len() >= MAX_HISTORY {
            hist.pop_front();
        }
        hist.push_back(report);
    }

    /// Drain the accumulated `ObservationReport` ring for this observer instance.
    ///
    /// Reports are removed from the ring after draining (Task 52).
    pub fn drain_reports(&self) -> Vec<ObservationReport> {
        let mut hist = self.history.lock().expect("observer history lock");
        hist.drain(..).collect()
    }

    /// Produce an `ObservationSummary` for `task_id` over the retained history (Task 64).
    pub fn summarize(&self, task_id: &str) -> ObservationSummary {
        let hist = self.history.lock().expect("observer history lock");
        let task_reports: Vec<&ObservationReport> = hist
            .iter()
            .filter(|r| r.task_id.as_str() == task_id)
            .collect();

        let observation_count = task_reports.len();
        if observation_count == 0 {
            return ObservationSummary {
                task_id: task_id.to_string(),
                observation_count: 0,
                mean_lsp_errors: 0.0,
                mean_parse_rate: 1.0,
                mean_construct_coverage: 1.0,
                dominant_action: ObserverAction::Continue,
                had_escalation: false,
            };
        }

        let mean_lsp_errors = task_reports
            .iter()
            .map(|r| r.lsp_error_count as f64)
            .sum::<f64>()
            / observation_count as f64;
        let mean_parse_rate =
            task_reports.iter().map(|r| r.parse_rate).sum::<f32>() / observation_count as f32;
        let mean_construct_coverage = task_reports
            .iter()
            .map(|r| r.construct_coverage)
            .sum::<f32>()
            / observation_count as f32;

        // Tally actions: pick the first mode
        let mut action_counts = [0usize; 5];
        let action_index = |a: &ObserverAction| match a {
            ObserverAction::Continue => 0,
            ObserverAction::RequestMoreEvidence => 1,
            ObserverAction::TriggerReplan => 2,
            ObserverAction::EscalateToHuman => 3,
            ObserverAction::EmitNegativeExample => 4,
        };
        let all_actions = [
            ObserverAction::Continue,
            ObserverAction::RequestMoreEvidence,
            ObserverAction::TriggerReplan,
            ObserverAction::EscalateToHuman,
            ObserverAction::EmitNegativeExample,
        ];
        for r in &task_reports {
            action_counts[action_index(&r.recommended_action)] += 1;
        }
        let (dom_idx, _) = action_counts
            .iter()
            .enumerate()
            .max_by_key(|&(_, c)| *c)
            .unwrap_or((0, &0));
        let dominant_action = all_actions[dom_idx];

        let had_escalation = task_reports
            .iter()
            .any(|r| r.recommended_action == ObserverAction::EscalateToHuman);

        ObservationSummary {
            task_id: task_id.to_string(),
            observation_count,
            mean_lsp_errors,
            mean_parse_rate,
            mean_construct_coverage,
            dominant_action,
            had_escalation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_rs_file(src: &str) -> NamedTempFile {
        let mut f = NamedTempFile::with_suffix(".rs").expect("tmpfile");
        f.write_all(src.as_bytes()).expect("write");
        f
    }

    #[test]
    fn compute_action_continue_when_clean() {
        let obs = Observer::with_default_policy();
        let action = obs.compute_action_raw(0, 0.95, 0.85);
        assert_eq!(action, ObserverAction::Continue);
    }

    #[test]
    fn compute_action_replan_on_lsp_errors() {
        let obs = Observer::with_default_policy();
        let action = obs.compute_action_raw(6, 0.95, 0.85);
        assert_eq!(action, ObserverAction::TriggerReplan);
    }

    #[test]
    fn compute_action_escalate_on_severe_errors() {
        let obs = Observer::with_default_policy();
        let action = obs.compute_action_raw(20, 0.90, 0.85);
        assert_eq!(action, ObserverAction::EscalateToHuman);
    }

    #[test]
    fn compute_action_more_evidence_on_low_parse() {
        let obs = Observer::with_default_policy();
        let action = obs.compute_action_raw(0, 0.50, 0.85);
        assert_eq!(action, ObserverAction::RequestMoreEvidence);
    }

    #[test]
    fn compute_action_negative_example_on_low_coverage() {
        let obs = Observer::with_default_policy();
        let action = obs.compute_action_raw(0, 0.95, 0.40);
        assert_eq!(action, ObserverAction::EmitNegativeExample);
    }

    #[test]
    fn observe_rust_file_clean_source() {
        let src = r#"
pub fn hello() -> &'static str {
    "hello"
}

pub struct Foo {
    x: i32,
}

impl Foo {
    pub fn bar(&self) -> i32 {
        self.x + 1
    }
}
"#;
        let f = make_rs_file(src);
        let obs = Observer::with_default_policy();
        let report = obs.observe_rust_file("s1", "t1", f.path());
        assert_eq!(report.task_id, "t1".into());
        assert_eq!(report.lsp_error_count, 0);
        assert_eq!(report.recommended_action, ObserverAction::Continue);
    }

    #[test]
    fn observe_rust_file_toestub_triggers_replan() {
        let src = "fn foo() {\n    todo!()\n}\nfn bar() {\n    todo!()\n}\nfn baz() {\n    todo!()\n}\nfn qux() {\n    todo!()\n}\nfn quux() {\n    todo!()\n}\nfn corge() {\n    todo!()\n}\n";
        let f = make_rs_file(src);
        let obs = Observer::with_default_policy();
        let report = obs.observe_rust_file("s1", "t1", f.path());
        assert!(report.lsp_error_count >= 5);
        assert!(matches!(
            report.recommended_action,
            ObserverAction::TriggerReplan | ObserverAction::EscalateToHuman
        ));
    }

    #[test]
    fn drain_reports_empties_history() {
        let obs = Observer::with_default_policy();
        let src = "fn x() {}\n";
        let f = make_rs_file(src);
        obs.observe_file("s1", "t1", f.path());
        obs.observe_file("s1", "t1", f.path());
        let drained = obs.drain_reports();
        assert_eq!(drained.len(), 2);
        let again = obs.drain_reports();
        assert!(again.is_empty());
    }

    #[test]
    fn history_bounded_at_max() {
        let obs = Observer::with_default_policy();
        let src = "fn x() {}\n";
        let f = make_rs_file(src);
        for _ in 0..25 {
            obs.observe_file("s", "t", f.path());
        }
        let hist = obs.history.lock().unwrap();
        assert!(hist.len() <= MAX_HISTORY);
    }

    #[test]
    fn summarize_task() {
        let obs = Observer::with_default_policy();
        let src = "fn x() {}\n";
        let f = make_rs_file(src);
        obs.observe_file("s1", "task-42", f.path());
        obs.observe_file("s1", "task-42", f.path());
        obs.observe_file("s1", "task-99", f.path()); // different task

        let summary = obs.summarize("task-42");
        assert_eq!(summary.task_id, "task-42");
        assert_eq!(summary.observation_count, 2);
    }
}
