use std::path::PathBuf;
use tempfile::TempDir;
use vox_telemetry::config::{
    ConsentState, install_id, install_salt, is_remote_allowed, remote_consent, set_remote_consent,
};

// These tests mutate env vars and files. Run with `--test-threads=1` or
// use serial_test if parallelism is ever added.

fn temp_config_env(dir: &TempDir) -> PathBuf {
    let d = dir.path().to_path_buf();
    // On Windows the path is built from APPDATA; on Unix from XDG_CONFIG_HOME or HOME.
    #[cfg(windows)]
    unsafe {
        std::env::set_var("APPDATA", &d);
    }
    #[cfg(not(windows))]
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", &d);
    }
    d
}

#[test]
fn consent_round_trips_granted() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    set_remote_consent(ConsentState::Granted);
    assert_eq!(remote_consent(), ConsentState::Granted);
}

#[test]
fn consent_round_trips_denied() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    set_remote_consent(ConsentState::Denied);
    assert_eq!(remote_consent(), ConsentState::Denied);
}

#[test]
fn consent_missing_file_returns_unset() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);
    // No file written — should be Unset.
    assert_eq!(remote_consent(), ConsentState::Unset);
}

#[test]
fn consent_unset_is_noop() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    // set_remote_consent(Unset) must not write a file.
    set_remote_consent(ConsentState::Unset);
    assert_eq!(remote_consent(), ConsentState::Unset);
    let path = dir.path().join("vox").join("remote-consent");
    // The consent file must NOT exist after a no-op Unset write.
    assert!(
        !path.exists(),
        "Unset write must not create the consent file"
    );
}

#[test]
fn install_id_is_stable_across_calls() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    let id1 = install_id();
    let id2 = install_id();
    assert_eq!(id1, id2, "install_id must be stable once persisted");
    // Must be a valid UUID string (hyphen-formatted, 36 chars).
    assert_eq!(id1.len(), 36, "install_id must be 36-char UUID string");
}

#[test]
fn install_salt_is_stable_across_calls() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    let s1 = install_salt();
    let s2 = install_salt();
    assert_eq!(s1, s2, "install_salt must be stable once persisted");
    // Must be exactly 16 bytes.
    assert_eq!(s1.len(), 16);
}

#[test]
fn install_salt_is_different_from_install_id_bytes() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);

    let id = install_id();
    let salt = install_salt();
    // They come from separate UUID v4 calls — extremely unlikely to be equal.
    let id_hex: String = id.replace('-', "");
    let salt_hex: String = salt.iter().map(|b| format!("{b:02x}")).collect();
    assert_ne!(
        id_hex, salt_hex,
        "install_id and install_salt must be independent"
    );
}

#[test]
fn is_remote_allowed_false_when_unset_consent() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);
    unsafe {
        std::env::remove_var("VOX_TELEMETRY");
    }
    // No consent set → Unset → remote NOT allowed.
    assert!(!is_remote_allowed());
}

#[test]
fn is_remote_allowed_true_when_granted_and_master_on() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);
    unsafe {
        std::env::remove_var("VOX_TELEMETRY");
    }
    set_remote_consent(ConsentState::Granted);
    assert!(is_remote_allowed());
}

#[test]
fn is_remote_allowed_false_when_master_off_even_if_granted() {
    let dir = TempDir::new().unwrap();
    temp_config_env(&dir);
    unsafe {
        std::env::set_var("VOX_TELEMETRY", "off");
    }
    set_remote_consent(ConsentState::Granted);
    // Master switch wins.
    assert!(!is_remote_allowed());
    unsafe {
        std::env::remove_var("VOX_TELEMETRY");
    }
}
