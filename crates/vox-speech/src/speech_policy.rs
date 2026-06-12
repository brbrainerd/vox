//! Clarification / confidence policy hooks for speech-to-code UX.

/// Returns `true` when the transcript confidence is below `min` and the caller should ask the user to repeat or confirm.
#[must_use]
pub fn clarification_recommended(transcript_confidence: f32, min_threshold: f32) -> bool {
    transcript_confidence < min_threshold.clamp(0.0, 1.0)
}
