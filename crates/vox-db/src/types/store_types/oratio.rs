use serde::{Deserialize, Serialize};

/// Row returned for an evaluation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OratioEvalRunRecord {
    pub run_id: String,
    pub run_type: String, // 'general_subtitle' | 'code_domain'
    pub backend: String,
    pub model_id: Option<String>,
    pub dataset_name: String,
    pub sample_count: u32,
    pub total_ref_words: u32,
    pub total_wer_errors: u32,
    pub global_wer: Option<f32>,
    pub global_cer: Option<f32>,
    pub avg_latency_ms: Option<f32>,
    pub avg_timing_offset_ms: Option<f32>,
    pub status: String,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct OratioEvalRunStartParams {
    pub run_id: String,
    pub run_type: String,
    pub backend: String,
    pub model_id: Option<String>,
    pub dataset_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OratioEvalSampleRecord {
    pub id: i64,
    pub run_id: String,
    pub audio_path: String,
    pub reference_text: String,
    pub hypothesis_text: String,
    pub wer: f32,
    pub cer: f32,
    pub latency_ms: Option<i64>,
    pub segment_count: Option<i32>,
    pub no_speech_dropped: i32,
    pub created_at: i64,
}
