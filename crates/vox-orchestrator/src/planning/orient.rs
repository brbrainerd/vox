//! Orient Phase processing for Socrates task context.

use crate::socrates::{OrientReport, SocratesTaskContext};
use crate::types::TaskCategory;

pub struct OrientPhase;

impl OrientPhase {
    /// Classifies the task category based on its description text.
    pub fn classify_task_category(description: &str) -> TaskCategory {
        let l = description.to_ascii_lowercase();
        if l.contains("research") || l.contains("investigate") {
            TaskCategory::Research
        } else if l.contains("test")
            || l.contains("assert")
            || l.contains("verify")
            || l.contains("spec")
        {
            TaskCategory::Testing
        } else if l.contains("refactor")
            || l.contains("clean")
            || l.contains("restructure")
            || l.contains("document")
            || l.contains("comment")
            || l.contains("readme")
            || l.contains("docstring")
        {
            TaskCategory::General
        } else if l.contains("analyze") || l.contains("audit") || l.contains("review") {
            TaskCategory::Review
        } else if l.contains("implement")
            || l.contains("add")
            || l.contains("create")
            || l.contains("build")
            || l.contains("fix")
        {
            TaskCategory::CodeGen // Implementation maps to CodeGen
        } else if l.contains("type") || l.contains("check") || l.contains("lint") {
            TaskCategory::TypeChecking
        } else if l.contains("parse") || l.contains("ast") {
            TaskCategory::Parsing
        } else {
            TaskCategory::General
        }
    }

    /// Evaluates context to produce an OrientReport holding evidence gap, risk band, and complexity.
    pub fn orient_phase(
        description: &str,
        ctx: &SocratesTaskContext,
        policy: &vox_orchestrator_types::socrates_policy::ConfidencePolicy,
    ) -> OrientReport {
        let evidence_gap = if ctx.required_citations == 0 {
            0.0
        } else {
            let coverage =
                (f64::from(ctx.evidence_count) / f64::from(ctx.required_citations)).clamp(0.0, 1.0);
            1.0 - coverage
        };

        let gate_outcome = crate::socrates::evaluate_socrates_gate(ctx, policy, description);

        let mut complexity: f64 = 5.0;
        if ctx.retrieval_used_lexical_fallback {
            complexity += 1.5;
        }
        if ctx.contradiction_hints > 0 {
            complexity += 2.0;
        }
        if ctx.source_diversity > 3 {
            complexity += 1.0;
        }
        if ctx.fatigue_active {
            complexity += 3.0; // Higher planning complexity when human is fatigued
        }

        OrientReport {
            evidence_gap,
            risk_band: gate_outcome.band,
            planning_complexity: complexity.clamp(1.0, 10.0),
            category: Some(Self::classify_task_category(description)),
        }
    }

    /// Returns a message string if the evidence gap is unacceptably high.
    pub fn request_missing_evidence(gap: f64) -> Option<String> {
        if gap > 0.4 {
            Some(format!(
                "Orient phase detected a {:.1}% evidence gap. Please gather more grounded evidence or run retrieval tools before proceeding.",
                gap * 100.0
            ))
        } else {
            None
        }
    }
}

/// Evaluates plan adequacy using Socrates LLM-as-judge heuristics.
pub struct SocratesPlanJudge;

impl SocratesPlanJudge {
    /// Constructs a prompt for LLM-as-judge to evaluate plan adequacy.
    pub fn generate_evaluation_prompt(goal: &str, plan_text: &str) -> String {
        format!(
            "Evaluate the following plan for adequacy based on the goal: '{goal}'.\n\
            \nPlan:\n{plan_text}\n\n\
            Please score each of these 5 points from 0 to 10:\n\
            1. Coverage: Does the plan cover all requirements?\n\
            2. Dep: Are preconditions and dependencies correctly ordered?\n\
            3. Destructive: Are destructive operations properly contained and safeguarded?\n\
            4. Concreteness: Are action verbs clear, unvague, and distinct?\n\
            5. Verification: Is there a test, assertion, or validation step present?\n\
            Respond with JSON format: {{\"coverage\": 10, \"dep\": 10, \"destructive\": 10, \"concreteness\": 10, \"verification\": 10}}"
        )
    }

    /// Parses the JSON output from the LLM-as-judge iteration.
    pub fn parse_evaluation_scores(llm_response: &str) -> Option<(u8, u8, u8, u8, u8)> {
        #[derive(serde::Deserialize)]
        struct Scores {
            coverage: u8,
            dep: u8,
            destructive: u8,
            concreteness: u8,
            verification: u8,
        }

        // Find json payload
        let start = llm_response.find('{')?;
        let end = llm_response.rfind('}')?;
        let json_str = &llm_response[start..=end];
        let s: Scores = serde_json::from_str(json_str).ok()?;

        Some((
            s.coverage.clamp(0, 10),
            s.dep.clamp(0, 10),
            s.destructive.clamp(0, 10),
            s.concreteness.clamp(0, 10),
            s.verification.clamp(0, 10),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orient_phase_classification() {
        assert_eq!(
            OrientPhase::classify_task_category("document the API"),
            TaskCategory::General
        );
        assert_eq!(
            OrientPhase::classify_task_category("run unit tests"),
            TaskCategory::Testing
        );
        assert_eq!(
            OrientPhase::classify_task_category("implement standard backend"),
            TaskCategory::CodeGen
        );
    }

    #[test]
    fn test_request_missing_evidence() {
        assert!(OrientPhase::request_missing_evidence(0.3).is_none());
        assert!(OrientPhase::request_missing_evidence(0.5).is_some());
    }

    #[test]
    fn test_parse_evaluation_scores() {
        let text = r#"
        Here is the evaluation:
        {
            "coverage": 8,
            "dep": 10,
            "destructive": 10,
            "concreteness": 5,
            "verification": 0
        }
        "#;
        let p = SocratesPlanJudge::parse_evaluation_scores(text).unwrap();
        assert_eq!(p, (8, 10, 10, 5, 0));
    }
}
