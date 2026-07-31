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

    // Word-boundary match (not substring): tokenize into lowercased
    // alphanumeric words with their byte offset, then require an *exact*
    // token match against ACTION_VERBS. This avoids matching "add" inside
    // "address", "fix" inside "prefix"/"fixed", "build" inside "building",
    // etc. (finding F2).
    let tokens = word_tokens_with_pos(content);
    let verb_pos = tokens
        .iter()
        .find(|(w, _)| ACTION_VERBS.contains(&w.as_str()))
        .map(|(_, pos)| *pos)?;

    // Trim to 200 chars, strip leading/trailing whitespace.
    let intent = content
        .chars()
        .take(200)
        .collect::<String>()
        .trim()
        .to_string();

    // Confidence is higher when the verb appears early in the message.
    let confidence_pct = if verb_pos < 20 { 85 } else { 60 };

    Some(ClassifyResult {
        intent,
        confidence_pct,
    })
}

/// Tokenize `content` into lowercased alphanumeric words, each paired with
/// its starting byte offset in the original string. Punctuation, whitespace,
/// and other separators split words; this is what lets `classify` compare
/// whole words instead of doing a raw substring search.
fn word_tokens_with_pos(content: &str) -> Vec<(String, usize)> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut start = 0usize;
    for (i, c) in content.char_indices() {
        // Apostrophes are kept inside a word (e.g. "don't") but do not start
        // one, so punctuation-only runs never produce an empty "word".
        if c.is_alphanumeric() || (c == '\'' && !current.is_empty()) {
            if current.is_empty() {
                start = i;
            }
            for lc in c.to_lowercase() {
                current.push(lc);
            }
        } else {
            if !current.is_empty() {
                tokens.push((std::mem::take(&mut current), start));
            }
        }
    }
    if !current.is_empty() {
        tokens.push((current, start));
    }
    tokens
}

/// Action verbs that signal the user wants something done.
const ACTION_VERBS: &[&str] = &[
    "fix",
    "add",
    "update",
    "create",
    "remove",
    "delete",
    "refactor",
    "write",
    "implement",
    "build",
    "migrate",
    "extract",
    "rename",
    "move",
    "replace",
    "rewrite",
    "upgrade",
    "configure",
    "setup",
    "install",
    "deploy",
    "test",
    "debug",
    "investigate",
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
        let long = format!(
            "fix the thing that is currently broken and causing the server to crash repeatedly {}",
            "x".repeat(300)
        );
        let result = classify("user", &long).unwrap();
        assert!(result.intent.len() <= 200);
    }

    // --- F2 regression: word-boundary matching, not substring `contains()` ---

    #[test]
    fn does_not_substring_match_add_inside_address_or_fix_inside_prefix() {
        // "address" contains "add"; "prefix" contains "fix"; "building"
        // contains "build". None of these are the standalone verbs.
        let msg = "this address needs some prefix handling before we can \
                   start building anything here";
        assert!(
            classify("user", msg).is_none(),
            "substring matches inside unrelated words must not classify as actionable"
        );
    }

    #[test]
    fn does_not_substring_match_fix_inside_past_tense_fixed() {
        // Directly from the finding: "I already fixed it" — "fixed" contains
        // "fix" as a substring but is not the standalone verb "fix".
        let msg = "I already fixed it and nothing else needs changing here \
                   today thanks so much";
        assert!(
            classify("user", msg).is_none(),
            "'fixed' must not substring-match the verb 'fix'"
        );
    }

    #[test]
    fn word_boundary_still_detects_genuine_actionable_message() {
        // A real, actionable message using a whole-word verb must still
        // classify as Some — the fix must not become over-strict.
        let msg = "please add a retry loop to this function it currently \
                   fails silently on timeout";
        let result = classify("user", msg);
        assert!(
            result.is_some(),
            "genuine whole-word verb match must still fire"
        );
        assert!(result.unwrap().intent.contains("add"));
    }

    #[test]
    fn word_tokens_with_pos_splits_on_punctuation_and_keeps_apostrophes() {
        let tokens = word_tokens_with_pos("Don't fix, address-the bug!");
        let words: Vec<&str> = tokens.iter().map(|(w, _)| w.as_str()).collect();
        assert_eq!(words, vec!["don't", "fix", "address", "the", "bug"]);
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
