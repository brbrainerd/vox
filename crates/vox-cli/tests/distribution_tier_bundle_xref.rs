//! Cross-reference test for P7 Ruling R4 (see docs/superpowers/plans/2026-09-05-00-INDEX.md
//! §2.1): `contracts/distribution/profiles.v1.yaml` `tiers` (Layer 1: audience -> package
//! identity) and `crates/vox-plugin-catalog/catalog.toml` `[[bundle]]` (Layer 2: capability
//! -> plugin set) are orthogonal axes joined by exactly one cross-reference: each tier's
//! `bundle:` key. This test asserts that key is not a decorative string — every tier's
//! `bundle` must name a bundle that actually resolves in the plugin catalog — so the two
//! files can no longer drift apart silently.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProfilesFile {
    tiers: BTreeMap<String, Tier>,
}

#[derive(Debug, Deserialize)]
struct Tier {
    #[serde(default)]
    bundle: Option<String>,
}

fn load_profiles() -> ProfilesFile {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/distribution/profiles.v1.yaml");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
    serde_yaml::from_str(&raw).unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
}

#[test]
fn every_tier_bundle_resolves_in_the_plugin_catalog() {
    let profiles = load_profiles();
    assert!(
        !profiles.tiers.is_empty(),
        "expected at least one tier in profiles.v1.yaml"
    );

    for (tier_name, tier) in &profiles.tiers {
        let bundle_id = tier
            .bundle
            .as_deref()
            .unwrap_or_else(|| panic!("tier `{tier_name}` has no `bundle:` cross-reference"));
        assert!(
            !bundle_id.is_empty(),
            "tier `{tier_name}` has an empty `bundle:` cross-reference"
        );

        vox_plugin_catalog::bundle_resolved(bundle_id).unwrap_or_else(|e| {
            panic!(
                "tier `{tier_name}` references bundle `{bundle_id}`, which does not resolve \
                 in crates/vox-plugin-catalog/catalog.toml: {e:?}"
            )
        });
    }
}

#[test]
fn unknown_bundle_id_does_not_resolve() {
    // Negative control: without this, the positive test above would pass vacuously
    // if `bundle_resolved` silently returned `Ok` for anything.
    let result = vox_plugin_catalog::bundle_resolved("no-such-bundle");
    assert!(
        result.is_err(),
        "expected `no-such-bundle` to fail to resolve, got {result:?}"
    );
}
