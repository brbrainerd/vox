//! Conservative PII/secret redaction. Moved from `vox-terminal-core::corpus::redact`
//! so non-terminal crates (e.g. agent operation capture) can reuse it without a
//! backwards dependency edge.
//!
//! Redaction is conservative: unknown patterns are left as-is. Only patterns with
//! high precision (low false-positive rate) are elided.

use regex::Regex;
use std::sync::OnceLock;

static RE_EMAIL: OnceLock<Regex> = OnceLock::new();
static RE_API_KEY: OnceLock<Regex> = OnceLock::new();
static RE_TOKEN: OnceLock<Regex> = OnceLock::new();
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

fn re_token() -> &'static Regex {
    // High-precision, low-false-positive vendor token shapes that appear as bare
    // VALUES (no `key=` prefix) — catches secrets stored under non-secret JSON keys.
    RE_TOKEN.get_or_init(|| {
        Regex::new(
            r"(?x)
            \b(?:
              gh[posru]_[A-Za-z0-9]{20,}      # GitHub PAT / OAuth / server / refresh
            | github_pat_[A-Za-z0-9_]{20,}     # GitHub fine-grained PAT
            | sk-[A-Za-z0-9-]{20,}             # OpenAI-style secret key
            | xox[baprs]-[A-Za-z0-9-]{10,}     # Slack tokens
            | AKIA[0-9A-Z]{16}                 # AWS access key id
            | eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}  # JWT
            )\b",
        )
        .unwrap()
    })
}

/// Redact PII/secret patterns in free text. Conservative: unknown patterns pass through.
pub fn redact_owned(text: &str) -> String {
    let s = re_email().replace_all(text, "[REDACTED_EMAIL]");
    let s = re_api_key().replace_all(&s, "[REDACTED_KEY]");
    let s = re_token().replace_all(&s, "[REDACTED_KEY]");
    let s = re_ipv4().replace_all(&s, "[REDACTED_IP]");
    let s = re_home().replace_all(&s, "~[REDACTED_PATH]");
    s.into_owned()
}

// ponytail: denylist is intentionally over-broad (substring match). Over-redaction
// is the safe failure mode for secrets — e.g. "author" matches "auth". The mining
// sub-project tolerates a few redacted fields; a leaked token is unacceptable.
fn key_is_secret(key: &str) -> bool {
    const DENY: &[&str] = &[
        "token",
        "key",
        "secret",
        "password",
        "passwd",
        "authorization",
        "auth",
        "credential",
        "apikey",
        "bearer",
        "cookie",
        "session",
    ];
    let k = key.to_ascii_lowercase();
    DENY.iter().any(|d| k.contains(d))
}

/// Recursively redact a JSON value: values under secret-ish keys become
/// `"[REDACTED]"`; all other string scalars are run through [`redact_owned`].
pub fn redact_args(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if key_is_secret(k) {
                    out.insert(k.clone(), Value::String("[REDACTED]".into()));
                } else {
                    out.insert(k.clone(), redact_args(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_args).collect()),
        Value::String(s) => Value::String(redact_owned(s)),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- redact_owned (moved verbatim from vox-terminal-core) ---

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

    // --- redact_args (new) ---

    #[test]
    fn redacts_value_under_secret_key() {
        let out = redact_args(&json!({ "api_key": "abc", "Authorization": "Bearer z" }));
        assert_eq!(out["api_key"], json!("[REDACTED]"));
        assert_eq!(out["Authorization"], json!("[REDACTED]"));
    }

    #[test]
    fn redacts_secret_pattern_in_nonsecret_field() {
        let out =
            redact_args(&json!({ "note": "use api_key= ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789" }));
        assert!(
            out["note"].as_str().unwrap().contains("[REDACTED_KEY]"),
            "got {out}"
        );
    }

    #[test]
    fn bare_vendor_tokens_redacted_without_key_prefix() {
        // Secret as a bare value under a non-secret key — caught by re_token.
        let out = redact_args(&json!({ "data": "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789" }));
        assert!(
            out["data"].as_str().unwrap().contains("[REDACTED_KEY]"),
            "got {out}"
        );
        assert!(redact_owned("token is AKIAIOSFODNN7EXAMPLE here").contains("[REDACTED_KEY]"));
        // A plain non-secret value is untouched.
        assert_eq!(
            redact_owned("just some normal text"),
            "just some normal text"
        );
    }

    #[test]
    fn recurses_objects_and_arrays_preserves_plain_values() {
        let out = redact_args(&json!({
            "outer": { "password": "p" },
            "list": ["alice@example.com", "ok"],
            "count": 3
        }));
        assert_eq!(out["outer"]["password"], json!("[REDACTED]"));
        assert!(
            out["list"][0]
                .as_str()
                .unwrap()
                .contains("[REDACTED_EMAIL]")
        );
        assert_eq!(out["list"][1], json!("ok"));
        assert_eq!(out["count"], json!(3));
    }
}
