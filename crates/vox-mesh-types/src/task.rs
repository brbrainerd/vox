use serde::{Deserialize, Serialize};

use crate::attestation::Attestation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    TextInfer,
    ImageGen,
    SpeechTranscribe,
    #[serde(rename = "train_qlora")]
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

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: from_str_loose not falling back to VoxScript for genuinely unknown variants.
    #[test]
    fn unknown_kind_string_falls_back_to_vox_script() {
        assert_eq!(TaskKind::from_str_loose("unknown_future_kind"), TaskKind::VoxScript);
        assert_eq!(TaskKind::from_str_loose(""), TaskKind::VoxScript);
    }

    // Catches: from_str_loose not recognising a known variant (e.g., wrong snake_case).
    #[test]
    fn known_kind_strings_parse_correctly() {
        assert_eq!(TaskKind::from_str_loose("text_infer"), TaskKind::TextInfer);
        assert_eq!(TaskKind::from_str_loose("image_gen"), TaskKind::ImageGen);
        assert_eq!(TaskKind::from_str_loose("speech_transcribe"), TaskKind::SpeechTranscribe);
        assert_eq!(TaskKind::from_str_loose("train_qlora"), TaskKind::TrainQLoRA);
        assert_eq!(TaskKind::from_str_loose("embed"), TaskKind::Embed);
    }

    // Catches: as_str returning a string that doesn't round-trip through from_str_loose.
    #[test]
    fn as_str_and_from_str_loose_are_consistent() {
        for kind in [
            TaskKind::TextInfer,
            TaskKind::ImageGen,
            TaskKind::SpeechTranscribe,
            TaskKind::TrainQLoRA,
            TaskKind::Embed,
        ] {
            assert_eq!(
                TaskKind::from_str_loose(kind.as_str()),
                kind,
                "as_str() must round-trip through from_str_loose for {kind}"
            );
        }
    }

    // Catches: TaskKind serde rename_all="snake_case" producing "train_q_lo_r_a" for TrainQLoRA
    // instead of "train_qlora" (BUG: serde's snake_case transform splits on each uppercase letter,
    // so "QLoRA" → "q_lo_r_a"). This test documents the discrepancy between as_str() and serde.
    #[test]
    fn train_qlora_serde_vs_as_str_discrepancy() {
        let json = serde_json::to_string(&TaskKind::TrainQLoRA).unwrap();
        // as_str() returns the correct "train_qlora" but serde produces "train_q_lo_r_a".
        // Either: add #[serde(rename = "train_qlora")] to TrainQLoRA, or rename the variant.
        let as_str_json = format!("\"{}\"", TaskKind::TrainQLoRA.as_str());
        assert_eq!(
            json, as_str_json,
            "TrainQLoRA serde output must match as_str(); \
             fix: add #[serde(rename = \"train_qlora\")] to the TrainQLoRA variant"
        );
    }

    // Catches: other TaskKind variants' serde producing wrong JSON tags.
    #[test]
    fn task_kind_other_variants_serialise_correctly() {
        let json = serde_json::to_string(&TaskKind::TextInfer).unwrap();
        assert_eq!(json, "\"text_infer\"");
        let json2 = serde_json::to_string(&TaskKind::ImageGen).unwrap();
        assert_eq!(json2, "\"image_gen\"");
        let json3 = serde_json::to_string(&TaskKind::Embed).unwrap();
        assert_eq!(json3, "\"embed\"");
    }

    // Catches: TaskKind::Display not matching as_str output.
    #[test]
    fn display_matches_as_str() {
        assert_eq!(TaskKind::ImageGen.to_string(), TaskKind::ImageGen.as_str());
        assert_eq!(TaskKind::VoxScript.to_string(), TaskKind::VoxScript.as_str());
    }
}
