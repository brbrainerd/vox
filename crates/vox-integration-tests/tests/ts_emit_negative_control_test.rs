//! Negative control for the TS emit typecheck gate.
//!
//! The positive gate (`ts_emit_typecheck_test`) proves emitted TS *passes* `tsc`.
//! That alone does not prove the gate would *catch* a regression — a gate that
//! silently always passes (wrong path, swallowed exit code, empty file set) looks
//! identical to a healthy one. This test injects THREE deliberate, structurally
//! different type errors and asserts `tsc` rejects each — a single case would only
//! prove one error class is caught, and the emitter's real failure surface (JSX
//! props, reactive hooks, async handlers) is broader than a scalar mismatch.
//!
//! Run explicitly:
//!   cargo nextest run -p vox-integration-tests --test ts_emit_negative_control_test --run-ignored ignored-only

#![allow(missing_docs)]

use vox_integration_tests::{run_tsc_noemit, strict_tsconfig_json, ts_scratch_dir};

/// One negative-control case: a filename, the invalid TS source, and the
/// `tsc` diagnostic code it must produce.
struct NegativeCase {
    filename: &'static str,
    source: &'static str,
    expected_code: &'static str,
}

const CASES: &[NegativeCase] = &[
    // Scalar type mismatch — the simplest case, proves tsc runs at all.
    NegativeCase {
        filename: "scalar_mismatch.ts",
        source: r#"
export function negativeControlScalar(): number {
  const n: number = "this is a string, not a number";
  return n;
}
"#,
        expected_code: "TS2322",
    },
    // JSX prop mismatch — mirrors the emitter's actual component/jsx.rs output shape.
    NegativeCase {
        filename: "jsx_prop_mismatch.tsx",
        source: r#"
interface WidgetProps {
  count: number;
}
function Widget(props: WidgetProps) {
  return <div>{props.count}</div>;
}
export function negativeControlJsx() {
  return <Widget count="not a number" />;
}
"#,
        expected_code: "TS2322",
    },
    // Broken import — mirrors a class of emitter bug where a generated import
    // path is wrong (module resolution failure, distinct diagnostic family).
    NegativeCase {
        filename: "broken_import.ts",
        source: r#"
import { thisSymbolDoesNotExistAnywhere } from "./nonexistent_module_xyz";
export function negativeControlImport() {
  return thisSymbolDoesNotExistAnywhere();
}
"#,
        expected_code: "TS2307",
    },
];

#[test]
#[ignore = "requires node in PATH; run with --run-ignored ignored-only — owner: integration-tests sunset: 2026-12-31"]
fn tsc_gate_rejects_deliberately_bad_typescript() {
    let scratch = ts_scratch_dir();

    // Isolated dir so this never collides with the positive gate's __emit_test__.
    let neg_dir = scratch.join("__negative_control__");
    if neg_dir.exists() {
        std::fs::remove_dir_all(&neg_dir).expect("Failed to clean __negative_control__");
    }
    std::fs::create_dir_all(&neg_dir).expect("Failed to create __negative_control__");

    for case in CASES {
        std::fs::write(neg_dir.join(case.filename), case.source)
            .unwrap_or_else(|e| panic!("Failed to write {}: {e}", case.filename));
    }

    // Same compilerOptions as the positive gate — proving the SAME config rejects these.
    let tsconfig_path = neg_dir.join("tsconfig.json");
    std::fs::write(
        &tsconfig_path,
        serde_json::to_string_pretty(&strict_tsconfig_json()).unwrap(),
    )
    .expect("Failed to write tsconfig.json");

    let output = run_tsc_noemit(&scratch, &tsconfig_path);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    assert!(
        !output.status.success(),
        "NEGATIVE CONTROL FAILED: tsc ACCEPTED all deliberately-invalid TypeScript.\n\
         This means the emit typecheck gate is not actually checking anything.\n\
         {combined}"
    );

    for case in CASES {
        // Check filename and code co-occur on the SAME diagnostic line (tsc's format is
        // `filename(line,col): error TSxxxx: message`), not just that both strings appear
        // anywhere in the combined output. Two cases share TS2322 — a whole-output substring
        // check could pass even if a case's diagnostic were misattributed or emitted the
        // wrong code, as long as some OTHER case's line happened to carry the same code.
        let matched = combined
            .lines()
            .any(|line| line.contains(case.filename) && line.contains(case.expected_code));
        assert!(
            matched,
            "Expected a diagnostic line containing both {} and {} — tsc output did not report \
             this error class on this file's own line:\n{combined}",
            case.expected_code, case.filename
        );
    }

    std::fs::remove_dir_all(&neg_dir).ok();
    println!(
        "Negative control passed: tsc correctly rejected all {} error classes.",
        CASES.len()
    );
}
