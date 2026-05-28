//! Right-of-reply window gate — SCIENTIA Phase 3.
//!
//! Enforces a 14-day right-of-reply window before a provider_atlas topic-pack
//! may be published. Mirrors the [`crate::preregistration::gate::PreregGate`] pattern: call before publishing,
//! receive a [`crate::preregistration::gate::GateResult`].

use crate::preregistration::gate::GateResult;

const WINDOW_DAYS: u64 = 14;
const SECS_PER_DAY: u64 = 86_400;
const WINDOW_SECS: u64 = WINDOW_DAYS * SECS_PER_DAY;

/// A record tracking the right-of-reply window for one provider.
#[derive(Debug, Clone)]
pub struct ReplyWindowRecord {
    pub provider_id: String,
    /// Unix timestamp (seconds) when the window was opened (draft sent to provider).
    pub window_opened_at: i64,
    /// True once the provider has explicitly cleared the window.
    pub provider_cleared: bool,
    /// Inline reply text per IMC measurement-paper conventions (None if no reply).
    pub reply_content: Option<String>,
}

/// Current status of a right-of-reply window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowStatus {
    /// Still within the 14-day window; provider has not cleared.
    Open { days_remaining: u64 },
    /// Provider explicitly cleared the window; `has_reply` indicates a reply was ingested.
    Cleared { has_reply: bool },
    /// 14 days have elapsed without a provider response; publication may proceed.
    Expired,
}

/// Gate that enforces the 14-day right-of-reply window before publication.
#[derive(Debug, Default, Clone)]
pub struct ReplyWindowGate;

impl ReplyWindowGate {
    pub fn new() -> Self {
        Self
    }

    /// Compute the current [`WindowStatus`] given `now_unix` (Unix seconds).
    ///
    /// Deterministic: callers pass the clock value, enabling test control.
    pub fn status(&self, record: &ReplyWindowRecord, now_unix: i64) -> WindowStatus {
        if record.provider_cleared {
            return WindowStatus::Cleared {
                has_reply: record.reply_content.is_some(),
            };
        }

        let elapsed = (now_unix - record.window_opened_at).max(0) as u64;
        if elapsed >= WINDOW_SECS {
            WindowStatus::Expired
        } else {
            let secs_remaining = WINDOW_SECS - elapsed;
            // Round up: partial day counts as a full day remaining.
            let days_remaining = secs_remaining.div_ceil(SECS_PER_DAY);
            WindowStatus::Open { days_remaining }
        }
    }

    /// Check whether publication is permitted.
    ///
    /// Returns [`GateResult::Approved`] when the window is `Cleared` or `Expired`.
    /// Returns [`GateResult::Refused`] with `days_remaining` when still `Open`.
    pub fn check_publication(&self, record: &ReplyWindowRecord, now_unix: i64) -> GateResult {
        match self.status(record, now_unix) {
            WindowStatus::Open { days_remaining } => GateResult::Refused {
                reason: format!(
                    "right-of-reply window is still open for provider '{}': {} day(s) remaining",
                    record.provider_id, days_remaining
                ),
            },
            WindowStatus::Cleared { .. } | WindowStatus::Expired => GateResult::Approved,
        }
    }
}

/// Ingest a reply from the provider.
///
/// Sets `record.reply_content` to `Some(reply_text)` and marks `provider_cleared = true`.
/// Per IMC conventions the reply is stored inline, not as an appendix.
pub fn ingest_reply(record: &mut ReplyWindowRecord, reply_text: &str) {
    record.reply_content = Some(reply_text.to_string());
    record.provider_cleared = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened_at() -> i64 {
        // Arbitrary fixed epoch: 2026-01-01 00:00:00 UTC
        1_767_225_600
    }

    fn base_record() -> ReplyWindowRecord {
        ReplyWindowRecord {
            provider_id: "provider-alpha".to_string(),
            window_opened_at: opened_at(),
            provider_cleared: false,
            reply_content: None,
        }
    }

    #[test]
    fn window_is_open_within_14_days() {
        let gate = ReplyWindowGate::new();
        let record = base_record();
        // 5 days after opening
        let now = opened_at() + 5 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(
            status,
            WindowStatus::Open { days_remaining: 9 },
            "5 days elapsed → 9 days remaining"
        );
    }

    #[test]
    fn window_is_expired_after_14_days() {
        let gate = ReplyWindowGate::new();
        let record = base_record();
        // 15 days after opening
        let now = opened_at() + 15 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(status, WindowStatus::Expired, "15 days elapsed → Expired");
    }

    #[test]
    fn provider_cleared_before_14_days() {
        let gate = ReplyWindowGate::new();
        let mut record = base_record();
        record.provider_cleared = true;
        // Only 3 days elapsed, but provider cleared
        let now = opened_at() + 3 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(
            status,
            WindowStatus::Cleared { has_reply: false },
            "provider_cleared=true, no reply → Cleared{{has_reply: false}}"
        );
    }

    #[test]
    fn ingested_reply_marks_cleared() {
        let mut record = base_record();
        assert!(!record.provider_cleared);
        assert!(record.reply_content.is_none());

        ingest_reply(&mut record, "We dispute the latency figures in §3.");

        assert!(
            record.provider_cleared,
            "ingest_reply must set provider_cleared"
        );
        assert_eq!(
            record.reply_content.as_deref(),
            Some("We dispute the latency figures in §3."),
            "ingest_reply must store reply text"
        );
    }
}
