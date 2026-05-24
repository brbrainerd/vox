use vox_dei_shim::research::provider::{ProviderConfig, ProviderRegistry};

#[test]
fn provider_registry_default_primary_name() {
    let r = ProviderRegistry::default();
    assert_eq!(r.primary_name(), "stub");
}

#[test]
fn provider_registry_from_env_with_config_does_not_panic() {
    let cfg = ProviderConfig::default();
    let r = ProviderRegistry::from_env_with_config(cfg);
    assert!(!r.primary_name().is_empty());
}
