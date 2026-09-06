//! Distribution SSOT parity gate (Track 0). Mirrors the pattern in
//! crates/vox-telemetry/tests/taxonomy_ssot_parity.rs.

use voxup::profiles::{self, Profiles};

const SSOT_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../contracts/distribution/profiles.v1.yaml"
);

fn load() -> Profiles {
    let txt = std::fs::read_to_string(SSOT_PATH)
        .expect("contracts/distribution/profiles.v1.yaml must exist");
    profiles::parse(&txt).expect("distribution SSOT must parse")
}

#[test]
fn schema_version_is_one() {
    assert_eq!(load().schema_version, 1);
}

#[test]
fn every_tier_binary_is_a_declared_binary() {
    let p = load();
    for (tier, t) in &p.tiers {
        for b in &t.binaries {
            assert!(
                p.binaries.contains(b),
                "tier '{tier}' ships binary '{b}' not in top-level binaries list"
            );
        }
    }
}

#[test]
fn three_tiers_exist() {
    let p = load();
    for name in ["minimal", "default", "full"] {
        assert!(p.tiers.contains_key(name), "tier '{name}' must be declared");
    }
}

#[test]
fn publish_and_non_publishable_are_disjoint() {
    let p = load();
    for c in &p.non_publishable {
        assert!(
            !p.publish.crates.contains(c),
            "crate '{c}' is in BOTH publish.crates and non_publishable"
        );
    }
}

#[test]
fn agy_only_in_full_tier_runtime_optional() {
    let p = load();
    for (tier, t) in &p.tiers {
        let has_agy = t.runtime_optional.iter().any(|d| d == "agy");
        if tier == "full" {
            assert!(has_agy, "full tier must list agy as runtime_optional");
        } else {
            assert!(!has_agy, "tier '{tier}' must NOT list agy");
        }
    }
}

#[test]
fn rust_version_matches_toolchain_contract() {
    let p = load();
    let contract_txt = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/toolchain/workspace-toolchain.v1.yaml"
    ))
    .expect("workspace-toolchain.v1.yaml must exist");
    let contract_rust = voxup::profiles::toolchain_rust_version(&contract_txt)
        .expect("workspace-toolchain.v1.yaml must have versions.rust");
    assert_eq!(
        contract_rust, p.rust_version,
        "SSOT rust_version '{}' != toolchain contract versions.rust '{contract_rust}'",
        p.rust_version
    );
}

#[test]
fn publish_set_is_subset_of_public_toml() {
    let p = load();
    let public = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../_public.toml"))
        .expect("crates/_public.toml must exist");
    let public_crates = voxup::profiles::public_toml_crates(&public);
    assert!(
        !public_crates.is_empty(),
        "_public.toml 'crates' array must be non-empty"
    );
    for c in &p.publish.crates {
        assert!(
            public_crates.contains(c),
            "SSOT publish crate '{c}' is not declared in crates/_public.toml"
        );
    }
}

#[test]
fn when_publish_enabled_every_crate_is_actually_publishable() {
    let p = load();
    if !p.publish.enabled {
        return; // deferred — Track C flips this; nothing to enforce yet.
    }
    for c in &p.publish.crates {
        let manifest = std::fs::read_to_string(format!(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../{}/Cargo.toml"),
            c
        ))
        .unwrap_or_else(|_| panic!("publish crate '{c}' has no crates/{c}/Cargo.toml"));
        assert!(
            !voxup::profiles::cargo_publish_is_false(&manifest),
            "publish.enabled is true but crate '{c}' has `publish = false` in its Cargo.toml"
        );
    }
}

/// Installer behaviour: the plan `voxup` will execute for each tier, not a
/// YAML-self-consistency check. `full` must ask for `vox-ml-cli`; every
/// tier's `bundle:` must resolve through the existing `bundle_resolved()`.
#[test]
fn installer_plan_honours_tier_binaries_and_resolved_bundle() {
    let p = load();
    let cases: &[(&str, &[&str], &str)] = &[
        ("minimal", &["vox-langtool"], "vox-base"),
        ("default", &["vox"], "vox-fullstack"),
        ("full", &["vox", "vox-ml-cli", "voxup"], "vox-dev"),
    ];
    for (tier, expected_bins, expected_bundle) in cases {
        let bins = voxup::install_plan::binaries_for_tier(&p, tier)
            .unwrap_or_else(|e| panic!("installer plan for '{tier}': {e}"));
        let got: Vec<&str> = bins.iter().map(String::as_str).collect();
        assert_eq!(
            got, *expected_bins,
            "installer would place the wrong binaries for tier '{tier}'"
        );
        let bundle = voxup::install_plan::bundle_id_for_tier(&p, tier)
            .unwrap_or_else(|e| panic!("installer plan for '{tier}' bundle: {e}"));
        assert_eq!(bundle, *expected_bundle);
        vox_plugin_catalog::bundle_resolved(bundle).unwrap_or_else(|e| {
            panic!(
                "installer-selected bundle '{bundle}' for tier '{tier}' \
                 must resolve via bundle_resolved(): {e}"
            )
        });
    }
}

#[test]
fn declared_binaries_have_crate_dirs() {
    // Maps SSOT binary name -> crate directory under crates/.
    // vox is produced by vox-cli; vox-ml-cli and voxup match their dir names.
    let dir_for = |bin: &str| -> String {
        match bin {
            "vox" => "vox-cli".to_string(),
            other => other.to_string(),
        }
    };
    let p = load();
    for bin in &p.binaries {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../").to_string() + &dir_for(bin);
        assert!(
            std::path::Path::new(&dir).is_dir(),
            "SSOT binary '{bin}' expects crate dir '{dir}' which does not exist"
        );
    }
}
