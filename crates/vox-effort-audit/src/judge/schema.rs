//! Public finding schema (stable contract for S2–S4).

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum WasteCategory {
    MechanicalSweep,
    MissingProjectConvention,
    LinterGap,
    LowLeverageDebugging,
    ExploratoryDeadEnd,
    LegitFeatureWork,
    LegitBugfix,
    LegitDocs,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum RemediationKind {
    ScriptAutomation,
    AgentsMdRule,
    LinterRule,
    CorpusNegativeExample,
    NoneNeeded,
    Unknown,
}

/// What the judge actually outputs (the inner `finding` object on JSONL rows).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JudgeFinding {
    pub waste_score: u8,                          // 0..=10 inclusive
    pub waste_category: WasteCategory,
    pub suggested_remediation_kind: RemediationKind,
    pub rationale_one_line: String,
    #[serde(default)]
    pub evidence_pointers: Vec<String>,
}

/// JSON Schema string for use with `LlmConfig.response_format`.
pub fn judge_finding_json_schema() -> serde_json::Value {
    serde_json::json!({
      "type": "object",
      "properties": {
        "waste_score": { "type": "integer", "minimum": 0, "maximum": 10 },
        "waste_category": { "type": "string", "enum": [
          "MechanicalSweep","MissingProjectConvention","LinterGap","LowLeverageDebugging",
          "ExploratoryDeadEnd","LegitFeatureWork","LegitBugfix","LegitDocs","Other"
        ]},
        "suggested_remediation_kind": { "type": "string", "enum": [
          "ScriptAutomation","AgentsMdRule","LinterRule","CorpusNegativeExample","NoneNeeded","Unknown"
        ]},
        "rationale_one_line": { "type": "string", "maxLength": 240 },
        "evidence_pointers": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["waste_score","waste_category","suggested_remediation_kind","rationale_one_line"],
      "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_judge_finding() {
        let f = JudgeFinding {
            waste_score: 8, waste_category: WasteCategory::MechanicalSweep,
            suggested_remediation_kind: RemediationKind::ScriptAutomation,
            rationale_one_line: "same edit x50".into(),
            evidence_pointers: vec!["crates/x:42".into()],
        };
        let j = serde_json::to_string(&f).unwrap();
        assert!(j.contains("\"MechanicalSweep\""));
        let back: JudgeFinding = serde_json::from_str(&j).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn schema_lists_all_enum_variants() {
        let s = judge_finding_json_schema();
        let cats = s["properties"]["waste_category"]["enum"].as_array().unwrap();
        assert_eq!(cats.len(), 9);
        let rems = s["properties"]["suggested_remediation_kind"]["enum"].as_array().unwrap();
        assert_eq!(rems.len(), 6);
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(SCHEMA_VERSION, "1.0");
    }
}
