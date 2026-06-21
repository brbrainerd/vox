//! Corpus writer — serialises curated + redacted events to JSONL for MENS training.
//!
//! Each line is a JSON object with `{"session_id":…,"seq":…,"kind":…}`.
//! The writer flushes after every write so partial outputs are recoverable.

use anyhow::Result;
use std::io::{BufWriter, Write};

use super::redact::redact_owned;
use crate::transcript::TranscriptEvent;

/// Writes [`TranscriptEvent`]s as JSONL, redacting each event's text fields.
pub struct CorpusWriter<W: Write> {
    out: BufWriter<W>,
}

impl<W: Write> CorpusWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            out: BufWriter::new(inner),
        }
    }

    /// Writes a single event, redacting PII, then flushes.
    pub fn write_event(&mut self, event: &TranscriptEvent) -> Result<()> {
        let redacted = redact_event(event);
        let line = serde_json::to_string(&redacted)?;
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\n")?;
        self.out.flush()?;
        Ok(())
    }

    /// Writes all events from a batch.
    pub fn write_batch(&mut self, events: &[TranscriptEvent]) -> Result<()> {
        for ev in events {
            self.write_event(ev)?;
        }
        Ok(())
    }
}

/// Returns a clone of `event` with text fields redacted.
fn redact_event(event: &TranscriptEvent) -> TranscriptEvent {
    use crate::transcript::TranscriptKind::*;
    let kind = match &event.kind {
        Submitted { intent, input } => Submitted {
            intent: redact_owned(intent),
            input: redact_owned(input),
        },
        Output { stream, text } => Output {
            stream: stream.clone(),
            text: redact_owned(text),
        },
        AgentTurn { text } => AgentTurn {
            text: redact_owned(text),
        },
        Corrected { from, to } => Corrected {
            from: redact_owned(from),
            to: redact_owned(to),
        },
        other => other.clone(),
    };
    TranscriptEvent {
        session_id: event.session_id.clone(),
        seq: event.seq,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{TranscriptEvent, TranscriptKind};

    #[test]
    fn writes_valid_jsonl() {
        let events = vec![
            TranscriptEvent {
                session_id: "s1".into(),
                seq: 0,
                kind: TranscriptKind::Submitted {
                    intent: "shell".into(),
                    input: "ls -la".into(),
                },
            },
            TranscriptEvent {
                session_id: "s1".into(),
                seq: 1,
                kind: TranscriptKind::Output {
                    stream: "stdout".into(),
                    text: "total 4\n".into(),
                },
            },
        ];

        let mut buf: Vec<u8> = Vec::new();
        {
            let mut writer = CorpusWriter::new(&mut buf);
            writer.write_batch(&events).unwrap();
        } // writer dropped here, releasing borrow of buf

        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        // Each line must be valid JSON.
        for line in &lines {
            serde_json::from_str::<serde_json::Value>(line).expect("invalid JSON line");
        }
    }

    #[test]
    fn email_redacted_in_output() {
        let event = TranscriptEvent {
            session_id: "s1".into(),
            seq: 0,
            kind: TranscriptKind::Output {
                stream: "stdout".into(),
                text: "sent by alice@example.com".into(),
            },
        };
        let mut buf: Vec<u8> = Vec::new();
        {
            let mut writer = CorpusWriter::new(&mut buf);
            writer.write_event(&event).unwrap();
        } // writer dropped here, releasing borrow

        let text = String::from_utf8(buf).unwrap();
        assert!(!text.contains("alice@example.com"), "PII leaked: {text}");
        assert!(text.contains("[REDACTED_EMAIL]"));
    }
}
