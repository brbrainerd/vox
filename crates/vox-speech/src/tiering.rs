//! Model / escalation tier hints for ambiguous speech (local-first).
//!
//! Hosts decide how to map tiers to concrete models; Oratio only surfaces policy booleans.

/// Whether to escalate from the default STT/planner tier based on joint confidence.
#[must_use]
pub fn speech_escalation_recommended(intent_confidence: f32, asr_confidence: f32) -> bool {
    let ic = intent_confidence.clamp(0.0, 1.0);
    let ac = asr_confidence.clamp(0.0, 1.0);
    let blended = ic * 0.5 + ac * 0.5;
    blended < 0.48
}

/// Simple negative cache key for repeated failing transcripts within a session (collision-safe stub).
#[must_use]
pub fn speech_cache_key(session_id: &str, transcript_fingerprint: &str) -> String {
    format!("{session_id}:{transcript_fingerprint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escalation_low_confidence() {
        assert!(speech_escalation_recommended(0.2, 0.3));
        assert!(!speech_escalation_recommended(0.9, 0.9));
    }
}
