//! Refute-pass response parsing.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RefuteResponse {
    pub refuted: bool,
    pub refutation_note: String,
}

pub fn refute_json_schema() -> serde_json::Value {
    serde_json::json!({
      "type":"object",
      "properties":{
        "refuted":{"type":"boolean"},
        "refutation_note":{"type":"string"}
      },
      "required":["refuted","refutation_note"],
      "additionalProperties":false
    })
}

pub fn parse(raw: &str) -> Result<RefuteResponse, String> {
    let cleaned = crate::route::strip_json_fence(raw);
    serde_json::from_str(cleaned).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_refute() {
        let r = parse(r#"{"refuted":false,"refutation_note":"ok"}"#).unwrap();
        assert!(!r.refuted);
    }
    #[test]
    fn parses_refute_with_fence() {
        let r =
            parse("```\n{\"refuted\":true,\"refutation_note\":\"slips through\"}\n```").unwrap();
        assert!(r.refuted);
        assert_eq!(r.refutation_note, "slips through");
    }
}

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: parse() returning Ok with default false for malformed JSON (silent accept)
    #[test]
    fn parse_rejects_malformed_json() {
        let err = parse("not json at all");
        assert!(
            err.is_err(),
            "malformed JSON must produce Err, not a default-Ok"
        );
    }

    // Catches: parse() ignoring the refuted field and always returning false
    #[test]
    fn parse_correctly_reads_refuted_true() {
        let r = parse(r#"{"refuted":true,"refutation_note":"it breaks"}"#).unwrap();
        assert!(r.refuted, "refuted:true must parse as true, not false");
        assert_eq!(r.refutation_note, "it breaks");
    }

    // Catches: parse() accepting JSON missing required fields (schema too lenient)
    #[test]
    fn parse_rejects_missing_refuted_field() {
        // Missing the required 'refuted' field
        let err = parse(r#"{"refutation_note":"note only"}"#);
        assert!(
            err.is_err(),
            "JSON missing 'refuted' field must be rejected"
        );
    }

    // Catches: refute_json_schema producing wrong type for 'refuted' (e.g. string not bool)
    #[test]
    fn refute_json_schema_refuted_is_boolean_type() {
        let schema = refute_json_schema();
        let refuted_type = &schema["properties"]["refuted"]["type"];
        assert_eq!(
            refuted_type.as_str().unwrap_or(""),
            "boolean",
            "refuted field in schema must be type 'boolean'"
        );
    }
}
