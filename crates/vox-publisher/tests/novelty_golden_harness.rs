//! Golden harness for `assess_novelty`.
//!
//! Loads `tests/fixtures/novelty_golden.v1.json`, runs every case through
//! `assess_novelty` with the default `NoveltyConfig`, then:
//!
//! - Asserts every verdict matches the labeled expected class (floor 1.0 —
//!   all inputs are deterministic, so any mismatch is a scorer regression or
//!   an intentional re-label).
//! - Emits per-class precision / recall over the four verdict classes.
//! - For the conflict-pair case, additionally asserts `conflicts` is non-empty.
//!
//! ## Why a floor of 1.0 on deterministic inputs?
//!
//! Unlike probabilistic or model-driven tests, every input here is hand-crafted
//! with known scores and years.  The scorer is a pure function of those values.
//! A mismatch means either (a) someone changed scorer logic without updating
//! the fixture labels — which CI should catch — or (b) a deliberate re-label
//! was made and the author forgot to update this harness.

use serde::Deserialize;
use vox_publisher::scientia_finding_ledger::NoveltyEvidenceBundleV1;
use vox_publisher::scientia_novelty_assess::assess_novelty;
use vox_scientia::inspect_bridge::NoveltyConfig;

// ---------------------------------------------------------------------------
// Fixture types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GoldenCase {
    name: String,
    claim_year: i32,
    expected: String,
    bundle: NoveltyEvidenceBundleV1,
}

// ---------------------------------------------------------------------------
// Per-class tallies for precision / recall
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ClassTally {
    /// True positives: predicted this class AND expected this class.
    tp: usize,
    /// False positives: predicted this class but expected something else.
    fp: usize,
    /// False negatives: expected this class but predicted something else.
    fn_: usize,
}

fn precision(t: &ClassTally) -> Option<f64> {
    let denom = t.tp + t.fp;
    if denom == 0 {
        None
    } else {
        Some(t.tp as f64 / denom as f64)
    }
}

fn recall(t: &ClassTally) -> Option<f64> {
    let denom = t.tp + t.fn_;
    if denom == 0 {
        None
    } else {
        Some(t.tp as f64 / denom as f64)
    }
}

// ---------------------------------------------------------------------------
// Test
// ---------------------------------------------------------------------------

#[test]
fn novelty_golden_harness() {
    // ------------------------------------------------------------------
    // Load fixture
    // ------------------------------------------------------------------
    let fixture_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/novelty_golden.v1.json");

    let raw = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixture_path.display()));

    let cases: Vec<GoldenCase> = serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("cannot deserialize novelty_golden.v1.json: {e}"));

    assert!(
        cases.len() >= 12,
        "fixture must contain at least 12 cases; got {}",
        cases.len()
    );

    // ------------------------------------------------------------------
    // Run every case
    // ------------------------------------------------------------------
    let config = NoveltyConfig::default();
    let classes = [
        "insufficient_evidence",
        "novel",
        "possibly_novel",
        "not_novel",
    ];

    let mut tallies: std::collections::HashMap<String, ClassTally> = classes
        .iter()
        .map(|c| (c.to_string(), ClassTally::default()))
        .collect();

    let mut mismatches: Vec<String> = Vec::new();

    for case in &cases {
        let assessment = assess_novelty(&case.bundle, case.claim_year, &config);
        let got = &assessment.verdict_kind;
        let expected = &case.expected;

        // Accumulate tally
        for cls in &classes {
            let cls_str = cls.to_string();
            let t = tallies.entry(cls_str).or_default();
            let predicted_this = got == cls;
            let expected_this = expected == cls;
            if predicted_this && expected_this {
                t.tp += 1;
            } else if predicted_this && !expected_this {
                t.fp += 1;
            } else if !predicted_this && expected_this {
                t.fn_ += 1;
            }
        }

        if got != expected {
            let msg = format!("MISMATCH {}: expected {} got {}", case.name, expected, got);
            eprintln!("{msg}");
            mismatches.push(msg);
        }

        // Special assertion: the conflict-pair case must have non-empty conflicts.
        if case.name.contains("conflict_pair") {
            assert!(
                !assessment.conflicts.is_empty(),
                "case '{}': expected non-empty conflicts vec but got []",
                case.name
            );
        }
    }

    // ------------------------------------------------------------------
    // Per-class precision / recall summary
    // ------------------------------------------------------------------
    eprintln!("\n--- per-class precision/recall (golden harness) ---");
    for cls in &classes {
        let t = &tallies[*cls];
        let p = precision(t).map_or("N/A".to_string(), |v| format!("{:.3}", v));
        let r = recall(t).map_or("N/A".to_string(), |v| format!("{:.3}", v));
        eprintln!(
            "  {:<25}  tp={} fp={} fn={}  precision={}  recall={}",
            cls, t.tp, t.fp, t.fn_, p, r
        );
    }
    eprintln!("---------------------------------------------------\n");

    // ------------------------------------------------------------------
    // Final assertion: all cases must match
    // ------------------------------------------------------------------
    assert!(
        mismatches.is_empty(),
        "\n{} golden case(s) failed.\n\nGoldens are deterministic: a mismatch means either a \
         scorer regression (scorer logic changed without updating labels) or a deliberate \
         re-label (update the fixture AND remove this assertion if the behavior change is \
         intentional).\n\nFailed cases:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
