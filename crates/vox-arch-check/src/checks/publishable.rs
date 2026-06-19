//! Rule 18 — Publishability closure gate.
//!
//! For every crate marked `publishable = true` in `layers.toml`, every
//! workspace-member crate in its transitive normal-dep closure must also be
//! publishable (i.e. not have `publish = []` in its Cargo.toml).
//!
//! `workspace-hack` is `publish = false` today and is a near-universal dep.
//! Track B resolves this by publishing workspace-hack first as an empty stub.
//! Until then, Rule 18 fires as a warn and is hardened to ERROR in Track B
//! Task B4.

use std::collections::{HashMap, HashSet, VecDeque};

/// Minimal per-crate record needed for the publishability check.
pub struct CrateRec {
    pub name: String,
    /// Normal workspace-member deps (resolved dep names, not aliases).
    pub deps: Vec<String>,
    /// True when the crate has `publish = []` in Cargo.toml.
    pub publish_false: bool,
    /// True when marked `publishable = true` in layers.toml.
    pub publishable: bool,
}

/// Returns pairs `(publishable_crate, unpublishable_dep)` where the dep is in
/// the transitive normal-dep closure of the publishable crate and is
/// `publish_false = true`.
pub fn check(crates: &[CrateRec]) -> Vec<(String, String)> {
    let by_name: HashMap<&str, &CrateRec> = crates.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut violations: Vec<(String, String)> = Vec::new();

    for root in crates.iter().filter(|c| c.publishable) {
        // BFS over the workspace-internal dep closure.
        let mut visited: HashSet<&str> = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(root.name.as_str());
        visited.insert(root.name.as_str());

        while let Some(current) = queue.pop_front() {
            let rec = match by_name.get(current) {
                Some(r) => r,
                None => continue,
            };
            for dep_name in &rec.deps {
                let dep_str = dep_name.as_str();
                if !visited.insert(dep_str) {
                    continue;
                }
                if let Some(dep_rec) = by_name.get(dep_str) {
                    if dep_rec.publish_false {
                        violations.push((root.name.clone(), dep_name.clone()));
                    }
                    queue.push_back(dep_str);
                }
            }
        }
    }

    violations.sort();
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(name: &str, deps: &[&str], publish_false: bool, publishable: bool) -> CrateRec {
        CrateRec {
            name: name.to_string(),
            deps: deps.iter().map(|s| s.to_string()).collect(),
            publish_false,
            publishable,
        }
    }

    #[test]
    fn clean_publishable_crate_passes() {
        let crates = vec![
            rec("vox-crypto", &[], false, true),
        ];
        assert!(check(&crates).is_empty());
    }

    #[test]
    fn direct_unpublishable_dep_is_flagged() {
        let crates = vec![
            rec("vox-secrets", &["workspace-hack"], false, true),
            rec("workspace-hack", &[], true, false),
        ];
        let violations = check(&crates);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0], ("vox-secrets".to_string(), "workspace-hack".to_string()));
    }

    #[test]
    fn transitive_unpublishable_dep_is_flagged() {
        let crates = vec![
            rec("vox-top", &["vox-mid"], false, true),
            rec("vox-mid", &["bad-dep"], false, false),
            rec("bad-dep", &[], true, false),
        ];
        let violations = check(&crates);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].0, "vox-top");
        assert_eq!(violations[0].1, "bad-dep");
    }

    #[test]
    fn non_publishable_crate_with_bad_dep_not_flagged() {
        // A crate without publishable=true is not checked.
        let crates = vec![
            rec("vox-internal", &["bad-dep"], false, false),
            rec("bad-dep", &[], true, false),
        ];
        assert!(check(&crates).is_empty());
    }

    #[test]
    fn multiple_violations_all_reported() {
        let crates = vec![
            rec("pub-crate", &["dep-a", "dep-b"], false, true),
            rec("dep-a", &[], true, false),
            rec("dep-b", &[], true, false),
        ];
        let violations = check(&crates);
        assert_eq!(violations.len(), 2);
    }
}
