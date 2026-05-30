//! Output writers: JSONL, markdown, manifest.

pub mod jsonl;

use serde::{Deserialize, Serialize};

/// The top-level shape of one line in `findings.jsonl`. Stable schema_version="1.0".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindingRow {
    pub schema_version: String,
    pub commit_sha: String,
    pub parent_sha: Option<String>,
    pub commit_ts: chrono::DateTime<chrono::Utc>,
    pub author_email_sha256: String,
    pub branch_hint: String,
    pub message_first_line: String,
    pub shape: crate::shape::ShapeFeatures,
    pub cost: crate::hybrid::MeasuredCost,
    pub judge: JudgeMeta,
    pub finding: Option<crate::judge::schema::JudgeFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JudgeMeta {
    pub model_id: String,
    pub latency_ms: u64,
    pub judge_input_tokens: u64,
    pub judge_output_tokens: u64,
    pub outcome: String, // "Judged" | "Failed" | "Skipped"
}
