use std::collections::VecDeque;

use crate::types::AgentId;

use super::budget::{ActionDescriptor, ApprovalTier, TierGateConfig, TrustTier};

/// Idle time after which trust begins decaying toward the neutral prior (T5.5).
/// Below this gap, `apply_idle_decay` is a no-op — active/recently-active agents
/// are never touched by decay.
pub const TRUST_DECAY_GRACE_MS: u64 = 24 * 60 * 60 * 1000; // 24h

/// Half-life (ms) for idle trust decay once the grace period has elapsed: every
/// `TRUST_DECAY_HALF_LIFE_MS` of additional idle time, the gap between `trust_score`
/// and the neutral prior (0.5) halves. ~7 days.
pub const TRUST_DECAY_HALF_LIFE_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Default rolling window (ms) for `windowed_repeated_approve_count` /
/// `windowed_approve_rate`: only outcomes recorded within this many ms of "now"
/// count toward `repeated_approve_count` / approve-rate (T5.5). ~30 days.
pub const DEFAULT_TRUST_WINDOW_MS: u64 = 30 * 24 * 60 * 60 * 1000;

/// Cap on how many recent outcomes are retained in `outcome_window`, bounding memory
/// regardless of how long an agent has been alive.
const MAX_WINDOW_SAMPLES: usize = 256;

/// Per-agent trust score with Kalman-filter update and hysteresis demotion.
///
/// The Kalman filter converges faster than EWMA for agents with consistent histories
/// (Task 62) while the `variance` field enables UCB exploration (Task 61).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentTrustScore {
    pub agent_id: AgentId,
    /// Kalman-filtered trust estimate ∈ [0.0, 1.0].
    pub trust_score: f64,
    pub tier: TrustTier,
    pub total_outcomes: u32,
    pub successful_outcomes: u32,
    /// Consecutive events below current tier's lower bound (hysteresis counter).
    pub below_tier_streak: u32,
    pub last_updated_ms: u64,
    /// Kalman estimate variance ∈ [0.0, 1.0] — high = uncertain = more exploration (Task 60).
    pub variance: f64,
    /// Manual override flag (Task 64)
    pub is_override: bool,
    /// Rolling window of recent `(timestamp_ms, success)` outcomes, most-recent first,
    /// bounded to `MAX_WINDOW_SAMPLES` (T5.5). Backs `windowed_repeated_approve_count` /
    /// `windowed_approve_rate` so old behavior ages out instead of weighing equally with
    /// recent behavior forever. Distinct from `total_outcomes`/`successful_outcomes`,
    /// which remain lifetime counters used by the Kalman filter and promotion gates.
    #[serde(default)]
    pub outcome_window: VecDeque<(u64, bool)>,
}

impl AgentTrustScore {
    /// Create a new agent with Empirical Bayes prior: trust = 0.5, variance = 0.25 (Task 63).
    ///
    /// The high initial variance (0.25) drives UCB exploration for new agents so they receive
    /// tasks before their performance is fully characterized.
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            agent_id,
            trust_score: 0.5,
            tier: TrustTier::Untrusted,
            total_outcomes: 0,
            successful_outcomes: 0,
            below_tier_streak: 0,
            last_updated_ms: crate::types::now_unix_ms(),
            variance: 0.25,
            is_override: false,
            outcome_window: VecDeque::new(),
        }
    }

    /// Update trust with a discrete Kalman filter step (Task 62).
    ///
    /// The Kalman gain `K = P / (P + R)` (measurement noise R = 0.1) adapts the update
    /// magnitude to the current variance, converging faster than a fixed-α EWMA when
    /// the agent is consistent.
    ///
    /// `provisional_min` and `trusted_min` come from `OrchestratorConfig`.
    pub fn record_outcome(
        &mut self,
        success: bool,
        _alpha: f64,
        provisional_min: u32,
        trusted_min: u32,
    ) -> f64 {
        const MEASUREMENT_NOISE: f64 = 0.10;
        const PROCESS_NOISE: f64 = 0.005;

        if self.is_override {
            return self.trust_score;
        }

        let observation = if success { 1.0_f64 } else { 0.0_f64 };

        // Prediction step: variance grows by process noise
        let p_pred = (self.variance + PROCESS_NOISE).min(1.0);

        // Update step: Kalman gain
        let k = p_pred / (p_pred + MEASUREMENT_NOISE);
        self.trust_score =
            (self.trust_score + k * (observation - self.trust_score)).clamp(0.0, 1.0);
        self.variance = (1.0 - k) * p_pred;

        self.total_outcomes += 1;
        if success {
            self.successful_outcomes += 1;
        }
        let now_ms = crate::types::now_unix_ms();
        self.last_updated_ms = now_ms;
        self.push_windowed_outcome(now_ms, success);
        self.update_tier(provisional_min, trusted_min);
        self.trust_score
    }

    /// Record `(now_ms, success)` into the bounded rolling window, evicting the
    /// oldest sample once `MAX_WINDOW_SAMPLES` is exceeded (T5.5).
    fn push_windowed_outcome(&mut self, now_ms: u64, success: bool) {
        self.outcome_window.push_front((now_ms, success));
        while self.outcome_window.len() > MAX_WINDOW_SAMPLES {
            self.outcome_window.pop_back();
        }
    }

    /// Count of approvals within the last `window_ms` of `now_ms` (T5.5).
    ///
    /// Replaces treating `successful_outcomes` (a lifetime accumulator) as the
    /// "repeated approve count" fed into `ActionDescriptor::repeated_approve_count` /
    /// `classify_tier`'s entropy-auto-approve graduation — old behavior now ages out
    /// of the count instead of weighing equally with recent behavior forever.
    #[must_use]
    pub fn windowed_repeated_approve_count(&self, now_ms: u64, window_ms: u64) -> u32 {
        let cutoff = now_ms.saturating_sub(window_ms);
        self.outcome_window
            .iter()
            .filter(|(ts, success)| *success && *ts >= cutoff)
            .count() as u32
    }

    /// Approve rate (successes / total) within the last `window_ms` of `now_ms` (T5.5).
    /// Returns `None` when there are no samples in the window (caller should fall back
    /// to a neutral prior, matching the existing `approve_rate` default of 0.5).
    #[must_use]
    pub fn windowed_approve_rate(&self, now_ms: u64, window_ms: u64) -> Option<f64> {
        let cutoff = now_ms.saturating_sub(window_ms);
        let mut total = 0u32;
        let mut success = 0u32;
        for (ts, ok) in &self.outcome_window {
            if *ts >= cutoff {
                total += 1;
                if *ok {
                    success += 1;
                }
            }
        }
        if total == 0 {
            None
        } else {
            Some(success as f64 / total as f64)
        }
    }

    /// Idle-time trust decay (T5.5): if the agent has gone quiet for longer than
    /// `TRUST_DECAY_GRACE_MS`, pull `trust_score` toward the neutral prior (0.5) with an
    /// exponential half-life of `TRUST_DECAY_HALF_LIFE_MS`, and grow `variance` back toward
    /// the fresh-agent prior (0.25) at the same rate — mirroring the Kalman filter's own
    /// prediction-step variance growth, so a long-idle agent is once again treated as
    /// uncertain (subject to UCB exploration) rather than confidently trusted on stale data.
    ///
    /// Pure and idempotent: calling it repeatedly with the same `now_ms` is a no-op after
    /// the first call because it also advances `last_updated_ms`, so the caller does not
    /// need to guard against double-decay. Overrides (`is_override`) are exempt — an
    /// operator-forced trust score does not erode with time.
    pub fn apply_idle_decay(&mut self, now_ms: u64) {
        if self.is_override {
            return;
        }
        let idle_ms = now_ms.saturating_sub(self.last_updated_ms);
        if idle_ms <= TRUST_DECAY_GRACE_MS {
            return;
        }
        let decay_elapsed_ms = idle_ms - TRUST_DECAY_GRACE_MS;
        // Exponential decay toward the neutral prior: factor = 0.5 ^ (elapsed / half_life).
        let half_lives = decay_elapsed_ms as f64 / TRUST_DECAY_HALF_LIFE_MS as f64;
        let retain = 0.5_f64.powf(half_lives);

        const NEUTRAL_TRUST: f64 = 0.5;
        const FRESH_VARIANCE: f64 = 0.25;

        self.trust_score = NEUTRAL_TRUST + (self.trust_score - NEUTRAL_TRUST) * retain;
        self.variance = FRESH_VARIANCE + (self.variance - FRESH_VARIANCE) * retain;
        self.last_updated_ms = now_ms;
        // Decay can pull a demoted-but-still-labeled tier's score below its floor (or a
        // long-idle low-trust agent's score back up); resync the tier label so it stays
        // consistent with the decayed score rather than freezing at whatever it was when
        // the agent went idle. Demotion hysteresis is intentionally bypassed here — this
        // is a time-driven correction, not an outcome-driven demotion — but never promotes
        // above what the decayed score alone would earn via the same tier floors used in
        // `update_tier`.
        self.resync_tier_to_score();
    }

    /// Re-derive `tier` purely from the current `trust_score` against the tier floors used
    /// by `update_tier`, without touching `below_tier_streak` hysteresis. Used by
    /// `apply_idle_decay` so a long-idle demotion (or a decay-driven partial recovery
    /// toward the neutral prior) is reflected immediately rather than waiting for 3
    /// consecutive outcome-driven demotion events that may never come for an idle agent.
    fn resync_tier_to_score(&mut self) {
        let target = if self.trust_score >= 0.90 {
            TrustTier::System
        } else if self.trust_score >= 0.70 {
            TrustTier::Trusted
        } else if self.trust_score >= 0.45 {
            TrustTier::Provisional
        } else {
            TrustTier::Untrusted
        };
        // Never let idle decay auto-promote into System (operator-only, per `update_tier`).
        self.tier = if target == TrustTier::System && self.tier != TrustTier::System {
            TrustTier::Trusted
        } else {
            target
        };
        self.below_tier_streak = 0;
    }

    /// UCB (Upper Confidence Bound) score for exploration-driven routing (Task 61).
    ///
    /// Combines the Kalman trust estimate with an exploration bonus proportional to `variance`.
    /// Agents with high uncertainty receive a bonus that encourages the router to sample them,
    /// spreading load more evenly than pure greedy selection.
    pub fn ucb_score(&self, exploration_weight: f64) -> f64 {
        // UCB1-style: μ + c * σ  where σ = sqrt(variance)
        (self.trust_score + exploration_weight * self.variance.sqrt()).clamp(0.0, 2.0)
    }

    fn update_tier(&mut self, provisional_min: u32, trusted_min: u32) {
        let lower = match self.tier {
            TrustTier::Untrusted => 0.0,
            TrustTier::Provisional => 0.45,
            TrustTier::Trusted => 0.70,
            TrustTier::System => 0.90,
        };

        // Promotion checks (System is operator-only; not auto-promoted)
        if self.trust_score >= 0.70
            && self.total_outcomes >= trusted_min
            && matches!(self.tier, TrustTier::Untrusted | TrustTier::Provisional)
        {
            self.tier = TrustTier::Trusted;
            self.below_tier_streak = 0;
            return;
        }
        if self.trust_score >= 0.45
            && self.total_outcomes >= provisional_min
            && self.tier == TrustTier::Untrusted
        {
            self.tier = TrustTier::Provisional;
            self.below_tier_streak = 0;
            return;
        }

        // Demotion with hysteresis (3 consecutive events below tier floor)
        if self.trust_score < lower && self.tier != TrustTier::Untrusted {
            self.below_tier_streak += 1;
            if self.below_tier_streak >= 3 {
                self.tier = match self.tier {
                    TrustTier::System => TrustTier::Trusted,
                    TrustTier::Trusted => TrustTier::Provisional,
                    TrustTier::Provisional => TrustTier::Untrusted,
                    TrustTier::Untrusted => TrustTier::Untrusted,
                };
                self.below_tier_streak = 0;
            }
        } else {
            self.below_tier_streak = 0;
        }
    }
}

/// Classify the approval tier for an action based on trust, complexity, and patterns.
/// All thresholds are taken from `gate` to avoid hard-coded constants.
pub fn classify_tier(
    trust: &AgentTrustScore,
    action: &ActionDescriptor,
    entropy: f64,
    gate: &TierGateConfig,
) -> ApprovalTier {
    // Hard blocks first
    if action.external {
        return ApprovalTier::Blocked;
    }
    if action.write_file_count > gate.untrusted_max_writes_before_block
        && trust.tier == TrustTier::Untrusted
    {
        return ApprovalTier::Blocked;
    }

    // Auto-approve graduation: low Shannon entropy over sufficient observations
    if entropy < gate.entropy_auto_approve_threshold
        && action.repeated_approve_count >= gate.auto_approve_min_observations
        && trust.trust_score >= gate.auto_approve_min_trust
        && matches!(trust.tier, TrustTier::Trusted | TrustTier::System)
    {
        return ApprovalTier::AutoApprove;
    }

    // Read-only actions are always safe
    if action.write_file_count == 0 && !action.external {
        return ApprovalTier::AutoApprove;
    }

    // Trust-based classification
    match trust.tier {
        TrustTier::System => ApprovalTier::AutoApprove,
        TrustTier::Trusted => {
            if action.write_file_count <= gate.trusted_single_file_confirm_limit {
                ApprovalTier::Confirm
            } else {
                ApprovalTier::Review
            }
        }
        TrustTier::Provisional => ApprovalTier::Review,
        TrustTier::Untrusted => {
            if action.write_file_count > gate.untrusted_max_writes_before_block {
                ApprovalTier::Blocked
            } else {
                ApprovalTier::Review
            }
        }
    }
}

#[cfg(test)]
mod trust_decay_and_window_tests {
    use super::*;

    fn trusted_agent(now_ms: u64) -> AgentTrustScore {
        // Simulate an agent that earned solid trust through consistent successes.
        // `record_outcome` stamps `last_updated_ms` (and window entries) with the real
        // wall clock, not a caller-supplied time, so pin both back to the synthetic
        // `now_ms` afterward to keep decay tests deterministic and independent of when
        // they happen to run.
        let mut ts = AgentTrustScore::new(AgentId(1));
        for _ in 0..20 {
            ts.record_outcome(true, 0.1, 5, 20);
        }
        ts.last_updated_ms = now_ms;
        for (ts_ms, _) in ts.outcome_window.iter_mut() {
            *ts_ms = now_ms;
        }
        ts
    }

    // (a) idle-for-a-long-period trust decays below an otherwise-identical active agent.
    #[test]
    fn idle_agent_trust_decays_below_active_twin() {
        let now_ms = 10_000_000_000_u64;
        let idle = trusted_agent(now_ms);
        let active = trusted_agent(now_ms);
        assert_eq!(
            idle.trust_score, active.trust_score,
            "twins must start identical"
        );

        let mut idle = idle;
        // Idle for far longer than the grace period + several half-lives.
        let later = now_ms + TRUST_DECAY_GRACE_MS + 4 * TRUST_DECAY_HALF_LIFE_MS;
        idle.apply_idle_decay(later);

        // The active twin keeps acting (each `record_outcome` resets its idle clock),
        // so it never enters decay and its score stays put (still trusted).
        let active = active;

        assert!(
            idle.trust_score < active.trust_score,
            "idle={:.4} should be measurably below active={:.4}",
            idle.trust_score,
            active.trust_score
        );
        // Decayed toward the neutral prior (0.5), not just "a little lower".
        assert!(
            (idle.trust_score - 0.5).abs() < (active.trust_score - 0.5).abs(),
            "idle score {:.4} should sit closer to the neutral prior than active {:.4}",
            idle.trust_score,
            active.trust_score
        );
    }

    #[test]
    fn decay_is_noop_within_grace_period() {
        let now_ms = 1_000_000_u64;
        let mut ts = trusted_agent(now_ms);
        let before = ts.trust_score;
        ts.apply_idle_decay(now_ms + TRUST_DECAY_GRACE_MS - 1);
        assert_eq!(
            ts.trust_score, before,
            "no decay before the grace period elapses"
        );
    }

    #[test]
    fn decay_pulls_score_toward_neutral_prior_not_past_it() {
        let now_ms = 1_000_000_u64;
        let mut ts = trusted_agent(now_ms);
        assert!(ts.trust_score > 0.5, "fixture should start above neutral");
        // Extremely long idle period: should approach 0.5 but never overshoot.
        ts.apply_idle_decay(now_ms + TRUST_DECAY_GRACE_MS + 100 * TRUST_DECAY_HALF_LIFE_MS);
        assert!(
            (ts.trust_score - 0.5).abs() < 0.01,
            "score {:.4} should have converged near the neutral prior",
            ts.trust_score
        );
    }

    #[test]
    fn decay_grows_variance_back_toward_fresh_prior() {
        let now_ms = 1_000_000_u64;
        let mut ts = trusted_agent(now_ms);
        let converged_variance = ts.variance;
        assert!(
            converged_variance < 0.25,
            "20 consistent successes should shrink variance below the fresh prior"
        );
        ts.apply_idle_decay(now_ms + TRUST_DECAY_GRACE_MS + 4 * TRUST_DECAY_HALF_LIFE_MS);
        assert!(
            ts.variance > converged_variance,
            "idle decay should grow variance back up (more UCB exploration), got {:.4} <= {:.4}",
            ts.variance,
            converged_variance
        );
    }

    #[test]
    fn override_scores_are_exempt_from_decay() {
        let now_ms = 1_000_000_u64;
        let mut ts = trusted_agent(now_ms);
        ts.is_override = true;
        let before = ts.trust_score;
        ts.apply_idle_decay(now_ms + TRUST_DECAY_GRACE_MS + 10 * TRUST_DECAY_HALF_LIFE_MS);
        assert_eq!(before, ts.trust_score, "operator override must not decay");
    }

    #[test]
    fn long_idle_demotes_tier_without_waiting_for_outcome_hysteresis() {
        let now_ms = 1_000_000_u64;
        let mut ts = trusted_agent(now_ms);
        assert_eq!(ts.tier, TrustTier::Trusted);
        ts.apply_idle_decay(now_ms + TRUST_DECAY_GRACE_MS + 50 * TRUST_DECAY_HALF_LIFE_MS);
        assert_eq!(
            ts.tier,
            TrustTier::Provisional,
            "score decayed to ~0.5 should resync tier to Provisional, not stay frozen at Trusted"
        );
    }

    // (b) repeated_approve_count / approve-rate reflect only a recent window.
    #[test]
    fn windowed_repeated_approve_count_ignores_old_approvals() {
        let mut ts = AgentTrustScore::new(AgentId(2));
        let window_ms = DEFAULT_TRUST_WINDOW_MS;
        let now_ms = 100 * window_ms; // plenty of headroom before "now"

        // 5 approvals well outside the window (old behavior).
        for i in 0..5 {
            ts.push_windowed_outcome(now_ms - 3 * window_ms - i, true);
        }
        // 3 approvals inside the window (recent behavior).
        for i in 0..3 {
            ts.push_windowed_outcome(now_ms - window_ms / 2 - i, true);
        }

        assert_eq!(
            ts.windowed_repeated_approve_count(now_ms, window_ms),
            3,
            "only the 3 recent approvals should count; the 5 old ones must age out"
        );
    }

    #[test]
    fn windowed_approve_rate_reflects_only_recent_window() {
        let mut ts = AgentTrustScore::new(AgentId(3));
        let window_ms = DEFAULT_TRUST_WINDOW_MS;
        let now_ms = 100 * window_ms;

        // Old history: 10 failures outside the window (would drag lifetime rate to 0
        // if it counted).
        for i in 0..10 {
            ts.push_windowed_outcome(now_ms - 3 * window_ms - i, false);
        }
        // Recent history: 4 successes inside the window.
        for i in 0..4 {
            ts.push_windowed_outcome(now_ms - window_ms / 4 - i, true);
        }

        let rate = ts
            .windowed_approve_rate(now_ms, window_ms)
            .expect("recent samples exist");
        assert_eq!(
            rate, 1.0,
            "old failures must not drag down a rate computed only over the recent window"
        );
    }

    #[test]
    fn windowed_approve_rate_none_when_no_recent_samples() {
        let mut ts = AgentTrustScore::new(AgentId(4));
        let window_ms = DEFAULT_TRUST_WINDOW_MS;
        let now_ms = 100 * window_ms;
        // Only old samples, nothing recent.
        ts.push_windowed_outcome(now_ms - 5 * window_ms, true);
        assert_eq!(ts.windowed_approve_rate(now_ms, window_ms), None);
    }

    #[test]
    fn record_outcome_feeds_the_rolling_window() {
        let mut ts = AgentTrustScore::new(AgentId(5));
        ts.record_outcome(true, 0.1, 5, 20);
        ts.record_outcome(false, 0.1, 5, 20);
        assert_eq!(ts.outcome_window.len(), 2);
        let now_ms = ts.last_updated_ms;
        assert_eq!(
            ts.windowed_repeated_approve_count(now_ms, DEFAULT_TRUST_WINDOW_MS),
            1,
            "one of the two recorded outcomes was a success"
        );
    }

    #[test]
    fn window_is_bounded_regardless_of_lifetime_length() {
        let mut ts = AgentTrustScore::new(AgentId(6));
        for _ in 0..(MAX_WINDOW_SAMPLES + 50) {
            ts.record_outcome(true, 0.1, 5, 20);
        }
        assert!(
            ts.outcome_window.len() <= MAX_WINDOW_SAMPLES,
            "window must stay bounded: {} > {}",
            ts.outcome_window.len(),
            MAX_WINDOW_SAMPLES
        );
    }
}
