//! The 2026-08-21 laptop query, end to end. This is the gate that unlocks the
//! discovery pipeline, so it asserts the criterion the spec actually states —
//! that the answer EXPLAINS the unified-memory reservation — not merely 48 < 64.

use vox_cli_core::market::*;

fn schema() -> CatalogSchema {
    CatalogSchema::load_from_str(include_str!(
        "../../../contracts/market/catalog-schema.v1.yaml"
    ))
    .expect("shipped contract parses")
}

fn seeded() -> Vec<CatalogItem> {
    seed_from_yaml(&schema(), include_str!("fixtures/laptops_seed.yaml")).expect("seed")
}

#[test]
fn only_128gb_machines_satisfy_a_64gb_gpu_memory_constraint() {
    let out = apply(
        &schema(),
        &seeded(),
        &[Constraint::gte("gpu_accessible_gb", 64.0, "GB")],
    );

    let passed: Vec<&str> = out.passed.iter().map(|i| i.item_id.as_str()).collect();
    assert!(passed.contains(&"mbp16-m5max-128"), "got {passed:?}");
    assert!(passed.contains(&"rog-flow-z13-128"), "got {passed:?}");
    assert!(
        !passed.contains(&"mbp14-m5pro-64"),
        "a 64GB machine exposes ~48GB and must not pass: {passed:?}"
    );

    let why = out
        .excluded
        .iter()
        .find(|e| e.item_id == "mbp14-m5pro-64")
        .expect("a measured miss, not an indeterminate");

    let r = why.reason.to_lowercase();
    assert!(
        r.contains("48"),
        "must state the derived value: {}",
        why.reason
    );
    assert!(
        r.contains("64"),
        "must state the requirement: {}",
        why.reason
    );
    // The criterion the spec states. A generic
    // "gpu_accessible_gb 48 GB < required 64 GB" satisfies the two lines above
    // and fails this one — deliberately.
    assert!(
        r.contains("unified") || r.contains("reserves"),
        "must explain WHY a 64GB machine offers 48GB: {}",
        why.reason
    );
}

/// The seed states `total_memory_gb`, which a vendor page really does say. The
/// 48 must come from the derivation, so a source claiming
/// `gpu_accessible_gb: 64` outright cannot quietly become the catalog's answer.
#[test]
fn gpu_accessible_memory_is_derived_from_total_memory_not_asserted() {
    let items = seeded();
    let mbp = items
        .iter()
        .find(|i| i.item_id == "mbp14-m5pro-64")
        .unwrap();

    let total = mbp.attributes.get("total_memory_gb").expect("observed");
    assert_eq!(total.number, 64.0);
    assert_eq!(
        total.evidence,
        Evidence::MerchantPage,
        "a page really does state 64GB"
    );

    let derived = mbp.attributes.get("gpu_accessible_gb").expect("derived");
    assert_eq!(derived.number, 48.0);
    assert_eq!(
        derived.evidence,
        Evidence::Derived,
        "no merchant page states 48GB — it must not borrow a page's provenance"
    );
}

#[test]
fn a_source_may_not_assert_a_derived_attribute() {
    const CHEATING: &str = r#"
items:
  - item_id: liar
    category: laptop
    arch: apple_unified
    attributes:
      total_memory_gb:   { number: 64, unit: GB, evidence: merchant_page }
      gpu_accessible_gb: { number: 64, unit: GB, evidence: merchant_page }
"#;
    let e = seed_from_yaml(&schema(), CHEATING).unwrap_err().to_string();
    assert!(e.contains("gpu_accessible_gb"), "got: {e}");
}

#[test]
fn the_reserve_is_per_architecture_not_a_flat_quarter() {
    // Apple's reserve and Strix Halo's are set by different mechanisms
    // (iogpu.wired_limit_percent vs a UEFI setting), so one shared constant
    // would be a coincidence rather than a rule. Asserted separately so a
    // future per-architecture correction touches one line and one assertion.
    assert_eq!(
        derive_gpu_accessible_gb(64.0, Arch::AppleUnified).number,
        48.0
    );
    assert_eq!(
        derive_gpu_accessible_gb(128.0, Arch::AppleUnified).number,
        96.0
    );
    assert_eq!(
        derive_gpu_accessible_gb(128.0, Arch::StrixHalo).number,
        96.0
    );
    // A discrete GPU's VRAM is not carved out of system memory at all.
    assert_eq!(derive_gpu_accessible_gb(64.0, Arch::Discrete).number, 64.0);
}

#[test]
fn an_impossible_constraint_explains_itself_rather_than_returning_empty() {
    let items = seeded();
    let out = apply(
        &schema(),
        &items,
        &[Constraint::gte("gpu_accessible_gb", 512.0, "GB")],
    );

    assert!(out.passed.is_empty());
    assert_eq!(
        out.excluded.len(),
        items.len(),
        "every item carries a reason"
    );
    for e in &out.excluded {
        // "Explains itself" was previously unasserted: the first draft checked
        // only that the exclusion list was the right LENGTH, which an
        // implementation emitting empty strings satisfies.
        assert!(
            !e.reason.trim().is_empty(),
            "empty reason for {}",
            e.item_id
        );
        assert!(
            e.reason.contains("512"),
            "must name the requirement: {}",
            e.reason
        );
    }
    // The closest candidate must be legible, so the reader learns how far off
    // the requirement is rather than only that nothing matched.
    assert!(
        out.excluded.iter().any(|e| e.reason.contains("96")),
        "the best machine in the catalog must be visible: {:?}",
        out.excluded
    );
}
