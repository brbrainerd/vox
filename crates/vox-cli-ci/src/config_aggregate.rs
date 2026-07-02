//! Collects every `#[derive(VoxConfig)]` domain's `config_keys()` into one slice
//! so the config-registry-parity gate sees domain-owned knobs without manual rows.
//! ponytail: explicit list (not linkme) — layering-safe; the coverage test guards drift.

use vox_config::VoxConfigDomain;
use vox_config::config_key::ConfigKey;

/// All domain config keys. Add one `extend_from_slice` per `#[derive(VoxConfig)]`
/// struct as domains are migrated (SP-A: orchestrator; SP-B+: oratio/search/…).
#[must_use]
pub fn all_domain_config_keys() -> Vec<ConfigKey> {
    let mut keys = Vec::new();
    keys.extend_from_slice(vox_orchestrator::config::OrchestratorConfig::config_keys());
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_is_nonempty_and_unique() {
        let keys = all_domain_config_keys();
        assert!(!keys.is_empty());
        let mut names: Vec<_> = keys.iter().map(|k| k.key).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n, "duplicate config key across domains");
    }
}
