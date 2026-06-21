use anyhow::Result;

/// Provenance manifest for a trained LoRA adapter. Written as `adapter_card.json`
/// alongside the adapter weights. Fail-closed: required fields must be non-empty.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdapterCard {
    /// HF repo id of the base model (e.g., "Qwen/Qwen3-14B@PLACEHOLDER-c4e8f122").
    pub base_hf_id: String,
    /// HF commit revision the adapter was trained against.
    pub base_revision: String,
    /// VRAM tier rung used for training (e.g., "qwen3_16g").
    pub base_rung: String,
    /// Quantization method (e.g., "qlora", "lora", "none").
    pub quantization: String,
    pub lora_rank: u32,
    pub lora_alpha: f32,
    pub seed: u64,
    pub corpus_hash: String,
    pub preset_version: String,
    pub metrics: serde_json::Value,
    pub cost_usd: f64,
    pub provider: String, // "local", "runpod", "vast"
    pub git_sha: String,
    pub created: String, // ISO-8601
}

impl AdapterCard {
    /// Fail-closed: returns Err if base_rung, quantization, or base_revision are empty.
    pub fn validate(&self) -> Result<()> {
        if self.base_rung.is_empty() {
            anyhow::bail!("AdapterCard.base_rung must not be empty");
        }
        if self.quantization.is_empty() {
            anyhow::bail!("AdapterCard.quantization must not be empty");
        }
        if self.base_revision.is_empty() {
            anyhow::bail!("AdapterCard.base_revision must not be empty");
        }
        Ok(())
    }

    /// Returns false if rung or quantization do not match this card (serve-side check).
    pub fn is_compatible_with(&self, serve_rung: &str, serve_quant: &str) -> bool {
        self.base_rung == serve_rung && self.quantization == serve_quant
    }

    /// Write this card as `adapter_card.json` adjacent to `adapter_path`.
    pub fn write_sidecar(&self, adapter_path: &std::path::Path) -> Result<()> {
        let sidecar = adapter_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("adapter path has no parent"))?
            .join("adapter_card.json");
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&sidecar, json)?;
        Ok(())
    }

    /// Read card from `adapter_card.json` adjacent to `adapter_path`. Returns None if absent.
    pub fn read_sidecar(adapter_path: &std::path::Path) -> Result<Option<Self>> {
        let sidecar = adapter_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("adapter path has no parent"))?
            .join("adapter_card.json");
        if !sidecar.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&sidecar)?;
        Ok(Some(serde_json::from_str(&json)?))
    }

    /// Convenience constructor for tests (non-empty required fields).
    pub fn for_test(rung: &str, quant: &str) -> Self {
        Self {
            base_hf_id: "test/model@abc123".to_string(),
            base_revision: "abc123".to_string(),
            base_rung: rung.to_string(),
            quantization: quant.to_string(),
            lora_rank: 16,
            lora_alpha: 32.0,
            seed: 42,
            corpus_hash: "test_hash".to_string(),
            preset_version: "1".to_string(),
            metrics: serde_json::Value::Null,
            cost_usd: 0.0,
            provider: "local".to_string(),
            git_sha: "abc".to_string(),
            created: "2026-06-21T00:00:00Z".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_empty_rung() {
        let mut card = AdapterCard::for_test("", "qlora");
        card.base_revision = "abc".to_string();
        assert!(card.validate().is_err(), "empty base_rung must fail validate");
    }

    #[test]
    fn validate_rejects_empty_quantization() {
        let mut card = AdapterCard::for_test("qwen3_16g", "");
        assert!(
            card.validate().is_err(),
            "empty quantization must fail validate"
        );
    }

    #[test]
    fn validate_rejects_empty_revision() {
        let mut card = AdapterCard::for_test("qwen3_16g", "qlora");
        card.base_revision = "".to_string();
        assert!(
            card.validate().is_err(),
            "empty base_revision must fail validate"
        );
    }

    #[test]
    fn validate_passes_with_all_required_fields() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        card.validate().unwrap();
    }

    #[test]
    fn is_compatible_with_matches_rung_and_quant() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        assert!(card.is_compatible_with("qwen3_16g", "qlora"));
    }

    #[test]
    fn is_compatible_with_rejects_rung_mismatch() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        assert!(!card.is_compatible_with("qwen3_24g", "qlora"));
    }

    #[test]
    fn is_compatible_with_rejects_quant_mismatch() {
        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        assert!(!card.is_compatible_with("qwen3_16g", "lora"));
    }

    #[test]
    fn sidecar_round_trip() {
        let tmp = std::env::temp_dir().join("vox_adapter_card_sidecar_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let adapter_path = tmp.join("adapter_model.safetensors");
        std::fs::write(&adapter_path, b"fake").unwrap();

        let card = AdapterCard::for_test("qwen3_16g", "qlora");
        card.write_sidecar(&adapter_path).unwrap();

        let loaded = AdapterCard::read_sidecar(&adapter_path).unwrap().unwrap();
        assert_eq!(loaded.base_rung, "qwen3_16g");
        assert_eq!(loaded.quantization, "qlora");
        assert_eq!(loaded.base_revision, "abc123");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn read_sidecar_returns_none_when_absent() {
        let tmp = std::env::temp_dir().join("vox_adapter_card_no_sidecar_test");
        std::fs::create_dir_all(&tmp).unwrap();
        let adapter_path = tmp.join("adapter_model.safetensors");
        std::fs::write(&adapter_path, b"fake").unwrap();

        let result = AdapterCard::read_sidecar(&adapter_path).unwrap();
        assert!(result.is_none());

        std::fs::remove_dir_all(&tmp).ok();
    }
}
