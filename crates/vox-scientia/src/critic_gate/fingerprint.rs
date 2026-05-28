//! Model fingerprints used for the GPT-4-grades-GPT-4 exclusion check.
//!
//! A fingerprint is a stable hash over `(provider, model_id, parameter_count_hint,
//! training_cutoff_date)`. Two fingerprints match iff they identify the same
//! deployable model surface — which is the contract for "no LLM may grade
//! its own outputs."
//!
//! Caveats:
//!
//! - `parameter_count_hint` is sometimes unknown (closed models). We accept
//!   `None` and degrade gracefully — the match check then ignores that
//!   component. This is *strictly more permissive*, but the rubric stays
//!   safe because the next two components still suffice when training
//!   cutoffs differ.
//! - We deliberately do NOT include "deployment version" (e.g., snapshot
//!   date inside a single model name); two snapshots of the same architecture
//!   trained on the same corpus must collide.

use serde::{Deserialize, Serialize};

/// Identifying surface for a deployable model.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelFingerprint {
    /// e.g., `"anthropic"`, `"openai"`, `"mistral"`. Compared
    /// case-insensitively.
    pub provider: String,
    /// e.g., `"claude-3-5-sonnet"`, `"gpt-4o"`. Compared case-insensitively
    /// with `_-/.` collapsed to `-`.
    pub model_id: String,
    /// e.g., `8_000_000_000` for an 8B model. `None` for closed models.
    pub parameter_count_hint: Option<u64>,
    /// e.g., `"2024-10"` for a training cutoff month. Compared as opaque
    /// string after lowercasing.
    pub training_cutoff: Option<String>,
}

impl ModelFingerprint {
    /// Two fingerprints are *colliding* iff they would let one model grade
    /// its own output. The rule is:
    ///
    /// - provider AND normalized-model-id must match, OR
    /// - provider matches AND training_cutoff matches AND parameter_count_hint
    ///   matches.
    ///
    /// In practice this catches "claude-3-5-sonnet" judging its own output
    /// regardless of whether the deployment renames are slightly different.
    pub fn collides_with(&self, other: &Self) -> bool {
        if !eq_ci(&self.provider, &other.provider) {
            return false;
        }
        if normalize_model_id(&self.model_id) == normalize_model_id(&other.model_id) {
            return true;
        }
        match (&self.training_cutoff, &other.training_cutoff) {
            (Some(a), Some(b)) if eq_ci(a, b) => {}
            _ => return false,
        }
        match (self.parameter_count_hint, other.parameter_count_hint) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn normalize_model_id(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| if matches!(c, '_' | '/' | '.' | ' ') { '-' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fp(provider: &str, model: &str, params: Option<u64>, cutoff: Option<&str>) -> ModelFingerprint {
        ModelFingerprint {
            provider: provider.into(),
            model_id: model.into(),
            parameter_count_hint: params,
            training_cutoff: cutoff.map(str::to_string),
        }
    }

    #[test]
    fn same_provider_and_model_collide() {
        let a = fp("anthropic", "claude-3-5-sonnet", None, Some("2024-10"));
        let b = fp("anthropic", "claude-3-5-sonnet", None, Some("2024-10"));
        assert!(a.collides_with(&b));
    }

    #[test]
    fn model_id_normalization_collapses_separators() {
        let a = fp("anthropic", "claude_3.5_sonnet", None, None);
        let b = fp("anthropic", "claude-3-5-sonnet", None, None);
        assert!(a.collides_with(&b));
    }

    #[test]
    fn different_providers_do_not_collide() {
        let a = fp("anthropic", "claude-3-5-sonnet", None, None);
        let b = fp("openai", "claude-3-5-sonnet", None, None);
        assert!(!a.collides_with(&b));
    }

    #[test]
    fn different_models_same_provider_do_not_collide_without_params_or_cutoff() {
        let a = fp("openai", "gpt-4o", None, None);
        let b = fp("openai", "gpt-3.5", None, None);
        assert!(!a.collides_with(&b));
    }

    #[test]
    fn same_provider_same_cutoff_and_params_collides_even_with_different_model_string() {
        let a = fp("acme", "model-snapshot-a", Some(8_000_000_000), Some("2024-10"));
        let b = fp("acme", "model-snapshot-b", Some(8_000_000_000), Some("2024-10"));
        assert!(a.collides_with(&b));
    }

    #[test]
    fn same_cutoff_but_different_params_does_not_collide() {
        let a = fp("acme", "model-a", Some(8_000_000_000), Some("2024-10"));
        let b = fp("acme", "model-b", Some(70_000_000_000), Some("2024-10"));
        assert!(!a.collides_with(&b));
    }

    #[test]
    fn provider_match_is_case_insensitive() {
        let a = fp("ANTHROPIC", "claude-3-5-sonnet", None, None);
        let b = fp("anthropic", "claude-3-5-sonnet", None, None);
        assert!(a.collides_with(&b));
    }

    #[test]
    fn missing_params_or_cutoff_falls_back_to_model_id_check_only() {
        // No params, no cutoff → must rely on normalized model id.
        let a = fp("acme", "x", None, None);
        let b = fp("acme", "y", None, None);
        assert!(!a.collides_with(&b));
    }
}
