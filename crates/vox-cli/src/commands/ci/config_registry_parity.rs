//! `vox ci config-registry-parity`: every operational VOX_* env knob read in code
//! must be a row in vox_config::config_registry::CONFIG_KEYS (the searchable SSOT),
//! and every Env-homed registry row should be read in code. Baseline ratchet
//! grandfathers today's registration backlog (mirrors config-hygiene).

use std::collections::BTreeSet;
use std::path::Path;

/// Returns (unregistered_used, registered_unused).
pub fn parity(
    used: &BTreeSet<String>,
    registered: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    (
        used.difference(registered).cloned().collect(),
        registered.difference(used).cloned().collect(),
    )
}

/// True if `used` is registered, either exactly or via a registered prefix row
/// (a registry name ending in `_` that prefixes `used`, e.g. `VOX_DB_`).
fn is_registered(used: &str, registered: &BTreeSet<String>) -> bool {
    if registered.contains(used) {
        return true;
    }
    registered
        .iter()
        .any(|reg| reg.ends_with('_') && used.starts_with(reg.as_str()))
}

/// Scan all non-test `.rs` under `crates/` for `VOX_[A-Z0-9_]+` env-knob names,
/// skipping this gate file and the registry-definition files (so we count *uses*,
/// not the registry/operator definitions themselves).
fn scan_env_uses(root: &Path) -> BTreeSet<String> {
    let re = regex::Regex::new(r"VOX_[A-Z0-9_]+").unwrap();
    let mut used = BTreeSet::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/");
        if rel.contains("config_registry_parity.rs")
            || rel.contains("config_registry.rs")
            || rel.contains("operator_registry.rs")
        {
            return;
        }
        for m in re.find_iter(src) {
            used.insert(m.as_str().to_string());
        }
    });
    used
}

/// The set of registered knob names from the federated config registry SSOT.
fn registered_set() -> BTreeSet<String> {
    vox_config::config_registry::registered_keys()
        .map(|k| k.to_string())
        .collect()
}

/// Path to the registration-backlog baseline, relative to repo root.
const BASELINE_REL_PATH: &str = "contracts/config/config-registry-baseline.txt";

/// Load baseline knob names. Non-empty, non-`#` lines are names. A missing file
/// yields an empty set (gate then fails on every unregistered name).
fn load_baseline(root: &Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let path = root.join(BASELINE_REL_PATH);
    if let Ok(text) = std::fs::read_to_string(&path) {
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            set.insert(line.to_string());
        }
    }
    set
}

/// Run the config-registry-parity gate.
///
/// Every operational `VOX_*` env knob read in code must be a registry row (or
/// covered by a registered prefix row). A baseline ratchet grandfathers today's
/// registration backlog; the gate fails only on NEW unregistered names. With
/// `update_baseline`, regenerate the baseline from the current tree.
pub fn run(update_baseline: bool) -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let used = scan_env_uses(&root);
    let registered = registered_set();

    // Unregistered = used names not covered by any registered exact/prefix row.
    let unregistered: BTreeSet<String> = used
        .iter()
        .filter(|u| !is_registered(u, &registered))
        .cloned()
        .collect();

    if update_baseline {
        let mut out = String::from(
            "# config-registry-parity baseline — registration backlog; burn down to 0. \
             Regenerate: vox ci config-registry-parity --update-baseline\n",
        );
        for name in &unregistered {
            out.push_str(name);
            out.push('\n');
        }
        let path = root.join(BASELINE_REL_PATH);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, out)?;
        println!(
            "config-registry-parity: wrote {} grandfathered name(s) to {}",
            unregistered.len(),
            BASELINE_REL_PATH
        );
        return Ok(());
    }

    // Phantom/declared registry rows: registered but never read in code.
    let (_unreg_vec, registered_unused) = parity(&used, &registered);
    for name in &registered_unused {
        eprintln!(
            "[config-registry-parity] WARN registered but never read in code (phantom/declared): {name}"
        );
    }

    let baseline = load_baseline(&root);
    let news: Vec<&String> = unregistered
        .iter()
        .filter(|u| !baseline.contains(*u))
        .collect();
    let grandfathered = unregistered.len() - news.len();

    if news.is_empty() {
        println!("config-registry-parity OK: {grandfathered} grandfathered, 0 new");
        return Ok(());
    }
    for name in &news {
        eprintln!(
            "[config-registry-parity] {name} is read in code but not registered in CONFIG_KEYS"
        );
    }
    anyhow::bail!(
        "config-registry-parity found {} NEW unregistered env knob(s) ({} grandfathered). \
         Register them in vox_config::config_registry::CONFIG_KEYS, or run \
         `vox ci config-registry-parity --update-baseline` to grandfather.",
        news.len(),
        grandfathered
    )
}

/// Recursive `.rs` walker (copied from config_hygiene): skips `target/` and
/// `*_tests.rs`/`tests.rs` so test-file references don't count.
fn collect_rs_files(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rs_files(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with("_tests.rs") || name == "tests.rs" {
                continue;
            }
            if let Ok(src) = std::fs::read_to_string(&path) {
                f(&path, &src);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_unregistered_and_phantom() {
        let used: BTreeSet<String> = ["VOX_A".into(), "VOX_B".into()].into_iter().collect();
        let reg: BTreeSet<String> = ["VOX_A".into(), "VOX_C".into()].into_iter().collect();
        let (unreg, unused) = parity(&used, &reg);
        assert_eq!(unreg, vec!["VOX_B".to_string()]);
        assert_eq!(unused, vec!["VOX_C".to_string()]);
    }

    #[test]
    fn prefix_row_covers_used_name() {
        let reg: BTreeSet<String> = ["VOX_DB_".into()].into_iter().collect();
        assert!(is_registered("VOX_DB_PATH", &reg));
        assert!(!is_registered("VOX_OTHER", &reg));
    }
}
