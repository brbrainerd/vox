//! W1 regression guard: any workflow that turns sccache ON at the **top-level
//! `env:`** must also pin `CARGO_INCREMENTAL: "0"` there, or sccache silently
//! caches nothing (incremental artifacts are not cacheable).
//!
//! This is a cross-line invariant (two keys in one block), which a single-line
//! arch-check `forbidden_pattern` cannot express — so it lives as a pure fn
//! exercised by a unit test that scans every workflow. The check is scoped to
//! the pre-`jobs:` header so the deliberate step-level `RUSTC_WRAPPER: ""`
//! opt-out in `toolchain-lint-wave` is neither required nor flagged.

/// True if the workflow's top-level env is sccache-safe: either it does not turn
/// sccache on at the top level, or it pins `CARGO_INCREMENTAL: "0"` there too.
pub fn top_env_pins_incremental(workflow_text: &str) -> bool {
    let header = workflow_text.split("\njobs:").next().unwrap_or(workflow_text);
    let on = header.contains("\n  RUSTC_WRAPPER: sccache");
    let pinned = header.contains("\n  CARGO_INCREMENTAL: \"0\"");
    !on || pinned
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_sccache_without_incremental() {
        assert!(!top_env_pins_incremental("env:\n  RUSTC_WRAPPER: sccache\njobs:\n  x: {}"));
    }

    #[test]
    fn passes_when_pinned() {
        assert!(top_env_pins_incremental(
            "env:\n  RUSTC_WRAPPER: sccache\n  CARGO_INCREMENTAL: \"0\"\njobs:\n  x: {}"
        ));
    }

    #[test]
    fn ignores_step_level_optout_after_jobs() {
        // A job that sets RUSTC_WRAPPER below `jobs:` must NOT trip the header check.
        assert!(top_env_pins_incremental(
            "env:\n  CARGO_TERM_COLOR: always\njobs:\n  lint:\n    env:\n      RUSTC_WRAPPER: sccache"
        ));
    }

    /// Scans every real workflow file — fails naming any sccache-on workflow that
    /// forgets the top-level `CARGO_INCREMENTAL: "0"`. Runs under `cargo nextest`,
    /// so it gates pre-merge without new CLI wiring.
    #[test]
    fn all_sccache_workflows_pin_incremental() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows");
        let mut bad = Vec::new();
        for entry in std::fs::read_dir(dir).expect("workflows dir") {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) == Some("yml") {
                let txt = std::fs::read_to_string(&path).unwrap();
                if !top_env_pins_incremental(&txt) {
                    bad.push(path.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }
        assert!(
            bad.is_empty(),
            "sccache-on workflows missing top-level CARGO_INCREMENTAL=0: {bad:?}"
        );
    }
}
