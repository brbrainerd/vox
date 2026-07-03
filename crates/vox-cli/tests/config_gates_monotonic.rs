//! Config-gate convergence + monotonic-baseline ratchet.
//!
//! 1. The config-registry-parity gate must recognize env vars registered in ANY
//!    of the federated SSOT sources (YAML registry, Clavis-managed secrets, typed
//!    `CONFIG_KEYS`) — not just the typed Rust registry. Otherwise YAML-only or
//!    Clavis-only vars are falsely flagged as "unregistered".
//! 2. The registration-backlog baselines must not silently grow: each is pinned
//!    under a generous cap so they can only ratchet down as findings get fixed.
//!
//! Harvested (re-derived via TDD) from salvage/env-ssot-p6. The branch's pre-push
//! wiring was intentionally dropped — it predated and would regress main's 25m
//! pre-push timeout guard.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
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

/// The parity gate's recognized set must be a superset of BOTH the YAML registry
/// and the typed `CONFIG_KEYS` registry — proving the federated union, not just
/// the Rust registry, drives the gate.
#[test]
fn unified_set_is_superset_of_yaml_and_typed_registries() {
    let root = workspace_root();

    let yaml_text = fs::read_to_string(root.join("contracts/config/registry.v1.yaml"))
        .expect("contracts/config/registry.v1.yaml must exist");
    let yaml_vars: BTreeSet<String> =
        vox_cli_ci::config_hygiene::load_registered_env_vars(&yaml_text)
            .into_iter()
            .collect();

    let unified = vox_cli_ci::config_registry_parity::unified_registered_set(&root);

    let missing_yaml: Vec<_> = yaml_vars.difference(&unified).cloned().collect();
    assert!(
        missing_yaml.is_empty(),
        "unified registered set is missing YAML-registered vars: {missing_yaml:?}"
    );

    let missing_typed: Vec<_> = vox_config::config_registry::registered_keys()
        .filter(|k| !unified.contains(*k))
        .collect();
    assert!(
        missing_typed.is_empty(),
        "unified registered set is missing typed CONFIG_KEYS: {missing_typed:?}"
    );
}

/// Monotonic-shrink ratchet: baseline files must stay under a pinned cap. They can
/// only shrink as findings are registered; raising a cap is an intentional, commit-
/// explained act. Caps = current main counts (hygiene 299, registry 706) + headroom.
#[test]
fn baselines_must_not_grow() {
    const MAX_HYGIENE_BASELINE: usize = 320;
    const MAX_REGISTRY_BASELINE: usize = 730;
    let root = workspace_root();

    let hygiene = non_comment_lines(&root.join("contracts/config/config-hygiene-baseline.txt"));
    assert!(
        hygiene <= MAX_HYGIENE_BASELINE,
        "config-hygiene-baseline.txt has {hygiene} non-comment lines, over the monotonic cap \
         {MAX_HYGIENE_BASELINE}. Register the findings (shrink), or raise the cap with an explanation."
    );

    let registry = non_comment_lines(&root.join("contracts/config/config-registry-baseline.txt"));
    assert!(
        registry <= MAX_REGISTRY_BASELINE,
        "config-registry-baseline.txt has {registry} non-comment lines, over the monotonic cap \
         {MAX_REGISTRY_BASELINE}. Register the findings (shrink), or raise the cap with an explanation."
    );
}
