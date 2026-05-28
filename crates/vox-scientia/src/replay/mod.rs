//! SCIENTIA Phase B — replay runner.
//!
//! Given a publication manifest's RO-Crate metadata containing a
//! [`MainEntity`](crate::ro_crate::MainEntity), this crate:
//!
//! 1. Stages the declared entry-point in a fresh sandbox directory.
//! 2. Executes it under `tokio::process::Command` with `kill_on_drop` and a
//!    hard wall-clock timeout.
//! 3. SHA3-256-hashes the declared output paths and compares to the
//!    `expected_output_hashes_hex` from the contract.
//! 4. Returns a [`ReplayReport`] suitable for writing back to worthiness
//!    signals as a measured `artifact_replayability_measured` value.
//!
//! The runner is platform-aware: POSIX hosts dispatch the entry-point via
//! `sh -c`; Windows hosts use `cmd /C`.

pub mod contract;
pub mod hash_compare;
pub mod report;
pub mod sandbox;

pub use report::{ReplayError, ReplayOutcome, ReplayReport};

use std::path::Path;
use std::time::Duration;
use crate::ro_crate::MainEntity;

/// Execute `main_entity` in a fresh sandbox under `stage_dir` and produce a
/// [`ReplayReport`].
///
/// `stage_dir` must already contain whatever artifacts the entry-point
/// expects to read; caller is responsible for staging. The runner only
/// re-executes the entry-point and verifies declared outputs.
pub async fn run_replay(
    stage_dir: &Path,
    main_entity: &MainEntity,
) -> Result<ReplayReport, ReplayError> {
    if main_entity.expected_output_paths.len() != main_entity.expected_output_hashes_hex.len() {
        return Err(ReplayError::ContractMalformed(format!(
            "expected_output_paths.len() = {}, expected_output_hashes_hex.len() = {}",
            main_entity.expected_output_paths.len(),
            main_entity.expected_output_hashes_hex.len()
        )));
    }
    let timeout = Duration::from_secs(main_entity.timeout_seconds as u64);
    let sandbox_outcome = sandbox::run_in_sandbox(stage_dir, &main_entity.entry_point, timeout)
        .await
        .map_err(ReplayError::Io)?;

    if sandbox_outcome.timed_out {
        return Ok(ReplayReport {
            outcome: ReplayOutcome::TimedOut,
            wall_ms: sandbox_outcome.wall_ms,
            measured_score: 0.0,
            diagnostics: vec![format!(
                "sandbox exceeded timeout of {} seconds",
                main_entity.timeout_seconds
            )],
            stdout: truncate(&sandbox_outcome.stdout, main_entity.max_stdout_bytes),
            stderr: truncate(&sandbox_outcome.stderr, main_entity.max_stderr_bytes),
        });
    }

    let exit_code = sandbox_outcome.exit_code;
    if !matches!(exit_code, Some(0)) {
        return Ok(ReplayReport {
            outcome: ReplayOutcome::NonZeroExit {
                exit_code: exit_code.unwrap_or(-1),
            },
            wall_ms: sandbox_outcome.wall_ms,
            measured_score: 0.0,
            diagnostics: vec![format!(
                "entry_point exited with code {:?}",
                exit_code
            )],
            stdout: truncate(&sandbox_outcome.stdout, main_entity.max_stdout_bytes),
            stderr: truncate(&sandbox_outcome.stderr, main_entity.max_stderr_bytes),
        });
    }

    let cmp = hash_compare::compare_output_hashes(
        stage_dir,
        &main_entity.expected_output_paths,
        &main_entity.expected_output_hashes_hex,
    )?;
    let (outcome, measured_score, diagnostics) = if cmp.all_match {
        (ReplayOutcome::Pass, 1.0, Vec::new())
    } else {
        (
            ReplayOutcome::HashMismatch,
            0.0,
            cmp.mismatches
                .iter()
                .map(|m| {
                    format!(
                        "output {} mismatch: expected sha3 {}, got {}",
                        m.path, m.expected_hex, m.actual_hex
                    )
                })
                .collect(),
        )
    };
    Ok(ReplayReport {
        outcome,
        wall_ms: sandbox_outcome.wall_ms,
        measured_score,
        diagnostics,
        stdout: truncate(&sandbox_outcome.stdout, main_entity.max_stdout_bytes),
        stderr: truncate(&sandbox_outcome.stderr, main_entity.max_stderr_bytes),
    })
}

fn truncate(s: &str, max_bytes: u64) -> String {
    let cap = max_bytes.try_into().unwrap_or(usize::MAX);
    if s.len() <= cap {
        s.to_string()
    } else {
        // Find the largest valid UTF-8 prefix ≤ cap.
        let mut end = cap;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        let mut out = String::with_capacity(end + 16);
        out.push_str(&s[..end]);
        out.push_str("\n…[truncated]");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_handles_short_strings_unchanged() {
        assert_eq!(truncate("ok", 100), "ok");
    }

    #[test]
    fn truncate_caps_long_strings_with_marker() {
        let long = "x".repeat(100);
        let out = truncate(&long, 50);
        assert!(out.contains("…[truncated]"));
        assert!(out.len() <= 50 + 16);
    }

    #[test]
    fn truncate_preserves_utf8_boundaries() {
        // Each "é" is 2 bytes in UTF-8.
        let s = "é".repeat(10); // 20 bytes
        let out = truncate(&s, 5);
        // Should not panic, and the prefix must be valid UTF-8.
        assert!(out.starts_with('é'));
        assert!(out.contains("…[truncated]"));
    }
}
