//! `vox doctor` check: surfaces which runtime-optional deps for an install tier
//! are present or missing on the current machine.

use serde::Deserialize;

/// The distribution SSOT, embedded at compile time.
const PROFILES_YAML: &str =
    include_str!("../../../../../../../contracts/distribution/profiles.v1.yaml");

#[derive(Debug, Deserialize)]
struct Profiles {
    tiers: std::collections::BTreeMap<String, Tier>,
}

#[derive(Debug, Deserialize)]
struct Tier {
    runtime_optional: Vec<String>,
}

/// Status of a single runtime-optional dep.
#[derive(Debug, PartialEq, Eq)]
pub struct DepStatus {
    pub name: String,
    pub present: bool,
    pub hint: String,
}

/// Check which runtime-optional deps for `tier` are present on this machine.
///
/// Returns one `DepStatus` per declared dep.  An empty vec means the tier
/// has no runtime-optional deps (e.g. `minimal` and `default`).
pub fn check_runtime_optional_deps(tier_name: &str) -> Vec<DepStatus> {
    let profiles: Profiles = match serde_yaml::from_str(PROFILES_YAML) {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    let tier = match profiles.tiers.get(tier_name) {
        Some(t) => t,
        None => return Vec::new(),
    };

    tier.runtime_optional
        .iter()
        .map(|dep| dep_status(dep))
        .collect()
}

fn dep_status(dep: &str) -> DepStatus {
    match dep {
        "model-weights" => {
            let present = model_weights_present();
            DepStatus {
                name: dep.to_string(),
                present,
                hint: if present {
                    String::new()
                } else {
                    // `vox mens pull` is not a subcommand — `vox-ml-cli mens` exposes
                    // pipeline/train/serve/corpus/probe and no download verb. Name the
                    // real entry point rather than a command that cannot be run.
                    "No model weights found. Fetch a corpus/model via `vox mens corpus` \
                     or point VOX_MENS_MODEL_DIR at an existing weights directory."
                        .to_string()
                },
            }
        }
        "plugins" => {
            let present = plugins_dir_present();
            DepStatus {
                name: dep.to_string(),
                present,
                hint: if present {
                    String::new()
                } else {
                    "Run `vox plugin install <name>` to install a plugin.".to_string()
                },
            }
        }
        _ => {
            let present = binary_on_path(dep);
            DepStatus {
                name: dep.to_string(),
                present,
                hint: if present {
                    String::new()
                } else {
                    format!(
                        "Install '{dep}' and ensure it is on PATH. See https://voxlang.org/reference/installation/ for the full-tier install guide."
                    )
                },
            }
        }
    }
}

fn binary_on_path(name: &str) -> bool {
    which_binary(name).is_some()
}

fn which_binary(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH")
        .as_deref()
        .unwrap_or_default()
        .to_string_lossy()
        .split(if cfg!(windows) { ';' } else { ':' })
        .map(std::path::Path::new)
        .find_map(|dir| {
            let candidate = if cfg!(windows) {
                dir.join(format!("{name}.exe"))
            } else {
                dir.join(name)
            };
            if candidate.is_file() {
                Some(candidate)
            } else {
                None
            }
        })
}

fn model_weights_present() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".vox").join("models").is_dir())
        .unwrap_or(false)
}

fn plugins_dir_present() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".vox").join("plugins").is_dir())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tier_returns_no_deps() {
        // Construct a synthetic tier with no runtime_optional.
        // Verify by calling with a tier that has no optional deps (minimal).
        let statuses = check_runtime_optional_deps("minimal");
        assert!(
            statuses.is_empty(),
            "minimal tier has no runtime-optional deps: {statuses:?}"
        );
    }

    #[test]
    fn unknown_tier_returns_no_deps() {
        let statuses = check_runtime_optional_deps("tier-that-does-not-exist");
        assert!(statuses.is_empty());
    }

    #[test]
    fn full_tier_returns_nonempty_status() {
        let statuses = check_runtime_optional_deps("full");
        assert!(
            !statuses.is_empty(),
            "full tier declares runtime-optional deps; should get at least one status"
        );
    }

    #[test]
    fn absent_binary_reports_not_present() {
        let s = dep_status("vox-nonexistent-binary-zzz9999");
        assert!(!s.present, "nonexistent binary must report present=false");
        assert!(!s.hint.is_empty(), "must provide an install hint");
    }

    #[test]
    fn absent_binary_hint_contains_name() {
        let s = dep_status("vox-nonexistent-binary-zzz9999");
        assert!(
            s.hint.contains("vox-nonexistent-binary-zzz9999"),
            "hint must mention the dep name: {}",
            s.hint
        );
    }
}
