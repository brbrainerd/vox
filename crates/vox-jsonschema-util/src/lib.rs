//! Compile [`serde_json::Value`] JSON Schemas with [`jsonschema`] and validate instances with stable `anyhow` context.
//!
//! File I/O stays with callers (e.g. `vox_bounded_fs`); this crate only centralizes compile/validate.

#![forbid(unsafe_code)]

use std::path::Path;

use anyhow::{Context as _, anyhow};
pub mod codegen;

pub use jsonschema::Validator;
use serde_json::Value;

/// Build a [`jsonschema::Validator`] from a parsed schema document.
pub fn compile_validator(
    schema: &Value,
    context: impl std::fmt::Display,
) -> anyhow::Result<jsonschema::Validator> {
    jsonschema::validator_for(schema).with_context(|| format!("compile JSON Schema ({context})"))
}

/// Compile after reading UTF-8 JSON from `path` (caller-supplied reader for caps / policy).
pub fn compile_validator_from_utf8(
    schema_src: &str,
    path: &Path,
) -> anyhow::Result<jsonschema::Validator> {
    let schema_val: Value =
        serde_json::from_str(schema_src).with_context(|| format!("parse {}", path.display()))?;
    compile_validator(&schema_val, path.display())
}

/// Run validation; errors include `context` and the underlying `jsonschema` diagnostic chain.
pub fn validate(
    instance: &Value,
    validator: &jsonschema::Validator,
    context: impl std::fmt::Display,
) -> anyhow::Result<()> {
    if let Err(e) = validator.validate(instance) {
        return Err(anyhow!(
            "JSON Schema validation ({context}): path {}: {e:#}",
            e.instance_path
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compile_and_validate_simple() {
        let schema = json!({
            "type": "object",
            "properties": { "x": { "type": "integer" } },
            "required": ["x"],
            "additionalProperties": false
        });
        let v = compile_validator(&schema, "test").expect("compile");
        validate(&json!({"x": 1}), &v, "ok").expect("valid");
        assert!(validate(&json!({}), &v, "bad").is_err());
    }
}
