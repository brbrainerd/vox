//! Cross-module integration test exercising `vox-runtime-rn` exactly the
//! way the JS side will (after uniffi-bindgen-react-native generates the
//! TypeScript TurboModule bindings).

use std::path::PathBuf;

use vox_runtime_rn::{
    JournalLine, RuntimeProfile, VoxConfig, VoxRuntimeHandle, default_desktop_config,
    default_mobile_config, open_file_journal,
};

#[test]
fn full_round_trip_mobile_lifecycle() {
    // Step 1 — JS side requests a mobile-default config.
    let cfg = default_mobile_config("/tmp/vox-rt-rn-it".to_string());
    assert_eq!(cfg.profile, RuntimeProfile::Mobile);
    assert_eq!(cfg.log_level, "info");

    // Step 2 — JS constructs the runtime handle from that config.
    let h = VoxRuntimeHandle::new(cfg.clone());
    assert_eq!(h.profile(), RuntimeProfile::Mobile);
    assert!(h.requires_suspend_hooks());

    // Step 3 — handle round-trips the data + model dirs back to JS as strings.
    let data_back: PathBuf = h.data_dir().into();
    let model_back: PathBuf = h.model_dir().into();
    assert_eq!(data_back, PathBuf::from("/tmp/vox-rt-rn-it/data"));
    assert_eq!(model_back, PathBuf::from("/tmp/vox-rt-rn-it/models"));

    // Step 4 — log() should accept every level and never panic.
    for level in &["error", "warn", "info", "debug", "trace", "garbage"] {
        h.log((*level).to_string(), "integration test".to_string());
    }
}

#[test]
fn desktop_handle_does_not_require_suspend_hooks() {
    let cfg = default_desktop_config();
    assert_eq!(cfg.profile, RuntimeProfile::Desktop);
    let h = VoxRuntimeHandle::new(cfg);
    assert!(!h.requires_suspend_hooks());
}

#[test]
fn caller_provided_config_overrides_defaults() {
    let cfg = VoxConfig {
        data_dir: "/custom/data".to_string(),
        model_dir: "/custom/models".to_string(),
        log_level: "debug".to_string(),
        profile: RuntimeProfile::Mobile,
    };
    let h = VoxRuntimeHandle::new(cfg);
    let data: PathBuf = h.data_dir().into();
    assert_eq!(data, PathBuf::from("/custom/data"));
}

/// The on-device journal opened through the uniffi bridge must use the
/// lifecycle-deferred durability mode (no per-append fsync) and survive a
/// reopen once `flush()` — the OS-suspend hook — has run. This exercises the
/// full mobile durability contract end-to-end through the bridge exactly as
/// the JS lifecycle handler will: append while foregrounded, `flush()` on
/// background, reopen on next launch and replay.
#[test]
fn mobile_journal_round_trips_through_deferred_flush() {
    let path = std::env::temp_dir().join(format!(
        "vox-rt-rn-journal-it-{}.ndjson",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let path_str = path.to_string_lossy().into_owned();

    // Foreground: append two mutations. Deferred mode means these are NOT
    // fsynced per-call; the bytes are handed to the OS and made durable at
    // flush() time.
    let journal = open_file_journal(path_str.clone()).expect("open journal");
    journal
        .append(JournalLine {
            json: r#"{"table":"entries","id":1}"#.to_string(),
        })
        .expect("append 1");
    journal
        .append(JournalLine {
            json: r#"{"table":"entries","id":2}"#.to_string(),
        })
        .expect("append 2");

    // Background lifecycle hook: flush() is the durability point under
    // Deferred mode (not a no-op).
    journal.flush().expect("flush on suspend");
    drop(journal);

    // Next launch: reopen and replay — both mutations must survive.
    let reopened = open_file_journal(path_str).expect("reopen journal");
    let lines = reopened.replay_all().expect("replay");
    assert_eq!(lines.len(), 2, "both deferred appends must survive flush");
    assert!(lines[0].json.contains("\"id\":1"));
    assert!(lines[1].json.contains("\"id\":2"));

    let _ = std::fs::remove_file(&path);
}
