//! Split-brain parity guard (pipeline-gap PATTERN #7): `vox-effort-audit` and
//! `vox-effort-route` each carry a byte-identical copy of `ModelRates::cost_usd`.
//! If one copy's pricing math drifts (rounding, direction swap, known/None
//! handling) the other must too, or the judge-cost reported by `audit` and
//! `route` silently disagree. This test fails on any behavioral divergence —
//! a strictly stronger guard than the `cross_crate_dup` detector, which only
//! warns while the bodies stay byte-identical and goes silent once they diverge
//! cosmetically while staying behaviorally split.

use vox_effort_audit::pricing::ModelRates as AuditRates;
use vox_effort_route::pricing::ModelRates as RouteRates;

/// Build both crates' copies from the same primitive inputs (the structs are
/// distinct types with no shared trait, so parity is asserted on `cost_usd`
/// output, not on the structs themselves).
fn pair(i: f64, o: f64, known: bool) -> (AuditRates, RouteRates) {
    (
        AuditRates {
            input_per_1k_usd: i,
            output_per_1k_usd: o,
            known,
        },
        RouteRates {
            input_per_1k_usd: i,
            output_per_1k_usd: o,
            known,
        },
    )
}

#[test]
fn cost_usd_agrees_across_audit_and_route() {
    // Representative matrix: normal cost, honest zero, direction-sensitive
    // (input-only vs output-only catches a swapped prompt/completion term),
    // known-free model, and unknown (must be None on both sides).
    let cases: &[(f64, f64, bool, u64, u64)] = &[
        (3.0, 15.0, true, 2000, 1000),  // normal
        (3.0, 15.0, true, 0, 0),        // honest $0
        (1.0, 100.0, true, 1000, 0),    // direction-sensitive: input-only
        (1.0, 100.0, true, 0, 1000),    // direction-sensitive: output-only
        (0.0, 0.0, true, 500, 500),     // known free model -> Some(0.0)
        (3.0, 15.0, false, 1000, 1000), // unknown -> None on both
    ];
    for &(i, o, known, p, c) in cases {
        let (a, r) = pair(i, o, known);
        assert_eq!(
            a.cost_usd(p, c),
            r.cost_usd(p, c),
            "split-brain: audit vs route disagree for rates=({i},{o},known={known}) tokens=({p},{c})"
        );
    }
}

#[test]
fn default_unknown_parity() {
    // Both ::default() must be `known=false` and return None — divergence here
    // means one crate started fabricating $0.00 for unpriced models.
    assert_eq!(
        AuditRates::default().cost_usd(9999, 9999),
        RouteRates::default().cost_usd(9999, 9999),
        "default (unknown) pricing must agree across crates"
    );
    assert_eq!(AuditRates::default().cost_usd(9999, 9999), None);
}
