//! Trust-tier gating for forwarded secrets injected into dispatched remote-worker
//! execution.
//!
//! The mesh forwards a [`SecretBag`] (the sender's grant) alongside a dispatched
//! task that declares which secrets it needs. This module decides what is *safe*
//! to inject into the dispatched subprocess, by execution tier:
//!
//! - **BareMetal** (source via `vox run --mode interp`, native binary): NO
//!   isolation — an injected secret is trivially exfiltrated (`env | curl …`), so
//!   **nothing** is forwarded. This is the only live tier today, so the current
//!   shippable behavior injects zero secrets — the correct, safe answer.
//! - **Sandboxed** (wasmtime/WASI): low-sensitivity declared secrets only; never
//!   credentials.
//!
//! Sensitivity is *derived* from the existing `SecretSpec` (a secret with an
//! `auth_registry` or a `required` policy is a credential) — no new data.

use super::secret_bag::SecretBag;

/// Isolation tier of the dispatched execution lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecTier {
    /// No isolation (source interpreter / native binary). Injects nothing.
    BareMetal,
    /// wasmtime/WASI sandbox. Low-sensitivity declared secrets only.
    Sandboxed,
}

/// Secret sensitivity, derived from `SecretSpec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sensitivity {
    /// Non-credential config (model ids, routing knobs).
    LowValue,
    /// API keys / registry tokens / required credentials.
    Credential,
}

/// Classify a declared secret (by its `SecretId` name) via the existing spec.
/// Unknown names and any spec with an `auth_registry` or a `required` policy are
/// treated as [`Sensitivity::Credential`] (deny on a sandboxed tier).
fn sensitivity_of(declared_name: &str) -> Sensitivity {
    use std::str::FromStr;
    let Ok(id) = vox_secrets::SecretId::from_str(declared_name) else {
        return Sensitivity::Credential;
    };
    match vox_secrets::all_specs().into_iter().find(|s| s.id == id) {
        Some(s) if s.auth_registry.is_none() && !s.policy.required => Sensitivity::LowValue,
        _ => Sensitivity::Credential,
    }
}

/// Decide which forwarded secrets to inject into the dispatched subprocess env.
///
/// Returns `bag ∩ declared ∩ tier-allowed` as `(ENV_KEY, value)` pairs. The bag
/// is the sender's grant; `declared` is the task's stated need (the
/// `required_secrets` list); the tier sets the sensitivity ceiling. BareMetal
/// always returns empty.
#[must_use]
pub fn gate_secrets(tier: ExecTier, declared: &[String], bag: &SecretBag) -> Vec<(String, String)> {
    match tier {
        ExecTier::BareMetal => Vec::new(),
        ExecTier::Sandboxed => {
            let allowed: Vec<String> = declared
                .iter()
                .filter(|name| sensitivity_of(name) == Sensitivity::LowValue)
                .cloned()
                .collect();
            bag.env_for_declared(&allowed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a2a::secret_bag::SecretBag;

    fn bag() -> SecretBag {
        // Keyed by SecretId name (the JWE bag's wire form). OpenRouterApiKey is a
        // credential (auth_registry); VoxOpenRouterChatModel is low-value config.
        SecretBag::from_decrypted(serde_json::json!({
            "OpenRouterApiKey": "sk-secret",
            "VoxOpenRouterChatModel": "some/model",
        }))
        .expect("bag")
    }

    #[test]
    fn baremetal_injects_nothing_even_with_declared_and_bag() {
        let declared = vec![
            "OpenRouterApiKey".to_string(),
            "VoxOpenRouterChatModel".to_string(),
        ];
        assert!(
            gate_secrets(ExecTier::BareMetal, &declared, &bag()).is_empty(),
            "BareMetal has no isolation — it must never receive forwarded secrets"
        );
    }

    #[test]
    fn sandboxed_injects_low_value_but_filters_credentials() {
        let declared = vec![
            "OpenRouterApiKey".to_string(),       // credential → filtered
            "VoxOpenRouterChatModel".to_string(), // low-value → injected
        ];
        let injected = gate_secrets(ExecTier::Sandboxed, &declared, &bag());
        assert_eq!(
            injected.len(),
            1,
            "only the low-value secret should be injected"
        );
        assert_eq!(injected[0].1, "some/model");
        assert!(
            !injected.iter().any(|(_, v)| v == "sk-secret"),
            "the credential must never be injected, even into a sandbox"
        );
    }

    #[test]
    fn sensitivity_classifier_matches_spec() {
        assert_eq!(sensitivity_of("OpenRouterApiKey"), Sensitivity::Credential);
        assert_eq!(
            sensitivity_of("VoxOpenRouterChatModel"),
            Sensitivity::LowValue
        );
        assert_eq!(
            sensitivity_of("totally-unknown-secret"),
            Sensitivity::Credential
        );
    }
}
