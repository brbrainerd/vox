//! `examples/golden/` doctor-green regression guard.
//!
//! Every `.vox` file under `examples/golden/` is canonical reference
//! material — docs link to them, the LLM-target training corpus pulls
//! from them, and `vox doctor --project` over them is part of the CR-L0
//! freshness gate. Any compiler tightening that breaks a golden example
//! must be caught here, not by a confused user reading stale docs.
//!
//! Files opt out via the leading `// vox:skip` annotation — used today
//! only for `tensor_gc_computation.vox`, which intentionally exercises
//! pre-release tensor surface that's not yet typeable.
//!
//! When this test red-lines, the fix is one of:
//!   1. Update the example to the current compiler surface (preferred —
//!      keeps the reference material useful), or
//!   2. Add `// vox:skip` as the first line with a comment explaining
//!      why (drift-toward-broken is a regression, not an acceptable
//!      escape hatch).

use vox_compiler::typeck::diagnostics::TypeckSeverity;

/// Read `examples/golden/*.vox` from the workspace root.
fn golden_dir() -> std::path::PathBuf {
    vox_audit::workspace_root().join("examples").join("golden")
}

fn is_skipped(src: &str) -> bool {
    src.lines()
        .next()
        .is_some_and(|line| line.trim_start().starts_with("// vox:skip"))
}

#[test]
fn every_golden_example_compiles_clean() {
    let dir = golden_dir();
    assert!(
        dir.is_dir(),
        "examples/golden/ should exist at {}",
        dir.display()
    );

    let mut total = 0usize;
    let mut skipped = 0usize;
    let mut clean = 0usize;
    let mut failing: Vec<(String, Vec<String>)> = Vec::new();

    for entry in std::fs::read_dir(&dir).expect("read examples/golden/") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("vox") {
            continue;
        }
        total += 1;
        let src = std::fs::read_to_string(&path).expect("read .vox");
        if is_skipped(&src) {
            skipped += 1;
            continue;
        }

        let diags = vox_compiler::pipeline::check_file(&src, &path.to_string_lossy());
        let errors: Vec<String> = diags
            .iter()
            .filter(|d| matches!(d.severity, TypeckSeverity::Error))
            .map(|d| d.message.clone())
            .collect();

        if errors.is_empty() {
            clean += 1;
        } else {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            failing.push((name, errors));
        }
    }

    if !failing.is_empty() {
        let mut msg = format!(
            "examples/golden/ regressions: {} of {} files have compile errors \
             (clean={clean}, skipped={skipped}). Fix the examples or add \
             `// vox:skip` (with a comment) to the first line.\n\n",
            failing.len(),
            total
        );
        for (name, errs) in &failing {
            msg.push_str(&format!("  ✗ {name}\n"));
            for e in errs.iter().take(3) {
                msg.push_str(&format!("      {e}\n"));
            }
            if errs.len() > 3 {
                msg.push_str(&format!("      … +{} more\n", errs.len() - 3));
            }
        }
        panic!("{msg}");
    }

    // Sanity: the directory should not be empty, and `clean + skipped + failing`
    // must account for every .vox file we counted.
    assert!(total > 0, "no .vox files found under {}", dir.display());
    assert_eq!(
        clean + skipped,
        total,
        "counter mismatch — bug in this test"
    );
}

/// `apps/marquee/<app>/src/main.vox` carries the three reference marquee
/// applications (todo-auth, chat, etc. — see
/// `contracts/marquee/manifest.v1.yaml`). They must compile clean — a
/// red marquee app is a demo regression that ships to LLM training and
/// onboarding tutorials.
#[test]
fn every_marquee_app_compiles_clean() {
    let root = vox_audit::workspace_root().join("apps").join("marquee");
    if !root.is_dir() {
        // Worktrees may not include marquee — skip rather than fail.
        return;
    }

    let mut total = 0usize;
    let mut failing: Vec<(String, Vec<String>)> = Vec::new();

    for entry in std::fs::read_dir(&root).expect("read apps/marquee/") {
        let app = entry.expect("dir entry").path();
        if !app.is_dir() {
            continue;
        }
        let main = app.join("src").join("main.vox");
        if !main.is_file() {
            continue;
        }
        total += 1;
        let src = std::fs::read_to_string(&main).expect("read main.vox");
        if is_skipped(&src) {
            continue;
        }
        let diags = vox_compiler::pipeline::check_file(&src, &main.to_string_lossy());
        let errors: Vec<String> = diags
            .iter()
            .filter(|d| matches!(d.severity, TypeckSeverity::Error))
            .map(|d| d.message.clone())
            .collect();
        if !errors.is_empty() {
            let name = app
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();
            failing.push((name, errors));
        }
    }

    if !failing.is_empty() {
        let mut msg = format!(
            "marquee app regressions: {} of {} apps have compile errors.\n\n",
            failing.len(),
            total
        );
        for (name, errs) in &failing {
            msg.push_str(&format!("  ✗ apps/marquee/{name}\n"));
            for e in errs.iter().take(3) {
                msg.push_str(&format!("      {e}\n"));
            }
            if errs.len() > 3 {
                msg.push_str(&format!("      … +{} more\n", errs.len() - 3));
            }
        }
        panic!("{msg}");
    }
}

/// `apps/interop/marquee_app/src/main.vox` is slot 1 of the canonical
/// marquee app set (`contracts/marquee/manifest.v1.yaml`). It lives outside
/// `apps/marquee/` so the previous guard missed it. CR-P1 references all 3
/// marquee slots — this test plus `every_marquee_app_compiles_clean` cover
/// them.
#[test]
fn marquee_slot_1_interop_app_compiles_clean() {
    let main = vox_audit::workspace_root()
        .join("apps")
        .join("interop")
        .join("marquee_app")
        .join("src")
        .join("main.vox");
    if !main.is_file() {
        // Worktree may not include slot 1; skip rather than fail.
        return;
    }
    let src = std::fs::read_to_string(&main).expect("read interop marquee_app main.vox");
    if is_skipped(&src) {
        return;
    }
    let diags = vox_compiler::pipeline::check_file(&src, &main.to_string_lossy());
    let errors: Vec<String> = diags
        .iter()
        .filter(|d| matches!(d.severity, TypeckSeverity::Error))
        .map(|d| d.message.clone())
        .collect();
    assert!(
        errors.is_empty(),
        "apps/interop/marquee_app/src/main.vox (slot 1) regressed: {errors:?}"
    );
}
