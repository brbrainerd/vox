//! Cross-module integration test exercising `vox-runtime-rn` exactly the
//! way the JS side will (after uniffi-bindgen-react-native generates the
//! TypeScript TurboModule bindings).

use std::path::PathBuf;

use vox_runtime_rn::{
    RuntimeProfile, VoxConfig, VoxRuntimeHandle, default_desktop_config, default_mobile_config,
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
