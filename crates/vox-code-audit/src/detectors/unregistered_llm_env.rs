//! Flags `env::var("NAME")` reads of LLM/AI-shaped, **non-secret** env vars that are
//! not declared in the `vox-llm-config` SSOT. Secret-shaped names (`*_API_KEY`,
//! `*_TOKEN`, `*_SECRET`) are intentionally left to `EnvSecretShapeDetector` so the
//! two detectors do not double-flag.
//!
//! The registered set is read live from `vox_config::vox_llm_config::LLM_CONFIG_KEYS`
//! (vox-code-audit already depends on vox-config), so it cannot drift from the SSOT.

use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;
use std::collections::HashSet;

pub struct UnregisteredLlmEnvDetector {
    env_call: Regex,
    llm_shape: Regex,
    /// Names that count as "registered" somewhere in the SSOT planes: the
    /// `vox-llm-config` display registry OR the `vox-secrets` managed-secret set
    /// (canonical + aliases). Built once so per-line checks are O(1).
    known: HashSet<&'static str>,
}

impl Default for UnregisteredLlmEnvDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl UnregisteredLlmEnvDetector {
    pub fn new() -> Self {
        let mut known: HashSet<&'static str> = vox_config::vox_llm_config::LLM_CONFIG_KEYS
            .iter()
            .map(|k| k.env)
            .collect();
        // Keys declared in the vox-secrets registry (incl. Band-B routing keys still
        // resolved through the secret plane) are registered — do not flag them.
        known.extend(vox_secrets::spec::managed_secret_env_names());
        Self {
            env_call: Regex::new(r#"env::var(?:_os)?\(\s*"([A-Z0-9_]+)"\s*\)"#).expect("valid env_call regex"),
            llm_shape: Regex::new(
                r"^(OPENROUTER_|OPENAI_|ANTHROPIC_|GEMINI_|TOGETHER_|OLLAMA_TUNING_|HF_|HUGGINGFACE_|VOX_GEMINI_|POPULI_|GROQ_|MISTRAL_|DEEPSEEK_|SAMBANOVA_|CEREBRAS_)",
            )
            .expect("valid llm_shape regex"),
            known,
        }
    }

    /// Secret-shaped names are owned by `EnvSecretShapeDetector`; skip them here.
    fn is_secret_shaped(name: &str) -> bool {
        name.ends_with("_API_KEY") || name.ends_with("_TOKEN") || name.ends_with("_SECRET")
    }

    fn is_registered(&self, name: &str) -> bool {
        self.known.contains(name)
    }
}

impl DetectionRule for UnregisteredLlmEnvDetector {
    fn id(&self) -> &'static str {
        "vox/llm/unregistered-env"
    }

    fn name(&self) -> &'static str {
        "Unregistered LLM Env Detector"
    }

    fn description(&self) -> &'static str {
        "Flags env vars that tune LLM/AI behavior but are not declared in vox-llm-config (the SSOT)."
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn languages(&self) -> &[Language] {
        &[Language::Rust]
    }

    fn explain(&self) -> &'static str {
        "Every LLM/AI setting must be declared in the vox-llm-config registry so it surfaces \
         to the GUI and the CI tuning-knob allowlist. Reading an unregistered LLM-shaped env var \
         bypasses the single source of truth.\n\n\
         BAD:\n  let t = std::env::var(\"OPENROUTER_SECRET_TWEAK\").ok();\n\n\
         GOOD:\n  // add an LlmConfigKey entry, then read via the vox-config accessor backed by it"
    }

    fn detect(
        &self,
        file: &SourceFile,
        _ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
                continue;
            }
            for cap in self.env_call.captures_iter(line) {
                let name = &cap[1];
                if !self.llm_shape.is_match(name)
                    || Self::is_secret_shaped(name)
                    || self.is_registered(name)
                {
                    continue;
                }
                findings.push(Finding {
                    rule_id: self.id().to_string(),
                    diagnostic_id: None,
                    rule_name: self.name().to_string(),
                    severity: Severity::Error,
                    file: file.path.clone(),
                    line: i + 1,
                    column: 0,
                    message: format!(
                        "LLM/AI env `{name}` is not registered in vox-llm-config (the settings SSOT)."
                    ),
                    suggestion: Some(
                        "Add an LlmConfigKey entry for it in crates/vox-llm-config, then read it \
                         through the vox-config accessor backed by the registry."
                            .to_string(),
                    ),
                    alternatives: vec![],
                    rationale: Some(
                        "Unregistered LLM settings never reach the GUI or the CI tuning-knob \
                         allowlist, and silently drift the configuration surface."
                            .to_string(),
                    ),
                    context: file.context_around(i + 1, 2),
                    confidence: Some(FindingConfidence::High),
                    evidence: None,
                });
            }
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rs(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("t.rs"), code.to_string())
    }

    #[test]
    fn flags_unregistered_llm_shaped_env() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("OLLAMA_TUNING_MYSTERY").ok();"#);
        assert!(
            !d.detect(&f, None).is_empty(),
            "unregistered llm-shaped env should fire"
        );
    }

    #[test]
    fn ignores_registered_key() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("OPENROUTER_BASE_URL").ok();"#);
        assert!(
            d.detect(&f, None).is_empty(),
            "registered keys must not fire"
        );
    }

    #[test]
    fn ignores_secret_shaped_env() {
        // Owned by EnvSecretShapeDetector — must not double-flag here.
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("OPENROUTER_MYSTERY_API_KEY").unwrap();"#);
        assert!(
            d.detect(&f, None).is_empty(),
            "secret-shaped names are owned by env_secret_shape"
        );
    }

    #[test]
    fn ignores_band_b_keys_registered_in_vox_secrets() {
        // VOX_GEMINI_ROUTE_POLICY / GEMINI_DIRECT_MODEL are declared in vox-secrets
        // SPECS_LLM (Band-B, resolved via the secret plane) — must not fire here.
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let prev = std::env::var("VOX_GEMINI_ROUTE_POLICY").ok();
let direct = std::env::var("GEMINI_DIRECT_MODEL").ok();"#);
        assert!(
            d.detect(&f, None).is_empty(),
            "Band-B keys registered in vox-secrets must not be flagged"
        );
    }

    #[test]
    fn ignores_non_llm_env() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("PATH").unwrap();"#);
        assert!(d.detect(&f, None).is_empty(), "non-llm env must not fire");
    }

    #[test]
    fn matches_both_bare_and_std_qualified_forms() {
        // The regex is unanchored, so `std::env::var("X")` matches via the `env::var("X")`
        // substring. Lock this so neither call form silently escapes detection.
        let d = UnregisteredLlmEnvDetector::new();
        let bare = rs(r#"let a = env::var("OLLAMA_TUNING_MYSTERY").ok();"#);
        let qualified = rs(r#"let b = std::env::var("OLLAMA_TUNING_MYSTERY").ok();"#);
        assert!(!d.detect(&bare, None).is_empty(), "bare env::var must fire");
        assert!(
            !d.detect(&qualified, None).is_empty(),
            "std::env::var must fire"
        );
    }

    #[test]
    fn ignores_env_in_full_line_comment() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"// example: std::env::var("OLLAMA_TUNING_MYSTERY")"#);
        assert!(
            d.detect(&f, None).is_empty(),
            "commented-out code must not fire"
        );
    }
}
