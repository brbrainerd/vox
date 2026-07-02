/// Phase 4 — Check D extended to ALL env var reads, not just VOX_* names.
/// Covers bare non-VOX reads (DB_PASSWORD), wrapper helpers (env_flag),
/// env::var_os, and the THIRD_PARTY_ALLOWLIST (HOME, PATH, etc.).
use vox_cli_ci::config_hygiene::{THIRD_PARTY_ALLOWLIST, check_env_reads_registered};

fn empty_registered() -> std::collections::HashSet<String> {
    std::collections::HashSet::new()
}

#[test]
fn bare_db_password_env_read_is_detected() {
    let src = r#"let pw = std::env::var("DB_PASSWORD").ok();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert_eq!(
        hits.len(),
        1,
        "DB_PASSWORD should be detected as unregistered"
    );
    assert_eq!(hits[0].check, "env-var-not-in-registry");
    assert_eq!(hits[0].env_var.as_deref(), Some("DB_PASSWORD"));
}

#[test]
fn env_var_os_is_detected() {
    let src = r#"let v = std::env::var_os("OPENAI_API_KEY").unwrap();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert_eq!(hits.len(), 1, "var_os reads should also be detected");
    assert_eq!(hits[0].env_var.as_deref(), Some("OPENAI_API_KEY"));
}

#[test]
fn wrapper_helper_env_flag_is_detected() {
    let src = r#"let enabled = env_flag("MY_FEATURE_FLAG");"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert_eq!(hits.len(), 1, "env_flag wrapper should be detected");
    assert_eq!(hits[0].env_var.as_deref(), Some("MY_FEATURE_FLAG"));
}

#[test]
fn wrapper_helper_env_u32_is_detected() {
    let src = r#"let n = env_u32("MAX_CONNECTIONS", 10);"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert_eq!(hits.len(), 1, "env_u32 wrapper should be detected");
    assert_eq!(hits[0].env_var.as_deref(), Some("MAX_CONNECTIONS"));
}

#[test]
fn home_dir_in_allowlist_is_not_flagged() {
    // HOME is in THIRD_PARTY_ALLOWLIST — should pass even without a registry row.
    let src = r#"let h = std::env::var("HOME").unwrap_or_default();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert!(
        hits.is_empty(),
        "HOME is in THIRD_PARTY_ALLOWLIST and must not be flagged"
    );
}

#[test]
fn rust_log_in_allowlist_is_not_flagged() {
    let src = r#"let level = std::env::var("RUST_LOG").ok();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert!(hits.is_empty(), "RUST_LOG is in THIRD_PARTY_ALLOWLIST");
}

#[test]
fn github_actions_in_allowlist_is_not_flagged() {
    let src = r#"let ci = std::env::var("GITHUB_ACTIONS").is_ok();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert!(
        hits.is_empty(),
        "GITHUB_ACTIONS is in THIRD_PARTY_ALLOWLIST"
    );
}

#[test]
fn registered_non_vox_name_is_not_flagged() {
    let mut registered = std::collections::HashSet::new();
    registered.insert("DATABASE_URL".to_string());
    let src = r#"let url = std::env::var("DATABASE_URL").expect("db url");"#;
    let hits = check_env_reads_registered(src, "x.rs", &registered);
    assert!(
        hits.is_empty(),
        "explicitly registered name must not be flagged"
    );
}

#[test]
fn vox_name_unregistered_still_flagged() {
    // The old VOX_* behaviour is preserved: unregistered VOX_ names are still caught.
    let src = r#"let v = std::env::var("VOX_UNREGISTERED_KNOB").ok();"#;
    let hits = check_env_reads_registered(src, "x.rs", &empty_registered());
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].env_var.as_deref(), Some("VOX_UNREGISTERED_KNOB"));
}

#[test]
fn third_party_allowlist_contains_expected_entries() {
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"HOME"));
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"PATH"));
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"RUST_LOG"));
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"CARGO_MANIFEST_DIR"));
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"GITHUB_ACTIONS"));
    assert!(THIRD_PARTY_ALLOWLIST.contains(&"TEMP"));
}
