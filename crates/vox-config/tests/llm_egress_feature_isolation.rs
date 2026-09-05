//! Guards the `vox-compiler` -> `vox-config` -> `vox-llm-egress` -> `vox-openai`
//! coupling that Task 1 of P7 ("Lean Core & Persona Split") severed.
//!
//! `vox-config` gates its `resolve_egress` module (and the `vox-llm-egress`
//! dependency it pulls in) behind a default-OFF `llm-egress` Cargo feature so
//! that a language-only artifact (`vox-compiler` and everything under it) does
//! not transitively link an OpenAI client. See spec §9, audience A, and
//! `crates/vox-config/Cargo.toml` for the feature declaration.
//!
//! `cargo tree -p X` roots feature resolution at package `X`; it is not
//! workspace-unified, so `-p` is a valid tool for an absence assertion even
//! though other workspace members enable the `llm-egress` feature elsewhere.

use std::process::Command;

/// Runs `cargo tree` for the given package/args, rooted at the workspace
/// manifest. Returns `None` (after `eprintln!`) if `cargo` could not be
/// spawned at all, so this test degrades gracefully in an environment with no
/// cargo rather than panicking on an unrelated infrastructure gap.
fn run_cargo_tree(args: &[&str]) -> Option<String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    // Workspace root is one level up from `crates/vox-config`.
    let workspace_root = std::path::Path::new(manifest_dir)
        .parent()
        .and_then(std::path::Path::parent)
        .expect("CARGO_MANIFEST_DIR should be crates/vox-config under the workspace root");

    let output = match Command::new(env!("CARGO"))
        .current_dir(workspace_root)
        .arg("tree")
        .args(args)
        .output()
    {
        Ok(output) => output,
        Err(err) => {
            eprintln!(
                "skipping llm-egress feature isolation test: could not spawn `cargo tree`: {err}"
            );
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "`cargo tree {args:?}` did not succeed (status: {:?}); stderr:\n{stderr}",
            output.status.code()
        );
    }

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[test]
fn vox_compiler_dependency_tree_excludes_vox_openai() {
    let Some(stdout) = run_cargo_tree(&["-p", "vox-compiler", "-e", "normal"]) else {
        return;
    };
    assert!(
        !stdout.contains("vox-openai"),
        "vox-compiler must not transitively depend on vox-openai (the language-only \
         artifact would link an LLM client); full `cargo tree -p vox-compiler -e normal` \
         output:\n{stdout}"
    );
    eprintln!(
        "OK: vox-compiler dependency tree ({} bytes) contains no vox-openai",
        stdout.len()
    );
}

/// Positive control: without this, a typo in "vox-openai" above would make the
/// negative assertion vacuously true. `vox-config --features llm-egress` is
/// the one place in this crate that still reaches `vox-llm-egress` ->
/// `vox-openai`, so the edge must still exist when the feature is enabled.
#[test]
fn vox_config_with_llm_egress_feature_still_reaches_vox_openai() {
    let Some(stdout) = run_cargo_tree(&["-p", "vox-config", "--features", "llm-egress"]) else {
        return;
    };
    assert!(
        stdout.contains("vox-openai"),
        "vox-config built with --features llm-egress must still reach vox-openai via \
         vox-llm-egress, or this test suite's negative assertion is vacuous; full \
         `cargo tree -p vox-config --features llm-egress` output:\n{stdout}"
    );
    eprintln!(
        "OK: vox-config --features llm-egress dependency tree ({} bytes) contains vox-openai",
        stdout.len()
    );
}
