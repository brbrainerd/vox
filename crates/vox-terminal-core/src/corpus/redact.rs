//! Transcript redaction — strips PII and secrets before corpus export.
//!
//! Redaction is conservative: unknown patterns are left as-is. Only patterns
//! with high precision (low false-positive rate) are elided.

use regex::Regex;
use std::sync::OnceLock;

static RE_EMAIL: OnceLock<Regex> = OnceLock::new();
static RE_API_KEY: OnceLock<Regex> = OnceLock::new();
static RE_IPV4: OnceLock<Regex> = OnceLock::new();
static RE_HOME: OnceLock<Regex> = OnceLock::new();

fn re_email() -> &'static Regex {
    RE_EMAIL
        .get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap())
}

fn re_api_key() -> &'static Regex {
    RE_API_KEY.get_or_init(|| {
        // Matches bearer tokens, api_key=, token= followed by >=32 char alphanumeric string.
        Regex::new(r"(?i)(?:bearer\s+|api[_-]?key[=:]\s*|token[=:]\s*)[A-Za-z0-9+/\-_]{32,}")
            .unwrap()
    })
}

fn re_ipv4() -> &'static Regex {
    RE_IPV4.get_or_init(|| {
        // Matches 127.x, 10.x, 192.168.x, 172.16-31.x
        Regex::new(
            r"\b(?:127|10|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}(?:\.\d{1,3})?\b",
        )
        .unwrap()
    })
}

fn re_home() -> &'static Regex {
    RE_HOME.get_or_init(|| {
        // Matches /home/<user>/... or C:\Users\<user>\...
        Regex::new(r"(?:/home/[^/\s]+|[A-Za-z]:\\Users\\[^\\]+)").unwrap()
    })
}

// Re-implement apply using static refs to avoid lifetime tangles.
pub fn redact_owned(text: &str) -> String {
    let s = re_email().replace_all(text, "[REDACTED_EMAIL]");
    let s = re_api_key().replace_all(&s, "[REDACTED_KEY]");
    let s = re_ipv4().replace_all(&s, "[REDACTED_IP]");
    let s = re_home().replace_all(&s, "~[REDACTED_PATH]");
    s.into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_redacted() {
        let r = redact_owned("contact me at alice@example.com please");
        assert!(!r.contains("alice@example.com"));
        assert!(r.contains("[REDACTED_EMAIL]"));
    }

    #[test]
    fn api_key_redacted() {
        let r = redact_owned("Authorization: Bearer sk-abcdefghijklmnopqrstuvwxyz123456");
        assert!(r.contains("[REDACTED_KEY]"), "got: {r}");
    }

    #[test]
    fn loopback_ip_redacted() {
        let r = redact_owned("server at 127.0.0.1:8080");
        assert!(r.contains("[REDACTED_IP]"), "got: {r}");
    }

    #[test]
    fn clean_text_unchanged() {
        let text = "hello world, run: cargo test --lib";
        assert_eq!(redact_owned(text), text);
    }
}
