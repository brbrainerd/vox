#![cfg_attr(test, allow(unsafe_code))] // test-only std::env::set_var (unsafe on edition 2024)
//! B4 — Schema-constrained decoding helpers for vLLM / outlines-backed inference servers.
//!
//! Wraps the `guided_json` / `guided_decoding_backend` vLLM request extension in a
//! typed struct and provides helpers to attach that spec to an arbitrary JSON request.
//!
//! # Config knob
//! The backend string is read from `VOX_MENS_GUIDED_DECODING_BACKEND` (env) or
//! `~/.vox/config.toml` under the same key.  When absent the compiled-in default
//! [`crate::schema_guided::DEFAULT_GUIDED_DECODING_BACKEND`] ("outlines") is used.
//!
//! No secret is involved; resolution goes through [`vox_config::env_parse::resolve_config_str`].

/// Compiled-in default backend when no config key is present.
pub const DEFAULT_GUIDED_DECODING_BACKEND: &str = "outlines";

/// The env / config-toml key that overrides [`DEFAULT_GUIDED_DECODING_BACKEND`].
pub const GUIDED_DECODING_BACKEND_KEY: &str = "VOX_MENS_GUIDED_DECODING_BACKEND";

/// Resolves the guided-decoding backend string from config with fallback to the default.
fn resolve_backend() -> String {
    vox_config::env_parse::resolve_config_str(
        GUIDED_DECODING_BACKEND_KEY,
        DEFAULT_GUIDED_DECODING_BACKEND,
    )
}

/// The typed `guided_json` + `guided_decoding_backend` fragment that vLLM accepts.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuidedDecodingSpec {
    /// The JSON Schema the model must conform to during generation.
    pub guided_json: serde_json::Value,
    /// The constrained-decoding backend (e.g. `"outlines"`, `"lm-format-enforcer"`).
    pub guided_decoding_backend: String,
}

/// Build a [`GuidedDecodingSpec`] from a JSON Schema value.
///
/// The backend is resolved from config; see module-level docs.
pub fn to_guided_decoding_spec(schema: &serde_json::Value) -> GuidedDecodingSpec {
    GuidedDecodingSpec {
        guided_json: schema.clone(),
        guided_decoding_backend: resolve_backend(),
    }
}

/// Merge a [`GuidedDecodingSpec`] into an existing vLLM-compatible request object.
///
/// The two extra fields (`guided_json`, `guided_decoding_backend`) are inserted at the
/// top level of `req`.  If `req` is not a JSON object the function returns it unchanged.
pub fn attach_guided_decoding(
    mut req: serde_json::Value,
    tool_schema: &serde_json::Value,
) -> serde_json::Value {
    if let Some(obj) = req.as_object_mut() {
        let spec = to_guided_decoding_spec(tool_schema);
        obj.insert("guided_json".into(), spec.guided_json);
        obj.insert(
            "guided_decoding_backend".into(),
            serde_json::Value::String(spec.guided_decoding_backend),
        );
    }
    req
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Rust 2024 made std::env::{set_var,remove_var} unsafe; single-threaded test binary.
    #![allow(unsafe_code)]
    use super::*;
    use serde_json::json;

    // Minimal schema used throughout the tests.
    fn simple_schema() -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "count": { "type": "integer" }
            },
            "required": ["name"]
        })
    }

    // ── to_guided_decoding_spec ──────────────────────────────────────────────

    #[test]
    fn spec_contains_schema() {
        let schema = simple_schema();
        let spec = to_guided_decoding_spec(&schema);
        assert_eq!(
            spec.guided_json, schema,
            "guided_json must equal the input schema"
        );
    }

    #[test]
    fn spec_backend_is_non_empty_string() {
        let spec = to_guided_decoding_spec(&simple_schema());
        assert!(
            !spec.guided_decoding_backend.is_empty(),
            "guided_decoding_backend must not be empty"
        );
    }

    #[test]
    fn spec_backend_default_is_outlines() {
        // When VOX_MENS_GUIDED_DECODING_BACKEND is not set in the test environment
        // (and no ~/.vox/config.toml entry is present), the backend must fall back to
        // the compiled-in default "outlines".
        //
        // We cannot guarantee the env is clean, so we only assert the default constant
        // itself is the right literal.
        assert_eq!(DEFAULT_GUIDED_DECODING_BACKEND, "outlines");
    }

    #[test]
    fn spec_backend_env_override() {
        // Set the env var and verify resolve_backend picks it up.
        // SAFETY: single-threaded test binary; no other thread reads this env var concurrently.
        let key = GUIDED_DECODING_BACKEND_KEY;
        let prev = std::env::var(key).ok();
        unsafe { std::env::set_var(key, "lm-format-enforcer") };
        let backend = resolve_backend();
        // Restore.
        unsafe {
            match prev {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        assert_eq!(backend, "lm-format-enforcer");
    }

    #[test]
    fn spec_is_serializable_to_json_object() {
        let spec = to_guided_decoding_spec(&simple_schema());
        let v = serde_json::to_value(&spec).expect("GuidedDecodingSpec must serialize");
        assert!(v.is_object(), "serialized spec must be a JSON object");
        assert!(v.get("guided_json").is_some(), "must have guided_json key");
        assert!(
            v.get("guided_decoding_backend").is_some(),
            "must have guided_decoding_backend key"
        );
    }

    // ── attach_guided_decoding ───────────────────────────────────────────────

    #[test]
    fn attach_adds_guided_json_field() {
        let req = json!({ "model": "qwen3-8b", "messages": [] });
        let schema = simple_schema();
        let out = attach_guided_decoding(req, &schema);
        assert_eq!(
            out.get("guided_json").expect("guided_json must be present"),
            &schema
        );
    }

    #[test]
    fn attach_adds_guided_decoding_backend_field() {
        let req = json!({ "model": "qwen3-8b", "messages": [] });
        let out = attach_guided_decoding(req, &simple_schema());
        let backend = out
            .get("guided_decoding_backend")
            .and_then(|v| v.as_str())
            .expect("guided_decoding_backend must be a string");
        assert!(!backend.is_empty(), "backend must not be empty");
    }

    #[test]
    fn attach_preserves_existing_fields() {
        let req = json!({ "model": "qwen3-8b", "temperature": 0.7, "messages": [] });
        let out = attach_guided_decoding(req, &simple_schema());
        assert_eq!(out.get("model").and_then(|v| v.as_str()), Some("qwen3-8b"));
        assert_eq!(out.get("temperature").and_then(|v| v.as_f64()), Some(0.7));
    }

    #[test]
    fn attach_non_object_is_returned_unchanged() {
        // If the request is not a JSON object, it must come back unmodified.
        let req = json!("not-an-object");
        let out = attach_guided_decoding(req.clone(), &simple_schema());
        assert_eq!(out, req);
    }

    #[test]
    fn attach_idempotent_schema_in_output() {
        // Calling attach twice overwrites with the same schema — last write wins.
        let req = json!({ "model": "x" });
        let schema = simple_schema();
        let once = attach_guided_decoding(req, &schema);
        let twice = attach_guided_decoding(once, &schema);
        assert_eq!(
            twice
                .get("guided_json")
                .expect("guided_json after two attaches"),
            &schema
        );
    }
}
