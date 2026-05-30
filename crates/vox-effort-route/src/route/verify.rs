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
    let cleaned = raw
        .trim()
        .strip_prefix("```json")
        .or_else(|| raw.trim().strip_prefix("```"))
        .unwrap_or(raw.trim());
    let cleaned = cleaned.strip_suffix("```").unwrap_or(cleaned).trim();
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
        let r = parse("```\n{\"refuted\":true,\"refutation_note\":\"slips through\"}\n```").unwrap();
        assert!(r.refuted);
        assert_eq!(r.refutation_note, "slips through");
    }
}
