//! vox-config is a *view* over the vox-llm-config SSOT. This smoke test proves the
//! re-export is reachable and the registry is non-empty. The full accessor-coverage
//! parity test arrives with Band-A Task 1.2 (registry population).
use vox_config::vox_llm_config::{self, Kind};

#[test]
fn registry_reachable_through_vox_config() {
    assert!(
        !vox_llm_config::LLM_CONFIG_KEYS.is_empty(),
        "registry must be reachable and seeded via vox_config re-export"
    );
}

#[test]
fn known_endpoint_key_is_registered() {
    let k = vox_llm_config::get("OPENROUTER_BASE_URL").expect("seed key present");
    assert_eq!(k.kind, Kind::Url);
    assert!(!k.secret);
    assert_eq!(k.default, "https://openrouter.ai/api");
}

#[test]
fn gui_fields_exclude_secret_api_keys() {
    let fields = vox_llm_config::gui_fields();
    assert!(
        fields.iter().all(|f| f.key != "OPENROUTER_API_KEY"),
        "secret API keys must not appear in the GUI field projection"
    );
    assert!(
        fields.iter().any(|f| f.key == "OPENROUTER_BASE_URL"),
        "non-secret endpoint keys must appear in the GUI projection"
    );
}
