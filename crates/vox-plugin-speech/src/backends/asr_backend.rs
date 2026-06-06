//! Common output type and trait for all Oratio STT backends.

use serde::{Deserialize, Serialize};

/// A segment of text with start and end timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimedSegment {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Raw text for this segment.
    pub text: String,
}
