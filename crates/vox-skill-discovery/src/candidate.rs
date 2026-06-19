//! The unified advisory output of the discovery engine.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CandidateKind {
    /// A recurring code block that could be extracted into a reusable skill/snippet.
    RepeatedCode,
    /// Two or more installed skills/tools that overlap heavily.
    DuplicatesInstalled,
    /// A skill declares an MCP tool that does not exist in the registry.
    SsotDrift,
}

/// Advisory draft frontmatter the user MAY accept (never auto-applied).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftFrontmatter {
    pub name: String,
    pub description: String,
    pub category: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub kind: CandidateKind,
    /// Provenance refs: "path:line", skill ids, or "skill_id->tool".
    pub members: Vec<String>,
    pub score: f32,
    pub suggested_action: String,
    pub draft_frontmatter: Option<DraftFrontmatter>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_serializes_to_json() {
        let c = Candidate {
            kind: CandidateKind::RepeatedCode,
            members: vec!["a.vox:1".into(), "b.vox:9".into()],
            score: 0.95,
            suggested_action: "Extract into a reusable Vox skill".into(),
            draft_frontmatter: None,
        };
        let j = serde_json::to_string(&c).unwrap();
        assert!(j.contains("RepeatedCode"));
        assert!(j.contains("a.vox:1"));
    }
}
