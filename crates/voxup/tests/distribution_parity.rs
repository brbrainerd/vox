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
