//! Deterministic calibration: turn logged interruption outcomes into a per-channel *gain* offset.
//! Sign convention matches `attention_policy::apply_calibration`, which ADDS the offset to
//! `expected_information_gain_bits` (higher gain ⇒ MORE likely to ask). So a channel that wastes
//! attention (high reject rate) must get a NEGATIVE offset (ask less); a well-received channel
//! gets a small POSITIVE offset (ask a bit more freely). Counts include suppressed-then-logged
//! decisions, satisfying the SSOT's "learn from suppressed interruptions too."

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChannelOutcomeCounts {
    pub accepted: u32,
    pub rejected: u32,
    pub suppressed: u32,
}

/// Bounded gain offset in `[-0.15, +0.05]` bits. NEGATIVE = raise the ask bar (channel wastes attention).
#[must_use]
pub fn channel_gain_offset(c: ChannelOutcomeCounts) -> f64 {
    let shown = c.accepted + c.rejected;
    if shown == 0 {
        return 0.0;
    }
    let reject_rate = c.rejected as f64 / shown as f64;
    // Center at a 25% acceptable reject rate. High reject ⇒ negative offset (ask less).
    let raw = (0.25 - reject_rate) * 0.20;
    raw.clamp(-0.15, 0.05)
}

/// Aggregate `(channel, outcome_or_event_str)` rows (as read from `attention_events`) into counts.
#[must_use]
pub fn aggregate_counts(
    rows: &[(Option<String>, String)],
) -> std::collections::HashMap<String, ChannelOutcomeCounts> {
    let mut map: std::collections::HashMap<String, ChannelOutcomeCounts> =
        std::collections::HashMap::new();
    for (channel, outcome) in rows {
        let key = channel.clone().unwrap_or_else(|| "unknown".to_string());
        let e = map.entry(key).or_default();
        match outcome.as_str() {
            "Accepted" | "Answered" => e.accepted += 1,
            "Rejected" => e.rejected += 1,
            "PolicyDeferred" | "PolicyProceedAuto" | "Suppressed" => e.suppressed += 1,
            _ => {}
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_data_is_neutral() {
        assert_eq!(channel_gain_offset(ChannelOutcomeCounts::default()), 0.0);
    }
    #[test]
    fn high_reject_rate_lowers_gain_to_ask_less() {
        let c = ChannelOutcomeCounts {
            accepted: 1,
            rejected: 9,
            suppressed: 3,
        };
        assert!(
            channel_gain_offset(c) < 0.0,
            "frequent rejection ⇒ negative offset (ask less)"
        );
    }
    #[test]
    fn mostly_accepted_raises_gain_slightly() {
        let c = ChannelOutcomeCounts {
            accepted: 19,
            rejected: 1,
            suppressed: 0,
        };
        assert!(
            channel_gain_offset(c) > 0.0,
            "mostly-accepted ⇒ positive offset (ask a bit more)"
        );
    }
    #[test]
    fn offset_is_bounded() {
        assert!(
            channel_gain_offset(ChannelOutcomeCounts {
                accepted: 0,
                rejected: 100,
                suppressed: 0
            }) >= -0.15
        );
    }
    #[test]
    fn aggregate_buckets_by_channel_and_outcome() {
        let rows = vec![
            (Some("mcp_chat".into()), "Rejected".to_string()),
            (Some("mcp_chat".into()), "Accepted".to_string()),
            (Some("mcp_chat".into()), "PolicyDeferred".to_string()),
        ];
        let m = aggregate_counts(&rows);
        assert_eq!(
            m["mcp_chat"],
            ChannelOutcomeCounts {
                accepted: 1,
                rejected: 1,
                suppressed: 1
            }
        );
    }
}

use crate::attention::budget::InterruptionCalibrationConfig;
use crate::attention::interruption_policy::InterruptionChannel;

fn interruption_channel_for_surface(surface: &str) -> InterruptionChannel {
    match surface {
        "vox_plan" | "vox_replan" | "vox_plan_status" => InterruptionChannel::PlanReview,
        "vox_inline_edit" | "vox_ghost_text" => InterruptionChannel::InlineAssist,
        _ => InterruptionChannel::ChatClarification,
    }
}

/// Produce a calibrated config by overwriting the four channel gain-offset fields from learned
/// per-channel counts. The HashMap keys are the REAL surface strings recorded in
/// `AttentionEvent.channel` (e.g. "vox_plan", "vox_inline_edit", "vox_ghost_text", the chat
/// surface) — NOT synthetic labels. We map them to channels via the existing
/// `interruption_channel_for_surface` helper to avoid string drift (DRY/SSOT).
/// Non-channel fields (backlog, trust) are preserved from `base`.
#[must_use]
pub fn apply_learned_offsets(
    base: InterruptionCalibrationConfig,
    counts: &std::collections::HashMap<String, ChannelOutcomeCounts>,
) -> InterruptionCalibrationConfig {
    let mut cfg = base;
    for (surface, c) in counts {
        let offset = channel_gain_offset(*c);
        match interruption_channel_for_surface(surface) {
            InterruptionChannel::PlanReview => cfg.plan_review_gain_offset_bits = offset,
            InterruptionChannel::TaskSubmit => cfg.task_submit_gain_offset_bits = offset,
            InterruptionChannel::A2AEscalation => cfg.a2a_escalation_gain_offset_bits = offset,
            InterruptionChannel::InlineAssist | InterruptionChannel::ChatClarification => {
                cfg.inline_assist_gain_offset_bits = offset
            }
            _ => {}
        }
    }
    cfg
}

#[cfg(test)]
mod close_loop_tests {
    use super::*;
    #[test]
    fn wasteful_channel_gets_negative_offset_into_config() {
        let mut counts = std::collections::HashMap::new();
        // Use a REAL surface string (what events actually carry), not "mcp_chat".
        counts.insert(
            "vox_inline_edit".to_string(),
            ChannelOutcomeCounts {
                accepted: 1,
                rejected: 9,
                suppressed: 0,
            },
        );
        let cfg = apply_learned_offsets(InterruptionCalibrationConfig::default(), &counts);
        assert!(
            cfg.inline_assist_gain_offset_bits < 0.0,
            "wasteful inline channel ⇒ ask less"
        );
        // non-channel knobs preserved
        assert_eq!(
            cfg.backlog_cost_penalty_per_item,
            InterruptionCalibrationConfig::default().backlog_cost_penalty_per_item
        );
    }
}
