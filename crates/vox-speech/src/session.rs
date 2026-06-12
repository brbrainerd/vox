//! Session-oriented Oratio contracts for CLI / MCP voice workflows.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::refine::{CorrectionContext, CorrectionTrace, OratioCorrectionProfile};
use crate::runtime_config::OratioRuntimeConfig;
use crate::transcribe_path_detailed;

/// Capture/session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    /// Session has not started capture yet.
    Idle,
    /// Session is actively capturing or waiting for capture completion.
    Capturing,
    /// Session is finalizing transcription and refinement.
    Finalizing,
    /// Session completed successfully.
    Completed,
    /// Session terminated due to timeout, cancellation, or internal failure.
    Failed,
}

/// Machine-readable timeout / deadline taxonomy for diagnostics (CLI + MCP).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OratioDeadlineTaxonomy {
    /// Enter / confirm UX gate timed out (caller responsibility; recorded for parity).
    CaptureTimeout,
    /// Transcription / inference exceeded `inference_deadline_ms` (post-hoc check).
    InferenceTimeout,
    /// Whole session exceeded `max_duration_ms`.
    DeadlineExceeded,
    /// Under limits.
    Ok,
}

/// Structured deadline diagnostics (phase + elapsed + limits).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeadlineDiagnostics {
    /// Phase that triggered or completed (`transcribe`, `refine`, `session_total`, ...).
    pub phase: String,
    /// Elapsed wall time for that phase in ms.
    pub elapsed_ms: u128,
    /// Limit that applies to the phase (for `Ok`, this is the inference or session cap).
    pub limit_ms: u64,
    /// Taxonomy bucket for this check.
    pub taxonomy: OratioDeadlineTaxonomy,
}

/// Config shared across CLI and MCP sessionized transcription flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OratioSessionConfig {
    /// UX / MCP `timeout_ms`: inactivity budget for Enter-or-timeout gates.
    pub timeout_ms: u64,
    /// Hard maximum duration for a capture/transcribe session (wall clock, post-inference).
    pub max_duration_ms: u64,
    /// Optional stricter cap for transcription + deterministic refine only; `None` uses runtime.
    pub inference_deadline_ms: Option<u64>,
    /// Preferred language override (passed by callers as metadata for now).
    pub language_hint: Option<String>,
    /// Correction profile for transcript refinement.
    pub correction_profile: OratioCorrectionProfile,
    /// Emit debug logs with parser payloads and correction traces.
    pub debug_parser_payload: bool,
    /// Periodic heartbeat interval for long-running operations.
    pub heartbeat_ms: u64,
    /// Optional client/session correlation id.
    pub session_id: Option<String>,
}

impl Default for OratioSessionConfig {
    fn default() -> Self {
        let rt = OratioRuntimeConfig::default();
        Self {
            timeout_ms: rt.session_timing.capture_timeout_ms,
            max_duration_ms: rt.session_timing.max_duration_ms,
            inference_deadline_ms: None,
            language_hint: None,
            correction_profile: OratioCorrectionProfile::Balanced,
            debug_parser_payload: false,
            heartbeat_ms: rt.session_timing.heartbeat_ms,
            session_id: None,
        }
    }
}

/// Timing diagnostics for one session run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OratioTimings {
    /// End-to-end session wall clock time.
    pub total_ms: u128,
    /// Time spent in transcription/decode path.
    pub transcribe_ms: u128,
    /// Time spent in refinement/correction path.
    pub refine_ms: u128,
    /// Config snapshot: capture UX timeout.
    pub capture_timeout_ms: u64,
    /// Config snapshot: session wall cap.
    pub max_duration_ms: u64,
    /// Effective inference/transcribe deadline used for checks.
    pub effective_inference_deadline_ms: u64,
}

/// Sessionized output returned to CLI and MCP callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OratioSessionResult {
    /// Session identifier used for tracing and two-way routing.
    pub session_id: String,
    /// Original path used for transcription (if any).
    pub path: PathBuf,
    /// Raw transcript text from backend.
    pub raw_text: String,
    /// Refined transcript text after correction rules.
    pub refined_text: String,
    /// Chosen display text (currently equal to refined text).
    pub text: String,
    /// Estimated confidence score for the final text.
    pub confidence: f32,
    /// Language hint used for this session.
    pub language_hint: Option<String>,
    /// Language diagnostic (normalized / validated hint and notes).
    pub language_diagnostics: Option<serde_json::Value>,
    /// Refinement trace for debugging and quality analysis.
    pub correction_trace: Vec<CorrectionTrace>,
    /// Alternate transcript hypotheses when re-ranking produced more than one candidate (best-first).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n_best: Option<Vec<String>>,
    /// Session timing diagnostics.
    pub timings: OratioTimings,
    /// Final state of the session.
    pub state: CaptureState,
    /// Deadline / timeout structured diagnostics.
    pub deadline_diagnostics: Vec<DeadlineDiagnostics>,
}

fn make_session_id(explicit: Option<&str>) -> String {
    if let Some(s) = explicit {
        return s.to_string();
    }
    format!(
        "oratio-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    )
}

/// Merge session config with runtime defaults for deadlines not explicitly set.
#[must_use]
pub fn session_config_with_runtime(
    cfg: &OratioSessionConfig,
    runtime: &OratioRuntimeConfig,
) -> OratioSessionConfig {
    let mut out = cfg.clone();
    if out.timeout_ms == 0 {
        out.timeout_ms = runtime.session_timing.capture_timeout_ms;
    }
    if out.max_duration_ms == 0 {
        out.max_duration_ms = runtime.session_timing.max_duration_ms;
    }
    if out.heartbeat_ms == 0 {
        out.heartbeat_ms = runtime.session_timing.heartbeat_ms;
    }
    out
}

/// Run an end-to-end transcription session from a provided file path.
pub fn transcribe_path_session(
    path: &Path,
    cfg: &OratioSessionConfig,
) -> Result<OratioSessionResult> {
    let runtime = OratioRuntimeConfig::resolve();
    transcribe_path_session_with_runtime(path, cfg, &runtime)
}

/// Like [`transcribe_path_session`] but uses a caller-provided resolved runtime snapshot (tests / MCP).
pub fn transcribe_path_session_with_runtime(
    path: &Path,
    cfg: &OratioSessionConfig,
    runtime: &OratioRuntimeConfig,
) -> Result<OratioSessionResult> {
    let cfg = session_config_with_runtime(cfg, runtime);
    if cfg.timeout_ms == 0 || cfg.max_duration_ms == 0 {
        anyhow::bail!("timeout_ms and max_duration_ms must be > 0");
    }
    if cfg.timeout_ms > cfg.max_duration_ms {
        anyhow::bail!("timeout_ms cannot exceed max_duration_ms");
    }
    let inference_limit = cfg
        .inference_deadline_ms
        .filter(|v| *v > 0)
        .unwrap_or_else(|| runtime.effective_inference_deadline_ms());

    let session_id = make_session_id(cfg.session_id.as_deref());
    let t0 = Instant::now();
    let transcribe_started = Instant::now();
    let (language_diagnostics, language_for_whisper) =
        crate::language::prepare_language_hint(cfg.language_hint.as_deref());
    let ctx =
        CorrectionContext::from_runtime(runtime, cfg.correction_profile, cfg.debug_parser_payload);
    let detail = transcribe_path_detailed(path, &ctx, language_for_whisper.as_deref())?;
    let transcribe_ms = transcribe_started.elapsed().as_millis();

    // Refinement is folded into `transcribe_path_detailed` today; keep column for future split.
    let refine_ms = 0u128;
    let total_ms = t0.elapsed().as_millis();

    let mut deadline_diagnostics = Vec::new();

    if transcribe_ms > u128::from(inference_limit) {
        deadline_diagnostics.push(DeadlineDiagnostics {
            phase: "transcribe".to_string(),
            elapsed_ms: transcribe_ms,
            limit_ms: inference_limit,
            taxonomy: OratioDeadlineTaxonomy::InferenceTimeout,
        });
        anyhow::bail!(
            "oratio_inference_timeout: transcribe_ms={transcribe_ms} inference_deadline_ms={inference_limit}"
        );
    }
    deadline_diagnostics.push(DeadlineDiagnostics {
        phase: "transcribe".to_string(),
        elapsed_ms: transcribe_ms,
        limit_ms: inference_limit,
        taxonomy: OratioDeadlineTaxonomy::Ok,
    });

    if total_ms > u128::from(cfg.max_duration_ms) {
        deadline_diagnostics.push(DeadlineDiagnostics {
            phase: "session_total".to_string(),
            elapsed_ms: total_ms,
            limit_ms: cfg.max_duration_ms,
            taxonomy: OratioDeadlineTaxonomy::DeadlineExceeded,
        });
        anyhow::bail!(
            "oratio_deadline_exceeded: wall_ms={total_ms} max_duration_ms={} phase=session_total",
            cfg.max_duration_ms
        );
    }
    deadline_diagnostics.push(DeadlineDiagnostics {
        phase: "session_total".to_string(),
        elapsed_ms: total_ms,
        limit_ms: cfg.max_duration_ms,
        taxonomy: OratioDeadlineTaxonomy::Ok,
    });

    if cfg.debug_parser_payload {
        tracing::debug!(
            target: "vox_oratio_session",
            stage = "final",
            session_id = session_id.as_str(),
            path = %path.display(),
            timeout_ms = cfg.timeout_ms,
            max_duration_ms = cfg.max_duration_ms,
            inference_deadline_ms = inference_limit,
            total_ms,
            transcribe_ms,
            "Oratio session diagnostics"
        );
    }

    Ok(OratioSessionResult {
        session_id,
        path: path.to_path_buf(),
        raw_text: detail.raw_text,
        refined_text: detail.refined_text.clone(),
        text: detail.refined_text,
        confidence: detail.confidence,
        language_hint: cfg.language_hint.clone(),
        language_diagnostics: Some(language_diagnostics),
        correction_trace: detail.correction_trace,
        n_best: detail.n_best.clone(),
        timings: OratioTimings {
            total_ms,
            transcribe_ms,
            refine_ms,
            capture_timeout_ms: cfg.timeout_ms,
            max_duration_ms: cfg.max_duration_ms,
            effective_inference_deadline_ms: inference_limit,
        },
        state: CaptureState::Completed,
        deadline_diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::Builder;

    use super::*;

    #[test]
    fn txt_fixture_session_roundtrip() {
        let mut f = Builder::new().suffix(".txt").tempfile().expect("tempfile");
        writeln!(f, "vox mends status").expect("write fixture");
        let out =
            transcribe_path_session(f.path(), &OratioSessionConfig::default()).expect("session");
        assert_eq!(out.text, "vox mens status");
        assert!(out.confidence > 0.0);
        assert!(
            out.deadline_diagnostics
                .iter()
                .any(|d| d.taxonomy == OratioDeadlineTaxonomy::Ok)
        );
    }
}
