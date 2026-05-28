//! Telemetry recorders that feed the CR-L8 corpus-feedback aggregator.
//!
//! Two implementations of [`TelemetryRecorder`] live here:
//!
//! - [`BufferedRecorder`] — in-memory `Vec<TelemetryEvent>`, suitable for
//!   tests and short-lived measurement runs where the aggregator runs in the
//!   same process as the emitters.
//! - [`JsonlFileRecorder`] — appends one JSON line per event to a file under
//!   `contracts/reports/corpus-feedback-events/<date>.jsonl`. The aggregator
//!   replays these files in a later quarterly pipeline run via [`load_events_from_jsonl`].
//!
//! Council-ratified 2026-05-15 (CR-L8 P2.2).

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use vox_telemetry::{TelemetryEvent, TelemetryRecorder};

/// Default on-disk events directory under the workspace root.
pub const DEFAULT_EVENTS_DIR: &str = "contracts/reports/corpus-feedback-events";

/// Default report destination directory under the workspace root.
pub const DEFAULT_REPORT_DIR: &str = "contracts/reports/corpus-feedback";

/// In-memory recorder used by tests and short-lived processes.
#[derive(Default)]
pub struct BufferedRecorder {
    events: Mutex<Vec<TelemetryEvent>>,
}

impl BufferedRecorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the current event buffer (clones the vector).
    pub fn snapshot(&self) -> Vec<TelemetryEvent> {
        self.events.lock().expect("buffered-recorder mutex").clone()
    }

    /// Drain the buffer, returning all captured events and emptying internal storage.
    pub fn drain(&self) -> Vec<TelemetryEvent> {
        std::mem::take(&mut *self.events.lock().expect("buffered-recorder mutex"))
    }

    pub fn len(&self) -> usize {
        self.events.lock().expect("buffered-recorder mutex").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl TelemetryRecorder for BufferedRecorder {
    fn record(&self, event: &TelemetryEvent) {
        self.events
            .lock()
            .expect("buffered-recorder mutex")
            .push(event.clone());
    }
}

/// Append-only JSON-lines recorder for cross-process / cross-run aggregation.
///
/// Each `record` call atomically appends one line of `serde_json::to_string`
/// output to the configured file. Read back via [`load_events_from_jsonl`].
pub struct JsonlFileRecorder {
    path: PathBuf,
    /// Inner mutex serializes appends across threads. The OS file-handle
    /// is opened per-write to keep the recorder cheap when not in use.
    lock: Mutex<()>,
}

impl JsonlFileRecorder {
    /// Construct a recorder pointing at `path`. The parent directory is created
    /// lazily on first write.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TelemetryRecorder for JsonlFileRecorder {
    fn record(&self, event: &TelemetryEvent) {
        // Best-effort: failures (full disk, permissions) silently drop the
        // event to avoid panicking through a `record_event!` call site. CI
        // failures should be observable via the freshness check on the
        // resulting report artifact.
        let Ok(_guard) = self.lock.lock() else { return };
        let Some(parent) = self.path.parent() else {
            return;
        };
        if let Err(_e) = std::fs::create_dir_all(parent) {
            return;
        }
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        else {
            return;
        };
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

/// Load a JSONL events file into a `Vec<TelemetryEvent>`.
///
/// Malformed lines are skipped (with no error returned) so partial-write
/// corruption from a crashed emitter doesn't break the quarterly aggregator.
/// Returns `Ok(vec![])` when the file does not exist (a fresh checkout).
pub fn load_events_from_jsonl(path: &Path) -> std::io::Result<Vec<TelemetryEvent>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let events = contents
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<TelemetryEvent>(line).ok())
        .collect();
    Ok(events)
}

/// Load every `*.jsonl` file under a directory and concatenate into one event
/// vector. Order matches the lexicographic order of filenames (which sorts
/// `YYYY-MM-DD.jsonl` chronologically).
pub fn load_events_from_dir(dir: &Path) -> std::io::Result<Vec<TelemetryEvent>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .collect();
    paths.sort();
    let mut events = Vec::new();
    for path in paths {
        let mut chunk = load_events_from_jsonl(&path)?;
        events.append(&mut chunk);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vox_telemetry::LintFindingEvent;

    fn finding(rule: &str) -> TelemetryEvent {
        TelemetryEvent::LintFinding(LintFindingEvent {
            rule_id: rule.into(),
            diagnostic_id: None,
            severity: "warning".into(),
            relative_path: "x.vox".into(),
            line: 1,
            autofix_available: false,
            confidence: None,
            repository_id: None,
        })
    }

    #[test]
    fn buffered_recorder_captures_events() {
        let r = BufferedRecorder::new();
        r.record(&finding("R1"));
        r.record(&finding("R2"));
        let snap = r.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn buffered_recorder_drain_empties_buffer() {
        let r = BufferedRecorder::new();
        r.record(&finding("R1"));
        let drained = r.drain();
        assert_eq!(drained.len(), 1);
        assert!(r.is_empty());
    }

    #[test]
    fn jsonl_recorder_appends_and_load_round_trips() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("events.jsonl");
        let r = JsonlFileRecorder::new(&path);
        r.record(&finding("R1"));
        r.record(&finding("R2"));
        r.record(&finding("R1"));
        let loaded = load_events_from_jsonl(&path).expect("load");
        assert_eq!(loaded.len(), 3);
        let rules: Vec<&str> = loaded
            .iter()
            .filter_map(|e| match e {
                TelemetryEvent::LintFinding(p) => Some(p.rule_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(rules, vec!["R1", "R2", "R1"]);
    }

    #[test]
    fn jsonl_recorder_creates_parent_directory_lazily() {
        let tmp = tempdir().expect("tempdir");
        // Path nested two levels deep — parent must be created lazily.
        let path = tmp.path().join("nested").join("dir").join("events.jsonl");
        let r = JsonlFileRecorder::new(&path);
        r.record(&finding("R1"));
        assert!(path.exists());
        let loaded = load_events_from_jsonl(&path).expect("load");
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn load_events_from_missing_file_returns_empty() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("does-not-exist.jsonl");
        let loaded = load_events_from_jsonl(&path).expect("load");
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_events_skips_malformed_lines_silently() {
        let tmp = tempdir().expect("tempdir");
        let path = tmp.path().join("partial.jsonl");
        let r = JsonlFileRecorder::new(&path);
        r.record(&finding("R1"));
        // Append a garbage line (simulating a crashed emitter).
        std::fs::write(&path, {
            let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
            existing.push_str("garbage not json\n");
            existing
        })
        .expect("write");
        r.record(&finding("R2"));
        let loaded = load_events_from_jsonl(&path).expect("load");
        assert_eq!(
            loaded.len(),
            2,
            "two valid events recovered, garbage skipped"
        );
    }

    #[test]
    fn load_events_from_dir_aggregates_multiple_files() {
        let tmp = tempdir().expect("tempdir");
        let dir = tmp.path();
        let r1 = JsonlFileRecorder::new(dir.join("2026-Q1.jsonl"));
        let r2 = JsonlFileRecorder::new(dir.join("2026-Q2.jsonl"));
        r1.record(&finding("R-q1"));
        r1.record(&finding("R-q1"));
        r2.record(&finding("R-q2"));
        let loaded = load_events_from_dir(dir).expect("load dir");
        assert_eq!(loaded.len(), 3);
    }

    #[test]
    fn load_events_from_missing_dir_returns_empty() {
        let tmp = tempdir().expect("tempdir");
        let dir = tmp.path().join("not-there");
        let loaded = load_events_from_dir(&dir).expect("load dir");
        assert!(loaded.is_empty());
    }
}
