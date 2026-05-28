use vox_plugin_catalog::{all_bundles, all_plugins};

#[test]
fn catalog_has_at_least_one_plugin() {
    let plugins = all_plugins();
    assert!(!plugins.is_empty(), "catalog has zero plugins");
    assert!(plugins.iter().any(|p| p.id == "mens-candle-cuda"));
}

#[test]
fn catalog_has_at_least_one_bundle() {
    let bundles = all_bundles();
    assert!(!bundles.is_empty(), "catalog has zero bundles");
    assert!(bundles.iter().any(|b| b.id == "vox-base"));
}

#[test]
fn catalog_has_all_code_and_composite_plugins() {
    let plugins = all_plugins();
    let code_ids: Vec<&str> = plugins
        .iter()
        .filter(|p| {
            matches!(
                p.payload_kind,
                vox_plugin_catalog::schema::PayloadKind::Code
                    | vox_plugin_catalog::schema::PayloadKind::Composite
            )
        })
        .map(|p| p.id.as_str())
        .collect();
    // All current first-party code/composite plugins.
    let expected = [
        "nvml-probe",
        "mens-candle-cuda",
        "mens-candle-metal",
        "oratio",
        "cloud",
        "populi-mesh",
        "webhook",
        "browser",
        "runtime-wasm",
        "runtime-container",
        "publication",
    ];
    for id in expected {
        assert!(
            code_ids.contains(&id),
            "missing code/composite plugin: {id}"
        );
    }
}

#[test]
fn catalog_has_all_skill_plugins() {
    let plugins = all_plugins();
    let skill_ids: Vec<&str> = plugins
        .iter()
        .filter(|p| {
            matches!(
                p.payload_kind,
                vox_plugin_catalog::schema::PayloadKind::Skill
            )
        })
        .map(|p| p.id.as_str())
        .collect();
    let expected = [
        "skill-compiler",
        "skill-git",
        "skill-memory",
        "skill-orchestrator",
        "skill-rag",
        "skill-testing",
        "skill-testing-validate",
        "skill-v0",
    ];
    for id in expected {
        assert!(skill_ids.contains(&id), "missing skill plugin: {id}");
    }
}

#[test]
fn catalog_has_all_bundles() {
    let bundles = all_bundles();
    let ids: Vec<&str> = bundles.iter().map(|b| b.id.as_str()).collect();
    let expected = [
        "vox-base",
        "vox-fullstack",
        "vox-ml",
        "vox-ml-metal",
        "vox-mesh",
        "vox-server",
        "vox-edge",
        "vox-cloud-only",
        "vox-dev",
        "vox-mobile",
    ];
    for id in expected {
        assert!(ids.contains(&id), "missing bundle: {id}");
    }
}

#[test]
fn status_field_is_present_on_entries() {
    use vox_plugin_catalog::schema::CatalogStatus;
    let plugins = all_plugins();
    // webhook was explicitly set to stable.
    let webhook = plugins.iter().find(|p| p.id == "webhook").expect("webhook");
    assert_eq!(webhook.status, CatalogStatus::Stable);
    // cloud was set to alpha.
    let cloud = plugins.iter().find(|p| p.id == "cloud").expect("cloud");
    assert_eq!(cloud.status, CatalogStatus::Alpha);
    // skill-v0 was marked deprecated.
    let v0 = plugins.iter().find(|p| p.id == "skill-v0").expect("skill-v0");
    assert_eq!(v0.status, CatalogStatus::Deprecated);
}
