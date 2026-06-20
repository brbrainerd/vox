//! Typed transcript events emitted by `Session` and written to `vox-journal`.
//!
//! These events are the flywheel source: they capture input intent, raw output,
//! agent turns, and block accept/reject decisions for later curation into the
//! VoxMENS training corpus.

use serde::{Deserialize, Serialize};

use crate::block::Block;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TranscriptKind {
    Submitted {
        intent: String,
        input: String,
    },
    Output {
        stream: String,
        text: String,
    },
    AgentTurn {
        text: String,
    },
    ExitStatus {
        code: i32,
    },
    /// User accepted the block's output as useful training signal.
    Accepted {
        block: Block,
    },
    /// User rejected (thumbs-down) the block.
    Rejected {
        block: Block,
    },
    /// User corrected the output: `from` is what the model did, `to` is what they wanted.
    Corrected {
        from: String,
        to: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub session_id: String,
    pub seq: u64,
    pub kind: TranscriptKind,
}

/// Append-only journal sink for transcript events.
///
/// Wraps `vox-journal`'s `FileJournal` so `Session` doesn't depend on it
/// directly — callers that don't need persistence (tests, minimal builds) can
/// use `NullSink` instead.
pub trait TranscriptSink: Send + 'static {
    fn append(&self, event: &TranscriptEvent) -> anyhow::Result<()>;
}

/// A sink that discards events. Useful for tests and minimal builds.
pub struct NullSink;

impl TranscriptSink for NullSink {
    fn append(&self, _event: &TranscriptEvent) -> anyhow::Result<()> {
        Ok(())
    }
}

/// A sink backed by `vox-journal`'s append-only JSONL file.
pub struct JournalSink {
    journal: vox_journal::FileJournal<TranscriptEvent>,
}

impl JournalSink {
    pub fn open(path: impl Into<std::path::PathBuf>) -> anyhow::Result<Self> {
        let opened = vox_journal::FileJournal::<TranscriptEvent>::open(path)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Self {
            journal: opened.journal,
        })
    }
}

impl TranscriptSink for JournalSink {
    fn append(&self, event: &TranscriptEvent) -> anyhow::Result<()> {
        self.journal
            .append(event)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }
}
