//! Hybrid cost signal: measured tokens where available, LLM estimate elsewhere.

pub mod transcripts;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum MeasuredCost {
    Measured {
        input_tokens: u64,
        output_tokens: u64,
        source: String,
        session_id: String,
    },
    Estimated {
        input_tokens: u64,
        output_tokens: u64,
    },
    Ambiguous,
    Unavailable,
}
