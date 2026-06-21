//! Corpus curation — filters transcript events for MENS training quality.
//!
//! A [`CurationPolicy`] decides whether a [`TranscriptEvent`] is worth keeping.
//! The default policy drops noise (empty output, ephemeral status messages).

use crate::transcript::{TranscriptEvent, TranscriptKind};

/// Decision made by a curation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Keep,
    Drop,
}

/// Trait for pluggable curation policies.
pub trait CurationPolicy: Send + Sync + 'static {
    fn decide(&self, event: &TranscriptEvent) -> Decision;
}

/// Default policy: keeps non-trivial submitted intents + accepted/corrected outcomes.
pub struct DefaultPolicy {
    /// Minimum non-whitespace character count for `Output` chunks to keep.
    pub min_output_chars: usize,
}

impl Default for DefaultPolicy {
    fn default() -> Self {
        Self {
            min_output_chars: 1,
        }
    }
}

impl CurationPolicy for DefaultPolicy {
    fn decide(&self, event: &TranscriptEvent) -> Decision {
        match &event.kind {
            // Always keep intent submissions.
            TranscriptKind::Submitted { input, .. } => {
                if input.trim().is_empty() {
                    Decision::Drop
                } else {
                    Decision::Keep
                }
            }
            // Keep output chunks that have enough content.
            TranscriptKind::Output { text, .. } => {
                let non_ws: usize = text.chars().filter(|c| !c.is_whitespace()).count();
                if non_ws >= self.min_output_chars {
                    Decision::Keep
                } else {
                    Decision::Drop
                }
            }
            // Always keep agent turns and correction pairs.
            TranscriptKind::AgentTurn { .. }
            | TranscriptKind::Accepted { .. }
            | TranscriptKind::Corrected { .. } => Decision::Keep,
            // Drop ephemeral / low-signal events.
            TranscriptKind::ExitStatus { .. } | TranscriptKind::Rejected { .. } => Decision::Drop,
        }
    }
}

/// Filters a slice of events according to the policy, returning owned kept events.
pub fn curate<P: CurationPolicy>(events: &[TranscriptEvent], policy: &P) -> Vec<TranscriptEvent> {
    events
        .iter()
        .filter(|e| policy.decide(e) == Decision::Keep)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{TranscriptEvent, TranscriptKind};

    fn make(kind: TranscriptKind) -> TranscriptEvent {
        TranscriptEvent {
            session_id: "s1".into(),
            seq: 0,
            kind,
        }
    }

    #[test]
    fn empty_input_dropped() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::Submitted {
            intent: "shell".into(),
            input: "  ".into(),
        });
        assert_eq!(policy.decide(&ev), Decision::Drop);
    }

    #[test]
    fn non_empty_input_kept() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::Submitted {
            intent: "shell".into(),
            input: "ls -la".into(),
        });
        assert_eq!(policy.decide(&ev), Decision::Keep);
    }

    #[test]
    fn whitespace_only_output_dropped() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::Output {
            stream: "stdout".into(),
            text: "\n   \n".into(),
        });
        assert_eq!(policy.decide(&ev), Decision::Drop);
    }

    #[test]
    fn nonempty_output_kept() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::Output {
            stream: "stdout".into(),
            text: "hi\n".into(),
        });
        assert_eq!(policy.decide(&ev), Decision::Keep);
    }

    #[test]
    fn agent_turn_kept() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::AgentTurn {
            text: "Here's what I found…".into(),
        });
        assert_eq!(policy.decide(&ev), Decision::Keep);
    }

    #[test]
    fn exit_status_dropped() {
        let policy = DefaultPolicy::default();
        let ev = make(TranscriptKind::ExitStatus { code: 0 });
        assert_eq!(policy.decide(&ev), Decision::Drop);
    }

    #[test]
    fn curate_filters_batch() {
        let events = vec![
            make(TranscriptKind::Submitted {
                intent: "shell".into(),
                input: "echo hi".into(),
            }),
            make(TranscriptKind::ExitStatus { code: 0 }),
            make(TranscriptKind::Output {
                stream: "stdout".into(),
                text: "hi\n".into(),
            }),
        ];
        let kept = curate(&events, &DefaultPolicy::default());
        assert_eq!(kept.len(), 2);
    }
}
