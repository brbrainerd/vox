//! P0-T4: task-scoped decrypted secrets, gated by `@uses(secret)` declarations.
//!
//! `SecretBag` owns the plaintext for the duration of one remote task. It
//! never enters the process environment unbidden — only secrets the skill
//! declares via `@uses(secret)` are projected into `RunOpts.env`.

use std::collections::HashMap;

#[derive(Clone)]
pub struct SecretBag {
    plaintexts: HashMap<String, String>,
}

impl std::fmt::Debug for SecretBag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut redacted: Vec<(&str, String)> = self
            .plaintexts
            .iter()
            .map(|(k, v)| (k.as_str(), format!("[redacted len={}]", v.len())))
            .collect();
        redacted.sort_by_key(|(k, _)| *k);
        f.debug_struct("SecretBag")
            .field("entries", &redacted)
            .finish()
    }
}

impl SecretBag {
    pub fn from_decrypted(value: serde_json::Value) -> Result<Self, String> {
        let map: HashMap<String, String> = serde_json::from_value(value)
            .map_err(|e| format!("SecretBag: expected object<string,string>: {e}"))?;
        Ok(Self { plaintexts: map })
    }

    /// Project the bag into `(env_key, value)` pairs for the skill runtime.
    ///
    /// `declared` is the list of `@uses(secret)` SecretIds parsed from the
    /// skill's effect annotations. Secrets not declared are never returned.
    pub fn env_for_declared(&self, declared: &[String]) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(declared.len());
        for id in declared {
            if let Some(plaintext) = self.plaintexts.get(id) {
                let env_key = secret_id_to_env_key(id);
                out.push((env_key, plaintext.clone()));
            }
        }
        out
    }

    /// Number of secrets in the bag. Used for telemetry; does NOT leak names.
    pub fn len(&self) -> usize {
        self.plaintexts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plaintexts.is_empty()
    }
}

/// Map a SecretId (CamelCase Rust variant name) to SCREAMING_SNAKE env-var key.
/// e.g. `VoxGitHubToken` → `VOX_GIT_HUB_TOKEN`.
fn secret_id_to_env_key(id: &str) -> String {
    let mut out = String::with_capacity(id.len() + 4);
    for (i, c) in id.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            out.push('_');
        }
        for u in c.to_uppercase() {
            out.push(u);
        }
    }
    out
}

#[cfg(test)]
mod unit {
    use super::*;
    #[test]
    fn env_key_camel_to_snake() {
        assert_eq!(secret_id_to_env_key("VoxGitHubToken"), "VOX_GIT_HUB_TOKEN");
        assert_eq!(secret_id_to_env_key("Foo"), "FOO");
    }
}
