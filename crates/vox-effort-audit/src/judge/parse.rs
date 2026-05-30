//! Robust parse of judge JSON response with one schema-error retry.

use crate::judge::schema::JudgeFinding;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("json parse failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema validation failed: {0}")]
    Schema(String),
}

/// Parse a judge response into a JudgeFinding. Strips common LLM artifacts
/// (leading ```json fences) before parsing.
pub fn parse(raw: &str) -> Result<JudgeFinding, ParseError> {
    let cleaned = strip_fence(raw);
    let v: serde_json::Value = serde_json::from_str(cleaned)?;
    validate_against_schema(&v)?;
    let f: JudgeFinding = serde_json::from_value(v)?;
    Ok(f)
}

/// Builds a corrective user message for a retry round, given the validator error.
pub fn retry_message(err: &ParseError) -> String {
    format!(
        "Your last response failed validation: {err}. \
         Re-emit ONLY the JSON object matching the schema. No prose, no fences."
    )
}

fn strip_fence(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_prefix("```json").or_else(|| s.strip_prefix("```")).unwrap_or(s);
    s.strip_suffix("```").unwrap_or(s).trim()
}

fn validate_against_schema(v: &serde_json::Value) -> Result<(), ParseError> {
    // Cheap structural check (we trust serde_json::from_value below for full enum check).
    // Only enforce the constraints serde would not catch.
    let score = v.get("waste_score").and_then(|s| s.as_u64())
        .ok_or_else(|| ParseError::Schema("missing waste_score".into()))?;
    if score > 10 {
        return Err(ParseError::Schema(format!("waste_score {score} > 10")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_finding() {
        let raw = r#"{"waste_score":4,"waste_category":"LegitBugfix","suggested_remediation_kind":"NoneNeeded","rationale_one_line":"ok"}"#;
        let f = parse(raw).unwrap();
        assert_eq!(f.waste_score, 4);
    }

    #[test]
    fn strips_fence() {
        let raw = "```json\n{\"waste_score\":1,\"waste_category\":\"LegitDocs\",\"suggested_remediation_kind\":\"NoneNeeded\",\"rationale_one_line\":\"x\"}\n```";
        assert!(parse(raw).is_ok());
    }

    #[test]
    fn rejects_score_above_10() {
        let raw = r#"{"waste_score":11,"waste_category":"Other","suggested_remediation_kind":"Unknown","rationale_one_line":"x"}"#;
        assert!(matches!(parse(raw), Err(ParseError::Schema(_))));
    }

    #[test]
    fn retry_message_mentions_error() {
        let e = ParseError::Schema("waste_score 11 > 10".into());
        let m = retry_message(&e);
        assert!(m.contains("11"));
    }
}
