//! Transcript redaction. The implementation moved to the `vox-redact` crate so
//! non-terminal crates can reuse it; re-exported here to keep the existing
//! `super::redact::redact_owned` / `corpus::redact_owned` paths stable.

pub use vox_redact::redact_owned;

#[cfg(test)]
mod tests {
    // These now exercise `vox_redact::redact_owned` through the re-export — a free
    // cross-crate behavior check that the moved redactor still elides what it must.
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
