//! Decide-pass response parsing.

use crate::route::ArtifactForm;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DecideResponse {
    pub artifact_form: ArtifactForm,
    pub confidence: f32,
    pub synthesized_fix_summary: String,
    pub drafted_body: String,
    pub form_rationale: String,
}

pub fn decide_json_schema(vox_capable: bool) -> serde_json::Value {
    let mut forms = vec![
        "AgentsMdRule",
        "CodeAuditDetector",
        "ArchRule",
        "CiGate",
        "CorpusNegativeExample",
        "None",
    ];
    if vox_capable {
        forms.push("VoxScript");
    }
    serde_json::json!({
      "type":"object",
      "properties":{
        "artifact_form":{"type":"string","enum":forms},
        "confidence":{"type":"number","minimum":0,"maximum":1},
        "synthesized_fix_summary":{"type":"string"},
        "drafted_body":{"type":"string"},
        "form_rationale":{"type":"string"}
      },
      "required":["artifact_form","confidence","synthesized_fix_summary","drafted_body","form_rationale"],
      "additionalProperties":false
    })
}

pub fn parse(raw: &str) -> Result<DecideResponse, String> {
    let cleaned = crate::route::strip_json_fence(raw);
    serde_json::from_str(cleaned).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_decide_response() {
        let raw = r#"{"artifact_form":"CiGate","confidence":0.8,"synthesized_fix_summary":"s","drafted_body":"b","form_rationale":"r"}"#;
        let d = parse(raw).unwrap();
        assert_eq!(d.artifact_form, ArtifactForm::CiGate);
    }
    #[test]
    fn parses_decide_response_with_json_fence() {
        let raw = "```json\n{\"artifact_form\":\"AgentsMdRule\",\"confidence\":0.5,\"synthesized_fix_summary\":\"s\",\"drafted_body\":\"b\",\"form_rationale\":\"r\"}\n```";
        let d = parse(raw).unwrap();
        assert_eq!(d.artifact_form, ArtifactForm::AgentsMdRule);
    }
    #[test]
    fn schema_excludes_vox_when_incapable() {
        let s = decide_json_schema(false);
        let arr = s["properties"]["artifact_form"]["enum"].as_array().unwrap();
        assert!(!arr.iter().any(|v| v == "VoxScript"));
    }
    #[test]
    fn schema_includes_vox_when_capable() {
        let s = decide_json_schema(true);
        let arr = s["properties"]["artifact_form"]["enum"].as_array().unwrap();
        assert!(arr.iter().any(|v| v == "VoxScript"));
    }
}
