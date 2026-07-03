//! `vox ci crate-edges` — exact edge-set ratchet + layer rule for workspace deps.
//!
//! Reads the LIVE graph from `cargo metadata` (never the regenerated
//! `crate-graph.v1.json` mirror — a mirror regen must not be able to admit edges).
//! Baseline + human-gated exceptions: `contracts/ci/crate-edges.allow.v1.json`.
//! Layer map (downward-only rule): `contracts/ci/crate-layers.v1.json`.
//! Spec: docs/superpowers/specs/2026-07-03-crate-disentanglement-ratchet-design.md

use anyhow::{Context, Result};
use cargo_metadata::DependencyKind;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const ALLOW_REL: &str = "contracts/ci/crate-edges.allow.v1.json";
pub const LAYERS_REL: &str = "contracts/ci/crate-layers.v1.json";
/// hakari feature-unification crate: exempt both directions, by design.
pub const EXEMPT: &str = "workspace-hack";

/// LLM-facing heal text. AGENTS.md "Dependency Discipline" states the same rules;
/// keep both short and in sync.
pub const HEAL: &str = "\
[diag id=arch/crate-edges heal=llm]
A workspace dependency edge is not in the committed baseline. Legal moves:
 1. Do not add the edge: check for a narrower `-types`/`-core` crate, or duplicate a
    helper under ~50 lines into your crate with a `// vox:defactored-from <crate> <date>`
    comment (see AGENTS.md 'Dependency Discipline').
 2. If the edge is genuinely needed: PROPOSE an `exceptions` entry in your PR
    description and STOP. Entries in contracts/ci/crate-edges.allow.v1.json are
    USER-AUTHORIZED-ONLY. Never write one yourself; never regenerate baselines to
    admit your own edge.
Tightening (removing edges) is always allowed: `vox ci crate-edges --tighten`.";

#[derive(Debug, Serialize, Deserialize)]
pub struct AllowFile {
    pub schema_version: u32,
    /// Frozen baseline, sorted [from, to] pairs. Machine-tightened only.
    pub edges: Vec<[String; 2]>,
    /// Human-gated ledger. Append requires explicit user authorization.
    pub exceptions: Vec<ExceptionEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceptionEntry {
    pub from: String,
    pub to: String,
    pub reason: String,
    pub date: String,
    pub authorized_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LayersFile {
    pub schema_version: u32,
    /// crate name -> layer (0 = leaf foundation ... 4 = apps/shells).
    pub layers: BTreeMap<String, u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Violation {
    NewEdge { from: String, to: String },
    UpwardEdge { from: String, from_layer: u8, to: String, to_layer: u8 },
    MissingLayer { krate: String },
}

/// Pure rule engine: violations + stale-baseline warnings. No IO.
///
/// IMPORTANT: an exception suppresses the NewEdge/UpwardEdge verdicts for its
/// own pair, but layer-presence checking is UNCONDITIONAL — an excepted edge must
/// never let a crate skip classification (a new crate sneaking in via an exception
/// with no layer entry is exactly the kind of gap this rule exists to prevent).
pub fn check(
    live: &BTreeSet<(String, String)>,
    allow: &AllowFile,
    layers: &LayersFile,
) -> (Vec<Violation>, Vec<(String, String)>) {
    let baseline: BTreeSet<(String, String)> = allow
        .edges
        .iter()
        .map(|e| (e[0].clone(), e[1].clone()))
        .collect();
    let excepted: BTreeSet<(String, String)> = allow
        .exceptions
        .iter()
        .map(|x| (x.from.clone(), x.to.clone()))
        .collect();

    let mut violations = Vec::new();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for (from, to) in live {
        if from == EXEMPT || to == EXEMPT {
            continue;
        }
        let pair = (from.clone(), to.clone());
        let is_excepted = excepted.contains(&pair);
        if !baseline.contains(&pair) && !is_excepted {
            violations.push(Violation::NewEdge { from: from.clone(), to: to.clone() });
        }
        // Layer-presence + upward-edge checks always run, regardless of exception
        // status; only the UpwardEdge verdict itself is suppressed when excepted.
        match (layers.layers.get(from), layers.layers.get(to)) {
            (Some(&fl), Some(&tl)) => {
                if fl < tl && !is_excepted {
                    violations.push(Violation::UpwardEdge {
                        from: from.clone(),
                        from_layer: fl,
                        to: to.clone(),
                        to_layer: tl,
                    });
                }
            }
            (f, t) => {
                if f.is_none() {
                    missing.insert(from.clone());
                }
                if t.is_none() {
                    missing.insert(to.clone());
                }
            }
        }
    }
    for krate in missing {
        violations.push(Violation::MissingLayer { krate });
    }
    // Stale covers BOTH the frozen baseline and the exceptions ledger — a dead
    // exception (its edge no longer exists) must also prompt cleanup via --tighten.
    let stale: Vec<(String, String)> = baseline
        .iter()
        .chain(excepted.iter())
        .filter(|e| !live.contains(*e))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    (violations, stale)
}

/// Live in-tree edge set from `cargo metadata`: workspace members only,
/// normal + build dependency kinds (dev-deps excluded in v1 — they don't ship
/// in the binary closure; `vox ci dep-cycles` covers dev-dep back-edges).
pub fn collect_live_edges(root: &Path) -> Result<BTreeSet<(String, String)>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()
        .context("run `cargo metadata`")?;
    let members: BTreeSet<&str> =
        metadata.workspace_packages().iter().map(|p| p.name.as_str()).collect();
    let mut edges = BTreeSet::new();
    for pkg in metadata.workspace_packages() {
        for dep in &pkg.dependencies {
            if !matches!(dep.kind, DependencyKind::Normal | DependencyKind::Build) {
                continue;
            }
            if members.contains(dep.name.as_str()) {
                edges.insert((pkg.name.as_str().to_string(), dep.name.clone()));
            }
        }
    }
    Ok(edges)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edges(pairs: &[(&str, &str)]) -> BTreeSet<(String, String)> {
        pairs.iter().map(|(f, t)| (f.to_string(), t.to_string())).collect()
    }
    fn allow(baseline: &[(&str, &str)], exceptions: &[(&str, &str)]) -> AllowFile {
        AllowFile {
            schema_version: 1,
            edges: baseline.iter().map(|(f, t)| [f.to_string(), t.to_string()]).collect(),
            exceptions: exceptions
                .iter()
                .map(|(f, t)| ExceptionEntry {
                    from: f.to_string(),
                    to: t.to_string(),
                    reason: "test".into(),
                    date: "2026-07-03".into(),
                    authorized_by: "test".into(),
                })
                .collect(),
        }
    }
    fn layers(assign: &[(&str, u8)]) -> LayersFile {
        LayersFile {
            schema_version: 1,
            layers: assign.iter().map(|(k, l)| (k.to_string(), *l)).collect(),
        }
    }

    #[test]
    fn new_edge_fails() {
        let (v, _) = check(
            &edges(&[("app", "lib")]),
            &allow(&[], &[]),
            &layers(&[("app", 4), ("lib", 0)]),
        );
        assert_eq!(v, vec![Violation::NewEdge { from: "app".into(), to: "lib".into() }]);
    }

    #[test]
    fn baseline_edge_passes() {
        let (v, stale) = check(
            &edges(&[("app", "lib")]),
            &allow(&[("app", "lib")], &[]),
            &layers(&[("app", 4), ("lib", 0)]),
        );
        assert!(v.is_empty());
        assert!(stale.is_empty());
    }

    #[test]
    fn exception_admits_edge_and_suppresses_upward_verdict() {
        let (v, _) = check(
            &edges(&[("lib", "app")]),
            &allow(&[], &[("lib", "app")]),
            &layers(&[("app", 4), ("lib", 0)]),
        );
        assert!(v.is_empty());
    }

    #[test]
    fn exception_does_not_mask_missing_layer() {
        let (v, _) = check(
            &edges(&[("app", "new")]),
            &allow(&[], &[("app", "new")]),
            &layers(&[("app", 4)]),
        );
        assert_eq!(v, vec![Violation::MissingLayer { krate: "new".into() }]);
    }

    #[test]
    fn removed_edge_reports_stale_not_fail() {
        let (v, stale) = check(
            &edges(&[]),
            &allow(&[("app", "lib")], &[]),
            &layers(&[]),
        );
        assert!(v.is_empty());
        assert_eq!(stale, vec![("app".to_string(), "lib".to_string())]);
    }

    #[test]
    fn dead_exception_reports_stale_too() {
        let (v, stale) = check(
            &edges(&[]),
            &allow(&[], &[("app", "lib")]),
            &layers(&[]),
        );
        assert!(v.is_empty());
        assert_eq!(stale, vec![("app".to_string(), "lib".to_string())]);
    }

    #[test]
    fn upward_layer_edge_fails() {
        let (v, _) = check(
            &edges(&[("lib", "app")]),
            &allow(&[("lib", "app")], &[]),
            &layers(&[("app", 4), ("lib", 0)]),
        );
        assert_eq!(
            v,
            vec![Violation::UpwardEdge { from: "lib".into(), from_layer: 0, to: "app".into(), to_layer: 4 }]
        );
    }

    #[test]
    fn same_layer_edge_ok() {
        let (v, _) = check(
            &edges(&[("db", "compiler")]),
            &allow(&[("db", "compiler")], &[]),
            &layers(&[("db", 2), ("compiler", 2)]),
        );
        assert!(v.is_empty());
    }

    #[test]
    fn missing_layer_fails() {
        let (v, _) = check(
            &edges(&[("app", "lib")]),
            &allow(&[("app", "lib")], &[]),
            &layers(&[("app", 4)]),
        );
        assert_eq!(v, vec![Violation::MissingLayer { krate: "lib".into() }]);
    }

    #[test]
    fn workspace_hack_exempt() {
        let (v, _) = check(
            &edges(&[("app", "workspace-hack"), ("workspace-hack", "lib")]),
            &allow(&[], &[]),
            &layers(&[]),
        );
        assert!(v.is_empty());
    }

    /// Integration: real workspace. vox-cli depends on vox-cli-ci (already true on
    /// main after the PR-4 CI extraction); if this fails the collector or the
    /// workspace layout changed.
    #[test]
    fn live_graph_contains_known_edge() {
        let root = crate::repo_root();
        let live = collect_live_edges(&root).expect("cargo metadata");
        assert!(live.contains(&("vox-cli".to_string(), "vox-cli-ci".to_string())));
        assert!(live.len() > 400, "expected hundreds of in-tree edges, got {}", live.len());
    }
}
