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

    // 4. JWT tokens (three base64url segments: eyJ...)
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    let jwt_re = JWT_RE.get_or_init(|| {
        Regex::new(r"\beyJ[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\.[A-Za-z0-9_-]{6,}\b").unwrap()
    });
    if jwt_re.is_match(&result) {
        result = jwt_re.replace_all(&result, "[REDACTED_JWT]").to_string();
        redacted = true;
    }

    // 5. HTTP Authorization Bearer tokens
    static BEARER_RE: OnceLock<Regex> = OnceLock::new();
    let bearer_re =
        BEARER_RE.get_or_init(|| Regex::new(r"(?i)\bbearer\s+[A-Za-z0-9._~+/=-]{16,}").unwrap());
    if bearer_re.is_match(&result) {
        result = bearer_re
            .replace_all(&result, "Bearer [REDACTED]")
            .to_string();
        redacted = true;
    }

    // 6. PEM private keys (-----BEGIN * PRIVATE KEY-----)
    static PEM_RE: OnceLock<Regex> = OnceLock::new();
    let pem_re = PEM_RE.get_or_init(|| {
        Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")
            .unwrap()
    });
    if pem_re.is_match(&result) {
        result = pem_re
            .replace_all(&result, "[REDACTED_PEM_KEY]")
            .to_string();
        redacted = true;
    }

    // 7. Opaque long tokens (40+ alphanumeric chars, covers libsql/turso auth tokens)
    // Run last to avoid matching on already-redacted text.
    static OPAQUE_RE: OnceLock<Regex> = OnceLock::new();
    let opaque_re = OPAQUE_RE.get_or_init(|| Regex::new(r"\b[A-Za-z0-9_-]{40,}\b").unwrap());
    if opaque_re.is_match(&result) {
        result = opaque_re
            .replace_all(&result, "[REDACTED_TOKEN]")
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
        assert_eq!(redact("just text").1, false);

        let (masked, flagged) = redact("token sk-ABC123DEF456GHI789");
        assert!(flagged);
        assert!(!masked.contains("sk-ABC123DEF456GHI789"));
        assert!(masked.contains("[REDACTED"));

        let (masked_gh, flagged_gh) =
            redact("github token ghp_123456789012345678901234567890123456");
        assert!(flagged_gh);
        assert!(!masked_gh.contains("ghp_123456789012345678901234567890123456"));
        assert!(masked_gh.contains("[REDACTED"));
    }

    #[test]
    fn redact_masks_jwt_pem_bearer_and_turso() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxIn0.abc123def456";
        let (masked, flagged) = redact(jwt);
        assert!(flagged, "should flag JWT");
        assert!(
            !masked.contains("eyJhbGciOiJIUzI1NiJ9"),
            "JWT header leaked"
        );

        let bearer_input = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz123456";
        let (masked_b, flagged_b) = redact(bearer_input);
        assert!(flagged_b, "should flag bearer");
        assert!(masked_b.contains("[REDACTED]"), "bearer value not redacted");

        let pem = "-----BEGIN PRIVATE KEY-----\nMIIBVgIBADANBg\n-----END PRIVATE KEY-----";
        let (masked_p, flagged_p) = redact(pem);
        assert!(flagged_p, "should flag PEM key");
        assert!(!masked_p.contains("MIIBVgIBADAN"), "PEM body leaked");

        // long opaque token (40+ chars, covers libsql/turso)
        let turso = "eyJ0eXA9libsqlauthtokenaaaaaaaaaaaaaaaaaaaaaa";
        let (masked_t, flagged_t) = redact(turso);
        assert!(flagged_t, "should flag long opaque token");
        assert!(masked_t.contains("[REDACTED"), "opaque token not redacted");

        // normal text unchanged
        assert_eq!(redact("just normal text").0, "just normal text");
    }
}
