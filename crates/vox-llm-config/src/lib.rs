//! SSOT for LLM/AI setting keys.
//!
//! `LLM_CONFIG_KEYS` is the single home for every LLM/AI provider/endpoint/model/
//! tuning/budget setting. `vox-secrets` (secret resolution), `vox-config` (typed
//! accessors), and `vox-gui` (the Runtime settings catalog) are all **views** over
//! this table — never parallel copies. Drift is forbidden by parity tests in those
//! crates.
//!
//! This crate is layer 0: pure data, zero workspace deps. It therefore holds only
//! key *metadata* (env name, kind, group, label, hint, literal default, secret flag,
//! persistence + config class) — never secret *values* (those resolve via Clavis in
//! `vox-secrets`) and never upward calls into accessor crates (defaults are literals;
//! the typed env-resolving accessors live in `vox-config`).

/// One LLM/AI setting. `env` is the canonical identity every view keys on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlmConfigKey {
    /// Canonical environment-variable name, e.g. `"OPENROUTER_BASE_URL"`.
    pub env: &'static str,
    /// Display default rendered when the key is unset (empty string = "unset").
    pub default: &'static str,
    pub kind: Kind,
    pub group: Group,
    pub class: ConfigClass,
    pub label: &'static str,
    pub hint: &'static str,
    /// Allowed values when `kind == Enum`; empty otherwise.
    pub options: &'static [&'static str],
    /// `true` → value resolves via Clavis in `vox-secrets`; never written to config.toml.
    pub secret: bool,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    String,
    Url,
    Float,
    Int,
    Bool,
    Path,
    Enum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    ModelsAndEndpoints,
    Tuning,
    Training,
}

/// Where a non-secret key is persisted. Secrets are `EnvOnly` (Clavis-backed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persistence {
    /// Sectioned `VoxConfig` field (`~/.vox/config.toml` `[vox]`/`[train]`/…).
    VoxConfig,
    /// Flat top-level key in `~/.vox/config.toml`.
    FlatToml,
    /// Resolved from environment / Clavis only; not stored in config.toml.
    EnvOnly,
}

/// Operator classification, mirrored from `vox_config::operator_registry::ConfigClass`
/// (kept here so this crate stays dependency-free; a parity test keeps them aligned).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigClass {
    UserPreference,
    NodeLocal,
    Bootstrap,
    CiGate,
}

/// GUI-facing projection of one non-secret key. `vox-gui` maps this to its Tauri DTO —
/// it does not own the field list.
#[derive(Debug, Clone)]
pub struct GuiField {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub group: &'static str,
    pub kind: &'static str,
    pub options: &'static [&'static str],
    pub default: &'static str,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::String => "string",
            Kind::Url => "string",
            Kind::Float => "float",
            Kind::Int => "int",
            Kind::Bool => "bool",
            Kind::Path => "path",
            Kind::Enum => "enum",
        }
    }
}

impl Group {
    pub fn as_str(self) -> &'static str {
        match self {
            Group::General => "General",
            Group::ModelsAndEndpoints => "Models & endpoints",
            Group::Tuning => "Tuning",
            Group::Training => "Training",
        }
    }
}

/// Look up a key by its canonical env name.
pub fn get(env: &str) -> Option<&'static LlmConfigKey> {
    LLM_CONFIG_KEYS.iter().find(|k| k.env == env)
}

/// GUI catalog: every non-secret key projected for the Runtime settings surface.
pub fn gui_fields() -> Vec<GuiField> {
    LLM_CONFIG_KEYS
        .iter()
        .filter(|k| !k.secret)
        .map(|k| GuiField {
            key: k.env,
            label: k.label,
            hint: k.hint,
            group: k.group.as_str(),
            kind: k.kind.as_str(),
            options: k.options,
            default: k.default,
        })
        .collect()
}

include!("keys.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_is_nonempty_and_keys_unique() {
        assert!(!LLM_CONFIG_KEYS.is_empty(), "registry must seed keys");
        let mut seen = HashSet::new();
        for k in LLM_CONFIG_KEYS {
            assert!(seen.insert(k.env), "duplicate key in registry: {}", k.env);
        }
    }

    #[test]
    fn secrets_are_env_only_and_have_no_visible_default() {
        for k in LLM_CONFIG_KEYS {
            if k.secret {
                assert_eq!(
                    k.persistence,
                    Persistence::EnvOnly,
                    "secret key {} must be EnvOnly (Clavis-backed)",
                    k.env
                );
                assert!(
                    k.default.is_empty(),
                    "secret key {} must not carry a display default",
                    k.env
                );
            }
        }
    }

    #[test]
    fn enum_keys_have_options() {
        for k in LLM_CONFIG_KEYS {
            if k.kind == Kind::Enum {
                assert!(!k.options.is_empty(), "enum key {} needs options", k.env);
            }
        }
    }

    #[test]
    fn gui_fields_exclude_secrets() {
        let fields = gui_fields();
        let secret_envs: HashSet<&str> = LLM_CONFIG_KEYS
            .iter()
            .filter(|k| k.secret)
            .map(|k| k.env)
            .collect();
        for f in &fields {
            assert!(
                !secret_envs.contains(f.key),
                "secret key {} leaked into gui_fields",
                f.key
            );
        }
    }
}
