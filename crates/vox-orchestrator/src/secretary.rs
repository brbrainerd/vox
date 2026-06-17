//! Secretary classifier: lightweight heuristic for detecting actionable intent
//! in chat messages.

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Outcome of classifying a single chat message.
#[derive(Debug, PartialEq, Eq)]
pub struct ClassifyResult {
    /// The extracted task intent (first 200 chars, normalised).
    pub intent: String,
    /// Heuristic confidence 0–100 (not a probability; just for logging).
    pub confidence_pct: u8,
}

/// Classify a single chat turn.
///
/// Returns `Some(ClassifyResult)` if the message contains actionable intent,
/// `None` if it should be ignored.
pub fn classify(role: &str, content: &str) -> Option<ClassifyResult> {
    if role != "user" {
        return None;
    }

    let words: Vec<&str> = content.split_whitespace().collect();
    if words.len() < 10 {
        return None;
    }

    let lower = content.to_lowercase();
    let matched_verb = ACTION_VERBS.iter().find(|&&v| lower.contains(v))?;

    // Trim to 200 chars, strip leading/trailing whitespace.
    let intent = content.chars().take(200).collect::<String>().trim().to_string();

    // Confidence is higher when the verb appears early in the message.
    let verb_pos = lower.find(matched_verb).unwrap_or(usize::MAX);
    let confidence_pct = if verb_pos < 20 { 85 } else { 60 };

    Some(ClassifyResult {
        intent,
        confidence_pct,
    })
}

/// Action verbs that signal the user wants something done.
const ACTION_VERBS: &[&str] = &[
    "fix", "add", "update", "create", "remove", "delete", "refactor",
    "write", "implement", "build", "migrate", "extract", "rename",
    "move", "replace", "rewrite", "upgrade", "configure", "setup",
    "install", "deploy", "test", "debug", "investigate",
];


// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_assistant_messages() {
        let result = classify(
            "assistant",
            "fix the bug in the authentication module please it is broken",
        );
        assert!(result.is_none());
    }

    #[test]
    fn ignores_short_messages() {
        // Under 10 words — should be ignored
        let result = classify("user", "fix the bug please");
        assert!(result.is_none());
    }

    #[test]
    fn detects_fix_verb() {
        let result = classify(
            "user",
            "fix the authentication bug in the login module where users cannot sign in",
        );
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.intent.contains("fix"));
        assert!(r.confidence_pct > 0);
    }

    #[test]
    fn detects_implement_verb() {
        let result = classify(
            "user",
            "implement the new retry logic for the HTTP client that currently fails on timeouts",
        );
        assert!(result.is_some());
    }

    #[test]
    fn ignores_no_action_verb() {
        let result = classify(
            "user",
            "the authentication module seems to be having some issues with the login flow currently",
        );
        assert!(result.is_none(), "no action verb should produce None");
    }

    #[test]
    fn intent_is_capped_at_200_chars() {
        let long = format!("fix the thing that is currently broken and causing the server to crash repeatedly {}", "x".repeat(300));
        let result = classify("user", &long).unwrap();
        assert!(result.intent.len() <= 200);
    }

    #[test]
    fn early_verb_gets_higher_confidence() {
        let early = classify(
            "user",
            "fix the memory leak in the websocket handler it is causing the server to crash",
        )
        .unwrap();
        let late = classify(
            "user",
            "the websocket handler seems to be leaking memory please fix it before the release",
        )
        .unwrap();
        assert!(early.confidence_pct > late.confidence_pct);
    }
}
