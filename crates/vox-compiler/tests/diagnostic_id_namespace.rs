//! P1-T9 acceptance: Phase 1 diagnostic IDs conform to the vox/<category>/<kebab> namespace.

use vox_compiler::typeck::diagnostics::codes;

#[test]
fn every_phase_1_code_is_kebab_case() {
    for code in codes::ALL_PHASE_1 {
        assert!(
            code.starts_with("vox/"),
            "code `{code}` missing vox/ prefix"
        );
        let parts: Vec<&str> = code.split('/').collect();
        assert_eq!(
            parts.len(),
            3,
            "code `{code}` must be `vox/<category>/<kebab>`"
        );
        let category = parts[1];
        let kebab = parts[2];
        assert!(
            category.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "category `{category}` in `{code}` must be lowercase-kebab"
        );
        assert!(
            kebab
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "kebab `{kebab}` in `{code}` must be lowercase-kebab"
        );
        assert!(
            !kebab.starts_with('-'),
            "code `{code}` kebab must not start with hyphen"
        );
        assert!(
            !kebab.ends_with('-'),
            "code `{code}` kebab must not end with hyphen"
        );
    }
}

#[test]
fn category_set_is_known() {
    let allowed: std::collections::HashSet<&'static str> =
        ["types", "effect", "workflow", "remote", "api"]
            .into_iter()
            .collect();
    for code in codes::ALL_PHASE_1 {
        let category = code.split('/').nth(1).unwrap();
        assert!(
            allowed.contains(category),
            "category `{category}` in `{code}` not in allowed set {allowed:?}"
        );
    }
}

#[test]
fn no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for code in codes::ALL_PHASE_1 {
        assert!(seen.insert(*code), "duplicate code `{code}` in ALL_PHASE_1");
    }
}
