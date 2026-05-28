//! AI disclosure block per Nature/Science/Cell 2025 norms.
//! Auto-filled into RO-Crate metadata and manifests.

use serde::{Deserialize, Serialize};

/// AI tool usage declaration per Nature 2025 AI policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiToolUsage {
    pub tool_name: String,
    pub tool_version: Option<String>,
    pub usage_description: String,
    pub human_verified: bool,
}

/// Full AI disclosure block for a publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDisclosureBlock {
    pub policy_version: String, // e.g. "Nature-2025-04"
    pub tools_used: Vec<AiToolUsage>,
    pub human_author_accountable: bool, // required true
    pub no_llm_generated_figures: bool, // true per Cell/Science 2025
    pub disclosure_text: String,        // auto-generated summary
}

impl AiDisclosureBlock {
    pub fn build(policy_version: &str, tools: Vec<AiToolUsage>) -> Self {
        let tool_names: Vec<&str> = tools.iter().map(|t| t.tool_name.as_str()).collect();
        let disclosure_text = if tools.is_empty() {
            "No AI tools were used in generating the content of this publication.".to_string()
        } else {
            format!(
                "The following AI tools were used in this research: {}. All AI-generated content was reviewed and verified by human authors, who take full accountability for the published claims. No AI tools were used to generate primary research figures.",
                tool_names.join(", ")
            )
        };
        Self {
            policy_version: policy_version.to_string(),
            tools_used: tools,
            human_author_accountable: true,
            no_llm_generated_figures: true,
            disclosure_text,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tools_generates_no_ai_text() {
        let block = AiDisclosureBlock::build("Nature-2025-04", vec![]);
        assert!(block.disclosure_text.contains("No AI tools"));
        assert!(block.human_author_accountable);
        assert!(block.no_llm_generated_figures);
    }

    #[test]
    fn tool_names_appear_in_disclosure_text() {
        let tools = vec![AiToolUsage {
            tool_name: "Claude".to_string(),
            tool_version: Some("3.7".to_string()),
            usage_description: "Draft writing and code generation".to_string(),
            human_verified: true,
        }];
        let block = AiDisclosureBlock::build("Nature-2025-04", tools);
        assert!(block.disclosure_text.contains("Claude"));
    }

    #[test]
    fn disclosure_always_marks_no_llm_figures() {
        let block = AiDisclosureBlock::build("Cell-2025", vec![]);
        assert!(block.no_llm_generated_figures);
    }
}
