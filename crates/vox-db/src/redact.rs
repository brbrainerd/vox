//! Secret pattern redaction.

use regex::Regex;
use std::sync::OnceLock;

/// Redact known secret pattern shapes (e.g. API keys, tokens, credentials) from text.
/// Returns the redacted text and a boolean flag indicating if any secrets were found.
pub fn redact(text: &str) -> (String, bool) {
    let mut result = text.to_string();
    let mut redacted = false;

    // 1. OpenAI / Anthropic / typical API key patterns (e.g. sk-..., gsk-..., pk-..., ak-..., sk-proj-...)
    static API_KEY_RE: OnceLock<Regex> = OnceLock::new();
    let api_key_re = API_KEY_RE.get_or_init(|| {
        Regex::new(r"\b(?:sk|gsk|pk|ak|uk|sk-proj|sk_live|sk_test)-[a-zA-Z0-9_-]{12,}\b").unwrap()
    });
    if api_key_re.is_match(&result) {
        result = api_key_re
            .replace_all(&result, "[REDACTED_API_KEY]")
            .to_string();
        redacted = true;
    }

    // 2. GitHub Personal Access Tokens (ghp_..., gho_..., ghu_..., ghs_..., ghr_...)
    static GITHUB_PAT_RE: OnceLock<Regex> = OnceLock::new();
    let github_pat_re = GITHUB_PAT_RE
        .get_or_init(|| Regex::new(r"\b(?:ghp|gho|ghu|ghs|ghr)_[a-zA-Z0-9]{30,}\b").unwrap());
    if github_pat_re.is_match(&result) {
        result = github_pat_re
            .replace_all(&result, "[REDACTED_GITHUB_TOKEN]")
            .to_string();
        redacted = true;
    }

    // 3. AWS Access Key ID (AKIA...)
    static AWS_KEY_RE: OnceLock<Regex> = OnceLock::new();
    let aws_key_re = AWS_KEY_RE.get_or_init(|| Regex::new(r"\bAKIA[0-9A-Z]{16}\b").unwrap());
    if aws_key_re.is_match(&result) {
        result = aws_key_re
            .replace_all(&result, "[REDACTED_AWS_KEY]")
            .to_string();
        redacted = true;
    }

    (result, redacted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_masks_known_secrets_keeps_clean_text() {
        assert_eq!(redact("just text").0, "just text");
        assert!(!redact("just text").1);

        let (masked, flagged) = redact("token sk-ABC123DEF456GHI789");
        assert!(flagged);
        assert!(!masked.contains("sk-ABC123DEF456GHI789"));
        assert!(masked.contains("[REDACTED_API_KEY]"));

        let (masked_gh, flagged_gh) =
            redact("github token ghp_123456789012345678901234567890123456");
        assert!(flagged_gh);
        assert!(!masked_gh.contains("ghp_123456789012345678901234567890123456"));
        assert!(masked_gh.contains("[REDACTED_GITHUB_TOKEN]"));
    }
}
