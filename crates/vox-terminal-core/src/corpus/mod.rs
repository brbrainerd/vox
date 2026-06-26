//! VoxMENS training flywheel corpus pipeline.
//!
//! Pipeline: `Session` transcripts → `curate` (filter) → `redact` (PII) → `writer` (JSONL)

pub mod curate;
pub mod redact;
pub mod writer;

pub use curate::{curate, CurationPolicy, Decision, DefaultPolicy};
pub use redact::redact_owned;
pub use writer::CorpusWriter;
