//! Mirrors what `build.rs` checks. If this passes, the catalog is well-formed.
//! `build.rs` runs the same logic at compile time so structural breakage is
//! a build error, not a runtime error.

use vox_plugin_catalog::schema::PayloadKind;
use vox_plugin_catalog::{all_bundles, all_plugins};

#[test]
fn every_plugin_id_is_unique() {
    let mut seen = std::collections::HashSet::new();
    for plugin in all_plugins() {
        assert!(
            seen.insert(&plugin.id),
            "duplicate plugin id: {}",
            plugin.id
        );
    }
}

#[test]
fn every_bundle_id_is_unique() {
    let mut seen = std::collections::HashSet::new();
    for bundle in all_bundles() {
        assert!(
            seen.insert(&bundle.id),
            "duplicate bundle id: {}",
            bundle.id
        );
    }
}

#[test]
fn every_bundled_in_reference_exists() {
    let bundle_ids: std::collections::HashSet<&str> =
        all_bundles().iter().map(|b| b.id.as_str()).collect();
    for plugin in all_plugins() {
        for bundle_id in &plugin.bundled_in {
            assert!(
                bundle_ids.contains(bundle_id.as_str()),
                "plugin '{}' lists bundled-in='{}', but no such bundle exists",
                plugin.id,
                bundle_id
            );
        }
    }
}

#[test]
fn every_bundle_plugin_reference_exists() {
    let plugin_ids: std::collections::HashSet<&str> =
        all_plugins().iter().map(|p| p.id.as_str()).collect();
    for bundle in all_bundles() {
        for plugin_id in &bundle.plugins {
            assert!(
                plugin_ids.contains(plugin_id.as_str()),
                "bundle '{}' lists plugin '{}', but no such plugin exists",
                bundle.id,
                plugin_id
            );
        }
    }
}

#[test]
fn every_extends_target_exists() {
    let bundle_ids: std::collections::HashSet<&str> =
        all_bundles().iter().map(|b| b.id.as_str()).collect();
    for bundle in all_bundles() {
        if let Some(parent) = &bundle.extends {
            assert!(
                bundle_ids.contains(parent.as_str()),
                "bundle '{}' extends '{}', but no such bundle exists",
                bundle.id,
                parent
            );
        }
    }
}

#[test]
fn code_plugins_declare_extension_points() {
    for plugin in all_plugins() {
        if matches!(
            plugin.payload_kind,
            PayloadKind::Code | PayloadKind::Composite
        ) {
            assert!(
                plugin.extension_points.is_some()
                    && !plugin.extension_points.as_ref().unwrap().is_empty(),
                "code/composite plugin '{}' must declare extension-points",
                plugin.id
            );
        }
    }
}

#[test]
fn skill_plugins_declare_exposed_tools() {
    for plugin in all_plugins() {
        if matches!(
            plugin.payload_kind,
            PayloadKind::Skill | PayloadKind::Composite
        ) {
            assert!(
                plugin.exposes_tools.is_some()
                    && !plugin.exposes_tools.as_ref().unwrap().is_empty(),
                "skill/composite plugin '{}' must declare exposes-tools",
                plugin.id
            );
        }
    }
}

#[test]
fn every_plugin_has_default_source() {
    // 1a guarantee: every plugin is standalone-installable.
    for plugin in all_plugins() {
        assert!(
            !plugin.default_source.is_empty(),
            "plugin '{}' has empty default-source",
            plugin.id
        );
    }
}

#[test]
fn gpu_ml_backends_use_the_first_party_release_asset_source() {
    // mens-candle-cuda and mens-candle-metal ship as release assets of vox's
    // own repo (see install_first_party_plugin), not a per-plugin repo or a
    // `local:` path. This guards against a future catalog edit accidentally
    // reverting either entry back to one of those other source forms.
    const FIRST_PARTY_SOURCE: &str = "github:vox-foundation/vox";
    for id in ["mens-candle-cuda", "mens-candle-metal"] {
        let plugin = all_plugins()
            .iter()
            .find(|p| p.id == id)
            .unwrap_or_else(|| panic!("expected plugin '{id}' in the catalog"));
        assert_eq!(
            plugin.default_source, FIRST_PARTY_SOURCE,
            "plugin '{}' must use the first-party release-asset source '{}', got '{}'",
            id, FIRST_PARTY_SOURCE, plugin.default_source
        );
    }
}

#[test]
fn every_plugin_bundled_in_claim_is_satisfied_by_the_named_bundle() {
    use vox_plugin_catalog::bundle_resolved;
    for plugin in all_plugins() {
        for bundle_id in &plugin.bundled_in {
            let resolved = bundle_resolved(bundle_id).unwrap_or_default();
            let resolved_ids: std::collections::HashSet<&str> =
                resolved.iter().map(|p| p.id.as_str()).collect();
            assert!(
                resolved_ids.contains(plugin.id.as_str()),
                "plugin '{}' claims bundled-in='{}', but bundle '{}' does not include it (check plugins[] or extends chain)",
                plugin.id,
                bundle_id,
                bundle_id
            );
        }
    }
}
