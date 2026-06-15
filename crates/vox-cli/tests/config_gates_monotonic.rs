//! Phase 6 monotonic-shrink and gate-convergence tests.
//!
//! - Verify that both gates agree on a shared fixture var in the YAML registry.
//! - Assert baseline line counts are within a pinned maximum so they can only
//!   shrink over time (monotonic ratchet).

use std::fs;
use std::path::Path;

fn workspace_root() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR is crates/vox-cli; workspace root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root must exist")
}

fn non_comment_lines(path: &Path) -> usize {
    fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .count()
}

/// `VOX_WASM_SKILL_FUEL` is registered in both:
///   - `contracts/config/registry.v1.yaml` (Check D SSOT)
///   - `vox_config::config_registry::CONFIG_KEYS` (typed Rust registry)
///
/// Both gates must recognize it as registered, confirming the unified set works.
#[test]
fn both_gates_agree_on_fixture_var() {
    let root = workspace_root();

    // Check D (config-hygiene) source: parse registry.v1.yaml.
    let registry_path = root.join("contracts/config/registry.v1.yaml");
    let yaml_text =
        fs::read_to_string(&registry_path).expect("contracts/config/registry.v1.yaml must exist");
    let hygiene_set = vox_cli::commands::ci::config_hygiene::load_registered_env_vars(&yaml_text);
    assert!(
        hygiene_set.contains("VOX_WASM_SKILL_FUEL"),
        "config-hygiene Check D must recognize VOX_WASM_SKILL_FUEL from registry.v1.yaml"
    );

    // config-registry-parity: unified set (YAML + Clavis + CONFIG_KEYS).
    let parity_set = vox_cli::commands::ci::config_registry_parity::unified_registered_set(&root);
    assert!(
        parity_set.contains("VOX_WASM_SKILL_FUEL"),
        "config-registry-parity unified set must recognize VOX_WASM_SKILL_FUEL"
    );
}

/// Monotonic-shrink assertion: baseline files must not grow beyond their pinned
/// maximum. When a finding is properly registered the baseline shrinks; when a
/// temporary exemption is added both the count update and this cap must be raised
/// with an explanation in the commit message.
///
/// The cap is intentionally generous (current + headroom) so CI does not break on
/// legitimate temporary additions, while still catching large unexplained additions.
#[test]
fn baseline_must_not_grow() {
    let root = workspace_root();

    let hygiene_path = root.join("contracts/config/config-hygiene-baseline.txt");
    let hygiene_count = non_comment_lines(&hygiene_path);
    assert!(
        hygiene_count <= MAX_HYGIENE_BASELINE,
        "config-hygiene-baseline.txt has {hygiene_count} non-comment lines, \
         but the monotonic cap is {MAX_HYGIENE_BASELINE}. \
         If you are adding a legitimate temporary exemption, raise the cap and explain in your commit."
    );

    let registry_path = root.join("contracts/config/config-registry-baseline.txt");
    let registry_count = non_comment_lines(&registry_path);
    assert!(
        registry_count <= MAX_REGISTRY_BASELINE,
        "config-registry-baseline.txt has {registry_count} non-comment lines, \
         but the monotonic cap is {MAX_REGISTRY_BASELINE}. \
         If you are adding a legitimate temporary exemption, raise the cap and explain in your commit."
    );
}

/// Maximum permitted non-comment lines in each baseline file.
/// These caps define the monotonic shrink ratchet — they can only decrease as
/// findings are properly registered, never increase without an intentional raise.
///
/// Current baseline counts: hygiene=300, registry=608.
/// Cap = current + 20-line headroom for temporary exemptions before a PR cycle.
const MAX_HYGIENE_BASELINE: usize = 320;
const MAX_REGISTRY_BASELINE: usize = 628;
