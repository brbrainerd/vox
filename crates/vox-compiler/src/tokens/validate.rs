//! Registry-level consistency checks for the design-token system.
//!
//! These checks run against a [`TokenRegistry`] independently of any WebIR
//! module; per-node token checks live in `web_ir::validate`.

use crate::tokens::TokenRegistry;

// ---------------------------------------------------------------------------
// Diagnostic type
// ---------------------------------------------------------------------------

/// A diagnostic emitted by [`validate_token_registry`].
#[derive(Debug, Clone)]
pub struct TokenValidationDiagnostic {
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for TokenValidationDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

// ---------------------------------------------------------------------------
// Validator
// ---------------------------------------------------------------------------

/// Run consistency checks on a [`TokenRegistry`].
///
/// # Checks
///
/// - `token.registry.empty` — the registry contains no tokens at all.
/// - `token.registry.invalid_key` — a token key contains whitespace characters.
///
/// # Future work (TODO)
///
/// - Contrast ratio checks between foreground/background color pairs require
///   a full CSS color parser; defer to a dedicated audit pass.
pub fn validate_token_registry(registry: &TokenRegistry) -> Vec<TokenValidationDiagnostic> {
    let mut out = Vec::new();

    if registry.is_empty() {
        out.push(TokenValidationDiagnostic {
            code: "token.registry.empty".to_string(),
            message: "token registry is empty — no design tokens were loaded".to_string(),
        });
        // No point checking keys if there are none.
        return out;
    }

    for key in registry.all_keys() {
        if key.chars().any(|c| c.is_whitespace()) {
            out.push(TokenValidationDiagnostic {
                code: "token.registry.invalid_key".to_string(),
                message: format!("token key {key:?} contains whitespace"),
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::TokenRegistry;

    #[test]
    fn empty_registry_warns() {
        let reg = TokenRegistry::load_from_str("{}").unwrap();
        let diags = validate_token_registry(&reg);
        assert!(diags.iter().any(|d| d.code == "token.registry.empty"));
    }

    #[test]
    fn valid_registry_no_warnings() {
        let reg =
            TokenRegistry::load_from_str(r##"{"color":{"primary":"#fff"}}"##).unwrap();
        let diags = validate_token_registry(&reg);
        assert!(diags.is_empty(), "unexpected diags: {diags:?}");
    }

    #[test]
    fn whitespace_key_emits_invalid_key() {
        let reg =
            TokenRegistry::load_from_str(r##"{"color":{"bad key":"#fff"}}"##).unwrap();
        let diags = validate_token_registry(&reg);
        assert!(
            diags.iter().any(|d| d.code == "token.registry.invalid_key"),
            "expected invalid_key diagnostic, got: {diags:?}"
        );
        assert!(
            diags.iter().any(|d| d.message.contains("bad key")),
            "diagnostic should name the offending key"
        );
    }
}

#[cfg(test)]
mod semcov_wave12_tests {
    use super::*;
    use crate::tokens::TokenRegistry;

    // -----------------------------------------------------------------------
    // Helper
    // -----------------------------------------------------------------------

    fn reg(json: &str) -> TokenRegistry {
        TokenRegistry::load_from_str(json).expect("valid JSON fixture")
    }

    // -----------------------------------------------------------------------
    // Error-path: invalid JSON must not silently succeed
    // -----------------------------------------------------------------------

    #[test]
    fn malformed_json_is_rejected() {
        // Catches: load_from_str swallowing a serde_json parse error and
        // returning a default empty registry instead of Err.
        let result = TokenRegistry::load_from_str("{broken json");
        assert!(
            result.is_err(),
            "load_from_str should propagate serde_json::Error for invalid JSON"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary: empty string (not even a JSON object)
    // -----------------------------------------------------------------------

    #[test]
    fn empty_string_is_rejected_not_treated_as_empty_registry() {
        // Catches: defensive early-return turning "" into Ok(Default::default())
        // which would then hide parse errors from callers.
        let result = TokenRegistry::load_from_str("");
        assert!(
            result.is_err(),
            "empty string is not valid JSON — must be Err, not Ok(empty registry)"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary: registry with only $-prefixed meta keys emits empty warning
    // -----------------------------------------------------------------------

    #[test]
    fn dollar_prefix_keys_are_skipped_and_registry_is_empty() {
        // Catches: walk_json accidentally ingesting $schema / $version keys into
        // by_css_var so that is_empty() returns false even though no usable
        // tokens were registered.
        let r = reg(r#"{"$schema":"http://example.com","$version":"1"}"#);
        assert!(
            r.is_empty(),
            "$ meta-keys must not populate by_css_var; registry should be empty"
        );
        let diags = validate_token_registry(&r);
        assert!(
            diags.iter().any(|d| d.code == "token.registry.empty"),
            "meta-only registry must trigger the empty diagnostic"
        );
    }

    // -----------------------------------------------------------------------
    // Invariant: all keys returned by all_keys() survive a lookup()
    // -----------------------------------------------------------------------

    #[test]
    fn all_keys_are_always_individually_lookable() {
        // Catches: by_css_var and an index diverging so that all_keys()
        // yields keys that lookup() cannot find (broken invariant).
        let r = reg(r#"{"color":{"primary":"#3a86ff","secondary":"#ff6b6b"},"spacing":{"sm":"8px","md":"16px"}}"#);
        for key in r.all_keys() {
            assert!(
                r.lookup(key).is_some(),
                "all_keys() returned {key:?} but lookup({key:?}) returned None — index out of sync"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Invariant: multiple whitespace keys all generate separate diagnostics
    // -----------------------------------------------------------------------

    #[test]
    fn each_whitespace_key_gets_its_own_diagnostic() {
        // Catches: early-return after the first invalid-key diagnostic that
        // silently drops subsequent violations from the report.
        let r = reg(r#"{"color":{"bad one":"#fff","bad two":"#000","good":"#aaa"}}"#);
        let diags = validate_token_registry(&r);
        let invalid_key_count = diags
            .iter()
            .filter(|d| d.code == "token.registry.invalid_key")
            .count();
        assert!(
            invalid_key_count >= 2,
            "expected at least 2 invalid_key diagnostics for 2 whitespace keys, got {invalid_key_count}"
        );
    }

    // -----------------------------------------------------------------------
    // Invariant: tab character is whitespace (not just space)
    // -----------------------------------------------------------------------

    #[test]
    fn tab_in_key_triggers_invalid_key_diagnostic() {
        // Catches: whitespace check using ' ' literal instead of char::is_whitespace(),
        // which would miss tab, newline, and other Unicode whitespace.
        let r = reg("{\"color\":{\"bad\tkey\":\"#fff\"}}");
        let diags = validate_token_registry(&r);
        assert!(
            diags.iter().any(|d| d.code == "token.registry.invalid_key"),
            "tab character in key must trigger invalid_key, got: {diags:?}"
        );
    }

    // -----------------------------------------------------------------------
    // State: validate is idempotent — calling twice yields the same result
    // -----------------------------------------------------------------------

    #[test]
    fn validate_is_idempotent() {
        // Catches: validate_token_registry mutating the registry or accumulating
        // state so that a second call returns more/fewer diagnostics than the first.
        let r = reg(r#"{"color":{"bad key":"#fff","ok":"#000"}}"#);
        let first = validate_token_registry(&r);
        let second = validate_token_registry(&r);
        assert_eq!(
            first.len(),
            second.len(),
            "validate_token_registry must be pure: first call={}, second call={}",
            first.len(),
            second.len()
        );
        for (a, b) in first.iter().zip(second.iter()) {
            assert_eq!(
                a.code, b.code,
                "diagnostic codes must be stable across repeated calls"
            );
        }
    }
}
