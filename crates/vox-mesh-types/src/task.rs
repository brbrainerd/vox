use serde::{Deserialize, Serialize};

use crate::attestation::Attestation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    TextInfer,
    ImageGen,
    SpeechTranscribe,
    TrainQLoRA,
    Embed,
    VoxScript,
}

impl std::fmt::Display for TaskKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl TaskKind {
    /// Return the canonical snake_case string for this task kind.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextInfer => "text_infer",
            Self::ImageGen => "image_gen",
            Self::SpeechTranscribe => "speech_transcribe",
            Self::TrainQLoRA => "train_qlora",
            Self::Embed => "embed",
            Self::VoxScript => "vox_script",
        }
    }

    /// Parse a task kind from a loose string, falling back to `VoxScript` for
    /// unknown values. This is used by the policy file parser for forward
    /// compatibility with future task kinds stored as plain strings.
    pub fn from_str_loose(s: &str) -> Self {
        match s {
            "text_infer" => Self::TextInfer,
            "image_gen" => Self::ImageGen,
            "speech_transcribe" => Self::SpeechTranscribe,
            "train_qlora" => Self::TrainQLoRA,
            "embed" => Self::Embed,
            _ => Self::VoxScript,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub kind: TaskKind,
    pub model_id: Option<String>,
    pub min_vram_mb: Option<u32>,
    pub priority: u8,
    pub timeout_secs: u64,
    pub payload_b64: String,
    pub source_blake3_hex: Option<String>,
    pub required_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: String,
    pub node_id: String,
    pub success: bool,
    pub output_b64: String,
    pub duration_ms: u64,
    pub payload_blake3_hex: Option<String>,
    /// Legacy flat signature field; superseded by `attestation` (P5-T4).
    pub worker_ed25519_sig_b64: Option<String>,
    /// Structured signed attestation envelope (P5-T4). Absent for legacy results.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation: Option<Attestation>,
}
