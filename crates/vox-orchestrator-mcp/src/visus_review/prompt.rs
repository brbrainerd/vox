//! System prompt for adversarial GUI screenshot review.

pub const RUBRIC: &str = r#"
Review this desktop-app surface SCREENSHOT adversarially against these principles. Hunt for real defects; do not flatter.
1 Visual hierarchy: exactly one primary action; scale/weight/contrast rank elements (#65-73).
2 Tokens/consistency: consistent color meaning, spacing rhythm, iconography; no ad-hoc visual noise (#20-24,#247-255).
3 Typography & spacing: readable measure/line-height; aligned to a spacing scale (#74-98).
4 Loading/empty/error: deliberate states, not silent blanks; errors actionable (#1.1,#47-52,#163-168).
5 Accessibility (visual): text contrast >=4.5:1, UI/focus >=3:1; targets >=24px; icon-only controls look labeled (#178-211).
6 Affordance & feedback: interactive elements look interactive; current location obvious (#132-162).
7 Minimalism: progressive disclosure; remove clutter (#42-46).
8 Error prevention: destructive actions visually distinct/guarded (#25-31).
"#;

pub fn system_prompt() -> String {
    format!(
        "You are a senior product-design reviewer performing an ADVERSARIAL critique of a desktop \
GUI surface screenshot. Be specific and skeptical: every finding must cite a visible region and a \
principle number.\n\nRUBRIC:\n{RUBRIC}\n\nOUTPUT CONTRACT: Respond with ONLY a single JSON object, no \
prose, no markdown fence:\n{{\n  \"score\": <integer 0-100, lower = more/worse defects>,\n  \"verdict\": \
\"pass\" | \"pass_with_notes\" | \"fail\",\n  \"findings\": [ {{ \"principle\": \"#NN short-name\", \
\"severity\": \"low\"|\"medium\"|\"high\", \"region\": \"<where on screen>\", \"critique\": \"<what is wrong and why>\" }} ]\n}}\n\
If the surface is clean, return an empty findings array, verdict \"pass\", score >=90."
    )
}

pub fn user_prompt(view_key: &str) -> String {
    format!(
        "Surface: '{view_key}'. Critique the attached screenshot per the rubric and output the JSON verdict."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_states_json_only_contract() {
        let p = system_prompt();
        assert!(p.contains("ONLY a single JSON object"));
        assert!(p.contains("\"findings\""));
        assert!(RUBRIC.contains("Visual hierarchy"));
    }
}
