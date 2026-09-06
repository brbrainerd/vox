//! Pure install-plan helpers: which binaries and which catalog bundle a tier
//! selects. No network, no filesystem writes.
//!
//! Plugin membership is resolved by the existing
//! `vox_plugin_catalog::bundle_resolved()` (called from tests). This module
//! does not add a third spelling.

use crate::profiles::{Profiles, Tier};
use anyhow::{Result, bail};

/// Binaries the installer must place for `tier`.
pub fn binaries_for_tier<'a>(profiles: &'a Profiles, tier: &str) -> Result<&'a [String]> {
    Ok(&tier_entry(profiles, tier)?.binaries)
}

/// Catalog bundle id the installer records for `tier`.
pub fn bundle_id_for_tier<'a>(profiles: &'a Profiles, tier: &str) -> Result<&'a str> {
    let t = tier_entry(profiles, tier)?;
    match t.bundle.as_deref() {
        Some(id) if !id.is_empty() => Ok(id),
        _ => bail!("tier '{tier}' has no bundle: key — profiles.v1.yaml is incomplete"),
    }
}

fn tier_entry<'a>(profiles: &'a Profiles, tier: &str) -> Result<&'a Tier> {
    profiles
        .tiers
        .get(tier)
        .ok_or_else(|| anyhow::anyhow!("unknown tier '{tier}'"))
}

/// Platform executable name for a SSOT binary id (`vox` → `vox.exe` on Windows).
pub fn exe_name(binary: &str) -> String {
    if cfg!(windows) {
        if binary.ends_with(".exe") {
            binary.to_string()
        } else {
            format!("{binary}.exe")
        }
    } else {
        binary.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::{PROFILES_YAML, parse};

    #[test]
    fn minimal_ships_langtool_only() {
        let p = parse(PROFILES_YAML).unwrap();
        let bins = binaries_for_tier(&p, "minimal").unwrap();
        assert_eq!(bins, &["vox-langtool".to_string()]);
    }

    #[test]
    fn default_ships_vox() {
        let p = parse(PROFILES_YAML).unwrap();
        let bins = binaries_for_tier(&p, "default").unwrap();
        assert_eq!(bins, &["vox".to_string()]);
    }

    #[test]
    fn full_ships_ml_cli() {
        let p = parse(PROFILES_YAML).unwrap();
        let bins = binaries_for_tier(&p, "full").unwrap();
        assert!(bins.iter().any(|b| b == "vox-ml-cli"), "got {bins:?}");
        assert!(bins.iter().any(|b| b == "vox"), "got {bins:?}");
        assert!(bins.iter().any(|b| b == "voxup"), "got {bins:?}");
    }

    #[test]
    fn every_shipped_tier_names_a_bundle() {
        let p = parse(PROFILES_YAML).unwrap();
        for name in ["minimal", "default", "full"] {
            let id = bundle_id_for_tier(&p, name).unwrap();
            assert!(!id.is_empty(), "tier {name} bundle id empty");
        }
    }

    #[test]
    fn exe_name_is_platform_correct() {
        if cfg!(windows) {
            assert_eq!(exe_name("vox"), "vox.exe");
        } else {
            assert_eq!(exe_name("vox"), "vox");
        }
    }
}
