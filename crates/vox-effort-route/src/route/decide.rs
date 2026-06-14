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

#[cfg(test)]
mod semcov_wave7_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    // Catches: parse() silently returning Ok with default values on missing required fields
    #[test]
    fn parse_rejects_missing_required_field() {
        // Missing 'drafted_body' — must be an error, not a default-populated Ok
        let raw = r#"{"artifact_form":"CiGate","confidence":0.8,"synthesized_fix_summary":"s","form_rationale":"r"}"#;
        let result = parse(raw);
        assert!(result.is_err(), "missing required field 'drafted_body' must be an error");
    }

    // Catches: parse() accepting completely invalid JSON without error
    #[test]
    fn parse_rejects_non_json() {
        assert!(parse("literally not json").is_err(), "non-JSON input must produce Err");
    }

    // Catches: decide_json_schema confidence bounds being inverted (max < min)
    #[test]
    fn schema_confidence_bounds_are_valid() {
        let s = decide_json_schema(false);
        let min = s["properties"]["confidence"]["minimum"].as_f64().unwrap_or(f64::MAX);
        let max = s["properties"]["confidence"]["maximum"].as_f64().unwrap_or(f64::MIN);
        assert!(min < max, "confidence minimum ({min}) must be less than maximum ({max})");
        assert_eq!(min, 0.0, "confidence minimum must be 0");
        assert_eq!(max, 1.0, "confidence maximum must be 1");
    }

    // Catches: VoxScript remaining in schema when vox_capable=false after a refactor
    #[test]
    fn schema_vox_script_absent_when_not_capable() {
        let s = decide_json_schema(false);
        let forms = s["properties"]["artifact_form"]["enum"]
            .as_array()
            .unwrap();
        assert!(
            !forms.iter().any(|v| v.as_str() == Some("VoxScript")),
            "VoxScript must not appear in schema when vox_capable=false"
        );
    }

    // Catches: 'None' form disappearing from schema (would cause parse failures for budget-skipped rows)
    #[test]
    fn schema_none_form_always_present() {
        for capable in [false, true] {
            let s = decide_json_schema(capable);
            let forms = s["properties"]["artifact_form"]["enum"].as_array().unwrap();
            assert!(
                forms.iter().any(|v| v.as_str() == Some("None")),
                "'None' artifact_form must be in schema (vox_capable={capable})"
            );
        }
    }
}
