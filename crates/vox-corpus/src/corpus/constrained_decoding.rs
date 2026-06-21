//! Modes for constrained generation (wired to inference servers).

/// How tightly to constrain token generation for structured outputs.
///
/// # Note on `Copy`
/// `SchemaGuided` carries a [`serde_json::Value`] payload, so `Copy` is intentionally
/// absent.  All callers must use `clone()` or take a `&ConstrainedDecodingMode` reference.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "schema", rename_all = "snake_case")]
pub enum ConstrainedDecodingMode {
    /// No extra constraints beyond sampling.
    #[default]
    None,
    /// Prefer valid JSON prefixes (logit processing in GPU path).
    JsonPrefix,
    /// Enforce strict JSON object shape post-hoc.
    StrictJson,
    /// Full schema-guided decoding: the inner [`serde_json::Value`] is forwarded as
    /// `guided_json` to the inference server (vLLM / outlines).
    ///
    /// `parse("schema_guided")` maps to [`Self::None`] with a warning because a
    /// `Value` argument cannot be recovered from a bare string label.
    SchemaGuided(serde_json::Value),
}

impl ConstrainedDecodingMode {
    /// Parse a snake-case or kebab-case label; unknown values map to [`Self::None`].
    ///
    /// `"schema_guided"` also maps to [`Self::None`] — a log warning is emitted because
    /// the variant requires a schema payload that cannot be recovered from a string label.
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "json_prefix" | "json-prefix" => ConstrainedDecodingMode::JsonPrefix,
            "strict_json" | "strict-json" => ConstrainedDecodingMode::StrictJson,
            "schema_guided" | "schema-guided" => {
                tracing::warn!(
                    "ConstrainedDecodingMode::parse: \
                     \"schema_guided\" requires a JSON schema payload and cannot be \
                     constructed from a string label alone — falling back to None"
                );
                ConstrainedDecodingMode::None
            }
            _ => ConstrainedDecodingMode::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_json_prefix() {
        assert_eq!(
            ConstrainedDecodingMode::parse("json_prefix"),
            ConstrainedDecodingMode::JsonPrefix
        );
        assert_eq!(
            ConstrainedDecodingMode::parse("json-prefix"),
            ConstrainedDecodingMode::JsonPrefix
        );
    }

    #[test]
    fn parse_strict_json() {
        assert_eq!(
            ConstrainedDecodingMode::parse("strict_json"),
            ConstrainedDecodingMode::StrictJson
        );
        assert_eq!(
            ConstrainedDecodingMode::parse("strict-json"),
            ConstrainedDecodingMode::StrictJson
        );
    }

    #[test]
    fn parse_unknown_maps_to_none() {
        assert_eq!(
            ConstrainedDecodingMode::parse("unknown"),
            ConstrainedDecodingMode::None
        );
    }

    #[test]
    fn parse_schema_guided_maps_to_none_with_no_panic() {
        // Must not panic; returns None (payload cannot be recovered from a string).
        assert_eq!(
            ConstrainedDecodingMode::parse("schema_guided"),
            ConstrainedDecodingMode::None
        );
        assert_eq!(
            ConstrainedDecodingMode::parse("schema-guided"),
            ConstrainedDecodingMode::None
        );
    }

    #[test]
    fn schema_guided_variant_carries_value() {
        let schema = json!({ "type": "object" });
        let mode = ConstrainedDecodingMode::SchemaGuided(schema.clone());
        match &mode {
            ConstrainedDecodingMode::SchemaGuided(v) => assert_eq!(v, &schema),
            other => panic!("expected SchemaGuided, got {:?}", other),
        }
    }

    #[test]
    fn schema_guided_round_trips_serde() {
        let schema = json!({ "type": "object", "properties": { "x": { "type": "integer" } } });
        let mode = ConstrainedDecodingMode::SchemaGuided(schema.clone());
        let serialized = serde_json::to_string(&mode).expect("serialize");
        let deserialized: ConstrainedDecodingMode =
            serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(deserialized, mode);
    }

    #[test]
    fn none_is_default() {
        assert_eq!(
            ConstrainedDecodingMode::default(),
            ConstrainedDecodingMode::None
        );
    }

    #[test]
    fn clone_works_for_schema_guided() {
        // Copy was removed; Clone must work instead.
        let mode = ConstrainedDecodingMode::SchemaGuided(json!({ "type": "string" }));
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
    }
}
