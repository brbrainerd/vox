//! [`SpoolSink`] — serializes `TelemetryEvent` to the local upload queue.
//!
//! Only S0–S1 events should reach this sink by default. The spool is drained
//! by `vox telemetry upload` when the user has configured upload credentials
//! (ADR 023).

use std::path::PathBuf;

use vox_telemetry::{TelemetryEvent, TelemetryRecorder};
use vox_telemetry_otlp::{otlp_json::to_otlp_log, project::project_event, redact::redact_event};

/// `TelemetryRecorder` sink that writes events as JSON files to the local spool.
///
/// `record` spawns a tokio task to avoid holding an async executor during file I/O.
/// Errors are logged at DEBUG (spool failure is non-critical).
pub struct SpoolSink {
    root: PathBuf,
}

impl SpoolSink {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

impl TelemetryRecorder for SpoolSink {
    fn record(&self, event: &TelemetryEvent) {
        // Two-layer privacy pipeline (redact-before-spool — spec §3):
        //   1. project_event: variant→(category, flat_map), drops free-form strings.
        //   2. redact_event: taxonomy-allowlist guard, drops fields not in contract.
        // If either layer returns None, the event is silently dropped (not spooled).
        // The spool ONLY ever holds clean OTLP-shaped JSON — no raw events.
        let Some((category, flat_map)) = project_event(event) else {
            return;
        };
        let Some(record) = redact_event(&category, &flat_map) else {
            return;
        };
        let install_id = vox_telemetry::config::install_id();
        let otlp_json = to_otlp_log(&record, &install_id);

        let root = self.root.clone();
        // Defensive: `tokio::spawn` panics when called outside a Tokio runtime.
        // Most `vox` codepaths run inside one, but tests and sync utility binaries
        // may not. Falling back to a blocking write keeps record_event! panic-free.
        match tokio::runtime::Handle::try_current() {
            Ok(_) => {
                tokio::spawn(async move {
                    if let Err(err) = crate::telemetry_spool::enqueue(&root, &otlp_json) {
                        tracing::debug!(?err, "SpoolSink: enqueue failed");
                    }
                });
            }
            Err(_) => {
                if let Err(err) = crate::telemetry_spool::enqueue(&root, &otlp_json) {
                    tracing::debug!(?err, "SpoolSink: enqueue failed (sync)");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_telemetry::{LintFindingEvent, TelemetryEvent};

    #[test]
    fn spool_sink_is_recorder() {
        fn _assert_recorder<T: vox_telemetry::TelemetryRecorder>() {}
        _assert_recorder::<SpoolSink>();
    }

    fn sample_event() -> TelemetryEvent {
        TelemetryEvent::LintFinding(LintFindingEvent {
            rule_id: "rule/x".into(),
            diagnostic_id: None,
            severity: "info".into(),
            relative_path: "x.vox".into(),
            line: 1,
            autofix_available: false,
            confidence: None,
            repository_id: None,
        })
    }

    /// Regression: prior to the A5 guard, `SpoolSink::record` panicked when
    /// invoked outside a Tokio runtime ("there is no reactor running").
    /// Verify the sync fallback path does not panic.
    #[test]
    fn record_does_not_panic_without_tokio_runtime() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = SpoolSink::new(tmp.path().to_path_buf());
        sink.record(&sample_event());
        // Sync fallback enqueues to the spool directory; verify the side effect.
        let pending = tmp.path().join("pending");
        assert!(
            pending.exists(),
            "sync fallback should have created the pending/ subdirectory at {}",
            pending.display()
        );
    }

    /// When a Tokio runtime IS available, the recorder spawns an async task
    /// instead of blocking. Verify both paths reach the same final state by
    /// running enough time for the spawned task to complete.
    #[tokio::test(flavor = "current_thread")]
    async fn record_spawns_inside_tokio_runtime() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let sink = SpoolSink::new(tmp.path().to_path_buf());
        sink.record(&sample_event());
        // Yield twice so the spawned task gets scheduled to completion.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        let pending = tmp.path().join("pending");
        assert!(
            pending.exists(),
            "async spawn should have created the pending/ subdirectory at {}",
            pending.display()
        );
    }
}
