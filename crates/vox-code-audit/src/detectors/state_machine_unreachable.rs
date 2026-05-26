use crate::diagnostics::catalog;
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use regex::Regex;

/// Detects states in `state_machine { }` blocks that have no outgoing `->` transitions.
pub struct StateMachineUnreachableDetector {
    /// Matches `state_machine` keyword opening
    state_machine_open: Regex,
    /// Matches a state declaration line: `  StateName {`
    state_decl: Regex,
    supported_langs: Vec<Language>,
}

impl Default for StateMachineUnreachableDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StateMachineUnreachableDetector {
    pub fn new() -> Self {
        Self {
            state_machine_open: Regex::new(r"\bstate_machine\b").expect("valid regex"),
            // A state declaration: leading whitespace, identifier, optional whitespace, `{`
            state_decl: Regex::new(r"^\s{2,}(\w+)\s*\{").expect("valid regex"),
            supported_langs: vec![Language::Vox],
        }
    }
}

impl DetectionRule for StateMachineUnreachableDetector {
    fn id(&self) -> &'static str {
        "state-machine/unreachable-state"
    }

    fn name(&self) -> &'static str {
        "State Machine Unreachable State Detector"
    }

    fn description(&self) -> &'static str {
        "Detects states in `state_machine { }` blocks that have no outgoing `->` \
        transition arrow — these states are terminal sinks and may indicate missing logic."
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn languages(&self) -> &[Language] {
        &self.supported_langs
    }

    fn diagnostic_id(&self) -> Option<&'static str> {
        Some(catalog::STATE_MACHINE_UNREACHABLE_STATE)
    }

    fn explain(&self) -> &'static str {
        "States in a `state_machine` block without any outgoing `->` transitions are terminal \
        sinks. Unless intentionally final (e.g. `Done`, `Error`), they may indicate missing \
        transition logic or a mistyped state name."
    }

    fn minimal_repro(&self) -> Option<&'static str> {
        Some(
            "// VIOLATION — Reviewing state has no outgoing -> transition (terminal sink)\n\
             state_machine OrderFlow {\n\
             \x20   Pending {\n\
             \x20       on submit -> Reviewing\n\
             \x20   }\n\
             \x20   Reviewing {    // no -> transition: stuck here forever!\n\
             \x20   }\n\
             }\n\
             \n\
             // FIX — add transitions from Reviewing\n\
             state_machine OrderFlow {\n\
             \x20   Pending {\n\
             \x20       on submit -> Reviewing\n\
             \x20   }\n\
             \x20   Reviewing {\n\
             \x20       on approve -> Fulfilled\n\
             \x20       on reject  -> Cancelled\n\
             \x20   }\n\
             }",
        )
    }

    fn detect(
        &self,
        file: &SourceFile,
        _rust_ctx: Option<&crate::analysis::RustFileContext>,
    ) -> Vec<Finding> {
        if file.language != Language::Vox {
            return vec![];
        }

        let mut findings = Vec::new();
        let lines = &file.lines;
        let n = lines.len();

        let mut i = 0;
        while i < n {
            let line = &lines[i];
            let trimmed = line.trim();

            // Skip comment lines
            if trimmed.starts_with("//") || trimmed.starts_with('#') {
                i += 1;
                continue;
            }

            // Look for `state_machine` keyword
            if !self.state_machine_open.is_match(line) {
                i += 1;
                continue;
            }

            let sm_line = i + 1; // 1-indexed line of state_machine keyword

            // Find the extent of this state_machine block by tracking braces
            // The `state_machine` line opens with `{` (may be on same line or next)
            let mut depth = 0i32;
            let mut block_start = i;
            let mut block_end = i;

            // Find opening brace
            for (j, line) in lines.iter().enumerate().take(n.min(i + 3)).skip(i) {
                if line.contains('{') {
                    block_start = j;
                    break;
                }
            }

            // Track brace depth to find block end
            for (j, line) in lines.iter().enumerate().skip(block_start) {
                for ch in line.chars() {
                    match ch {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                block_end = j;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                if depth == 0 && j > block_start {
                    block_end = j;
                    break;
                }
            }

            // Collect all state declarations within the block
            // States are lines matching `  StateName {` (indented 2+ spaces, identifier, `{`)
            let mut states: Vec<(String, usize)> = Vec::new(); // (name, line_number)

            for (j, block_line) in lines
                .iter()
                .enumerate()
                .skip(block_start + 1)
                .take(block_end.saturating_sub(block_start + 1))
            {
                if let Some(caps) = self.state_decl.captures(block_line)
                    && let Some(name_match) = caps.get(1)
                {
                    let state_name = name_match.as_str().to_string();
                    // Skip common keywords that look like state declarations
                    if !matches!(
                        state_name.as_str(),
                        "fn" | "let"
                            | "if"
                            | "else"
                            | "match"
                            | "for"
                            | "while"
                            | "loop"
                            | "impl"
                            | "struct"
                            | "enum"
                            | "pub"
                            | "mod"
                            | "use"
                    ) {
                        states.push((state_name, j + 1)); // 1-indexed
                    }
                }
            }

            // For each state, check if it has any outgoing `->` transition in its body
            for (state_name, state_line_num) in &states {
                let state_line_idx = state_line_num - 1; // back to 0-indexed

                // Find the body of this state by scanning forward for its braces
                let mut state_depth = 0i32;
                let mut state_body_end = state_line_idx;
                let mut found_open = false;

                for (j, line) in lines
                    .iter()
                    .enumerate()
                    .skip(state_line_idx)
                    .take(block_end.saturating_sub(state_line_idx))
                {
                    for ch in line.chars() {
                        match ch {
                            '{' => {
                                state_depth += 1;
                                found_open = true;
                            }
                            '}' => {
                                state_depth -= 1;
                                if state_depth == 0 && found_open {
                                    state_body_end = j;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if state_depth == 0 && found_open {
                        break;
                    }
                }

                // Check if any line in the state body contains a `->` transition
                let has_transition = lines[state_line_idx..=state_body_end]
                    .iter()
                    .any(|l| l.contains("->"));

                if !has_transition {
                    findings.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: self.diagnostic_id().map(str::to_string),
                        rule_name: self.name().to_string(),
                        severity: Severity::Warning,
                        file: file.path.clone(),
                        line: *state_line_num,
                        column: 0,
                        message: format!(
                            "State `{state_name}` in `state_machine` block has no outgoing `->` \
                            transitions — it is a terminal sink."
                        ),
                        suggestion: Some(format!(
                            "Add a transition arrow (`-> NextState`) inside `{state_name}`, or \
                            document it as intentionally terminal with a comment."
                        )),
                        alternatives: vec![
                            "Rename to a conventionally-terminal name like `Done` or `Error` to signal intent.".into(),
                        ],
                        rationale: Some(
                            "States without outgoing transitions are terminal sinks; unless \
                            intentional (e.g. Done/Error states), they indicate missing logic \
                            or a mistyped transition target.".into(),
                        ),
                        context: file.context_around(*state_line_num, 3),
                        confidence: Some(FindingConfidence::Medium),
                        evidence: None,
                    });
                }
            }

            let _ = sm_line;
            // Move past this state_machine block
            i = block_end + 1;
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn source(code: &str) -> SourceFile {
        SourceFile::new(PathBuf::from("test.vox"), code.to_string())
    }

    #[test]
    fn flags_state_with_no_transitions() {
        let d = StateMachineUnreachableDetector::new();
        let code = "state_machine TrafficLight {\n  Red {\n    on Enter { light.set_red() }\n  }\n  Green {\n    on Enter { light.set_green() }\n    on Timer -> Red\n  }\n}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            !findings.is_empty(),
            "Red state with no transitions should be flagged"
        );
        assert!(findings.iter().any(|f| f.message.contains("Red")));
    }

    #[test]
    fn ignores_states_with_transitions() {
        let d = StateMachineUnreachableDetector::new();
        let code = "state_machine Door {\n  Closed {\n    on Open -> Opened\n  }\n  Opened {\n    on Close -> Closed\n  }\n}";
        let f = source(code);
        let findings = d.detect(&f, None);
        assert!(
            findings.is_empty(),
            "states with -> transitions should not fire"
        );
    }
}
