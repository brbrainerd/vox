//! Typed reader for the distribution SSOT (`contracts/distribution/profiles.v1.yaml`).
//!
//! The SSOT is embedded at compile time so release binaries can validate tiers
//! without needing access to the source repository.

/// The distribution SSOT, embedded at compile time.
pub const PROFILES_YAML: &str = include_str!("../../../contracts/distribution/profiles.v1.yaml");

use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Profiles {
    pub schema_version: u32,
    pub rust_version: String,
    pub binaries: Vec<String>,
    pub tiers: std::collections::BTreeMap<String, Tier>,
    pub publish: Publish,
    pub non_publishable: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Tier {
    pub description: String,
    pub binaries: Vec<String>,
    /// Layer-2 catalog bundle id (`vox-base` / `vox-fullstack` / `vox-dev`).
    /// Orthogonal to `binaries`; resolved by `vox_plugin_catalog::bundle_resolved`.
    #[serde(default)]
    pub bundle: Option<String>,
    pub build_deps: Vec<String>,
    pub runtime_optional: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct Publish {
    pub enabled: bool,
    pub crates: Vec<String>,
}

/// Parse the SSOT from a YAML string.
pub fn parse(yaml: &str) -> Result<Profiles, serde_yaml::Error> {
    serde_yaml::from_str(yaml)
}

/// Extract `versions.rust` from a workspace-toolchain.v1.yaml document.
// Used by crates/voxup/tests/distribution_parity.rs; clippy can't see cross-unit usage.
#[allow(dead_code)]
pub fn toolchain_rust_version(yaml: &str) -> Option<String> {
    let v: serde_yaml::Value = serde_yaml::from_str(yaml).ok()?;
    v["versions"]["rust"].as_str().map(String::from)
}

/// Extract the top-level `crates = [...]` string array from `_public.toml`.
// Used by crates/voxup/tests/distribution_parity.rs; clippy can't see cross-unit usage.
#[allow(dead_code)]
pub fn public_toml_crates(toml_text: &str) -> Vec<String> {
    let v: toml::Value = match toml::from_str(toml_text) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    v.get("crates")
        .and_then(|c| c.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// True iff a Cargo.toml sets `[package] publish = false`.
// Used by crates/voxup/tests/distribution_parity.rs; clippy can't see cross-unit usage.
#[allow(dead_code)]
pub fn cargo_publish_is_false(cargo_toml_text: &str) -> bool {
    let v: toml::Value = match toml::from_str(cargo_toml_text) {
        Ok(v) => v,
        Err(_) => return false,
    };
    matches!(
        v.get("package").and_then(|p| p.get("publish")),
        Some(toml::Value::Boolean(false))
    )
}

/// Validate that `tier` names a tier declared in the distribution SSOT.
///
/// Returns `Ok(())` on a known tier, or `Err` with a human-readable message
/// listing the valid tier names (derived from the SSOT, not hard-coded).
pub fn validate_tier(profiles_yaml: &str, tier: &str) -> Result<(), String> {
    let profiles = parse(profiles_yaml).map_err(|e| format!("SSOT parse error: {e}"))?;
    if profiles.tiers.contains_key(tier) {
        return Ok(());
    }
    let mut valid: Vec<&str> = profiles.tiers.keys().map(String::as_str).collect();
    valid.sort_unstable();
    Err(format!(
        "unknown tier '{tier}'. Valid tiers: {}",
        valid.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let yaml = r#"
schema_version: 1
rust_version: "1.98.1"
binaries: [vox]
tiers:
  minimal:
    description: "x"
    binaries: [vox]
    build_deps: [rust]
    runtime_optional: []
publish:
  enabled: false
  crates: [voxup]
non_publishable: [vox-orchestrator-mcp]
"#;
        let p = parse(yaml).expect("must parse");
        assert_eq!(p.schema_version, 1);
        assert_eq!(p.rust_version, "1.98.1");
        assert!(p.tiers.contains_key("minimal"));
        assert!(!p.publish.enabled);
    }
}
