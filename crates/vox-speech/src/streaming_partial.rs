//! Streaming partial-hypothesis stabilization (scaffold for future live ASR).
//!
//! Pair with endpointing/VAD tuning documented in platform STT docs. Values are **hints** until
//! a streaming decoder is wired.

/// Tunables for when to treat a partial transcript as “stable enough” to show or route.
#[derive(Debug, Clone, Copy)]
pub struct StreamingStabilizationConfig {
    /// Milliseconds with no token changes before locking in a partial.
    pub partial_quiet_ms: u64,
    /// Hard cap on time spent waiting for stabilization (p95 guardrail hook).
    pub max_wait_ms: u64,
}

impl Default for StreamingStabilizationConfig {
    fn default() -> Self {
        Self {
            partial_quiet_ms: 350,
            max_wait_ms: 2_500,
        }
    }
}

impl StreamingStabilizationConfig {
    /// Load from env `VOX_ORATIO_STREAM_PARTIAL_QUIET_MS` / `VOX_ORATIO_STREAM_MAX_WAIT_MS`.
    #[must_use]
    pub fn from_env() -> Self {
        let mut c = Self::default();
        if let Some(s) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioStreamPartialQuietMs)
                .expose()
            && let Ok(v) = s.parse::<u64>()
        {
            c.partial_quiet_ms = v.max(50);
        }
        if let Some(s) =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioStreamMaxWaitMs).expose()
            && let Ok(v) = s.parse::<u64>()
        {
            c.max_wait_ms = v.max(200);
        }
        c
    }
}

/// `true` when `quiet_for_ms` exceeds the stabilization threshold and wall time is under budget.
#[must_use]
pub fn should_commit_partial(
    elapsed_ms: u64,
    quiet_for_ms: u64,
    cfg: &StreamingStabilizationConfig,
) -> bool {
    quiet_for_ms >= cfg.partial_quiet_ms && elapsed_ms <= cfg.max_wait_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_when_quiet_and_under_cap() {
        let cfg = StreamingStabilizationConfig {
            partial_quiet_ms: 300,
            max_wait_ms: 1000,
        };
        assert!(should_commit_partial(500, 400, &cfg));
        assert!(!should_commit_partial(2000, 400, &cfg));
    }
}
