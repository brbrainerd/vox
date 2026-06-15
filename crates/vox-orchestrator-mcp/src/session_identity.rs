/// Normalize optional session ids by trimming whitespace and removing empty values.
#[must_use]
pub(crate) fn normalize_optional_session_id(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string)
}

/// Normalize chat session id using stable `"default"` fallback for backward compatibility.
#[must_use]
pub(crate) fn normalize_chat_session_id(raw: Option<&str>) -> (String, bool) {
    match normalize_optional_session_id(raw) {
        Some(session_id) => (session_id, false),
        None => ("default".to_string(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_chat_session_id, normalize_optional_session_id};

    #[test]
    fn normalize_optional_trims_whitespace() {
        assert_eq!(
            normalize_optional_session_id(Some("  abc-123  ")).as_deref(),
            Some("abc-123")
        );
    }

    #[test]
    fn normalize_optional_drops_empty() {
        assert!(normalize_optional_session_id(Some("   ")).is_none());
        assert!(normalize_optional_session_id(None).is_none());
    }

    #[test]
    fn chat_normalization_preserves_default() {
        let (session_id, implicit) = normalize_chat_session_id(None);
        assert_eq!(session_id, "default");
        assert!(implicit);
    }
}
