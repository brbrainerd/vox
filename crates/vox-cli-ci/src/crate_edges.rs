//! `vox ci crate-edges` — exact edge-set ratchet + layer rule for workspace deps.
//!
//! Reads the LIVE graph from `cargo metadata` (never the regenerated
//! `crate-graph.v1.json` mirror — a mirror regen must not be able to admit edges).
//! Baseline + human-gated exceptions: `contracts/ci/crate-edges.allow.v1.json`.
//! Layer map (downward-only rule): `contracts/ci/crate-layers.v1.json`.
//! Spec: docs/superpowers/specs/2026-07-03-crate-disentanglement-ratchet-design.md

use anyhow::{bail, Context, Result};
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

pub fn write_allow_file(path: &Path, file: &AllowFile) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(file)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

/// Regenerate the baseline from the live set. Removal-only: refuses if any live
/// edge is absent from the prior baseline union exceptions. Keeps exceptions whose
/// edge still exists; excepted pairs are NOT duplicated into edges.
pub fn tighten(root: &Path, live: &BTreeSet<(String, String)>) -> Result<()> {
    let path = root.join(ALLOW_REL);
    let live_real: BTreeSet<(String, String)> = live
        .iter()
        .filter(|(f, t)| f != EXEMPT && t != EXEMPT)
        .cloned()
        .collect();
    let prior: Option<AllowFile> = if path.exists() {
        Some(
            serde_json::from_str(
                &std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?,
            )
            .context("parse crate-edges.allow.v1.json")?,
        )
    } else {
        None
    };
    let mut exceptions: Vec<ExceptionEntry> = Vec::new();
    if let Some(prior) = &prior {
        let prior_allowed: BTreeSet<(String, String)> = prior
            .edges
            .iter()
            .map(|e| (e[0].clone(), e[1].clone()))
            .chain(prior.exceptions.iter().map(|x| (x.from.clone(), x.to.clone())))
            .collect();
        let additions: Vec<&(String, String)> =
            live_real.iter().filter(|e| !prior_allowed.contains(e)).collect();
        if !additions.is_empty() {
            bail!(
                "--tighten would ADD {} edge(s) (tighten is removal-only): {:?}\n\n{HEAL}",
                additions.len(),
                additions
            );
        }
        exceptions = prior
            .exceptions
            .iter()
            .filter(|x| live_real.contains(&(x.from.clone(), x.to.clone())))
            .cloned()
            .collect();
    }
    let excepted: BTreeSet<(String, String)> =
        exceptions.iter().map(|x| (x.from.clone(), x.to.clone())).collect();
    let file = AllowFile {
        schema_version: 1,
        edges: live_real
            .iter()
            .filter(|e| !excepted.contains(e))
            .map(|(f, t)| [f.clone(), t.clone()])
            .collect(),
        exceptions,
    };
    write_allow_file(&path, &file)?;
    println!(
        "crate-edges: baseline tightened to {} edges (+{} exceptions) -> {}",
        file.edges.len(),
        file.exceptions.len(),
        path.display()
    );
    Ok(())
}

/// Bootstrap heuristic only: layer = longest path to an in-tree leaf, capped at 4.
/// Written once when the layers file is absent; hand-adjusted afterwards, never overwritten.
pub fn suggest_layers(live: &BTreeSet<(String, String)>) -> LayersFile {
    let mut adj: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut nodes: BTreeSet<&str> = BTreeSet::new();
    for (f, t) in live {
        if f == EXEMPT || t == EXEMPT {
            continue;
        }
        adj.entry(f.as_str()).or_default().push(t.as_str());
        nodes.insert(f.as_str());
        nodes.insert(t.as_str());
    }
    fn depth<'a>(
        n: &'a str,
        adj: &BTreeMap<&'a str, Vec<&'a str>>,
        memo: &mut BTreeMap<&'a str, u8>,
    ) -> u8 {
        if let Some(&d) = memo.get(n) {
            return d;
        }
        memo.insert(n, 0);
        let d = adj
            .get(n)
            .map(|ds| ds.iter().map(|c| depth(c, adj, memo).saturating_add(1)).max().unwrap_or(0))
            .unwrap_or(0)
            .min(4);
        memo.insert(n, d);
        d
    }
    let mut memo = BTreeMap::new();
    let layers = nodes.iter().map(|n| ((*n).to_string(), depth(n, &adj, &mut memo))).collect();
    LayersFile { schema_version: 1, layers }
}

/// Guard entry point (`vox ci crate-edges [--tighten]`).
pub fn run(root: &Path, tighten_mode: bool) -> Result<()> {
    let live = collect_live_edges(root)?;
    let layers_path = root.join(LAYERS_REL);
    if tighten_mode {
        if !layers_path.exists() {
            let suggested = suggest_layers(&live);
            std::fs::write(&layers_path, serde_json::to_string_pretty(&suggested)? + "\n")?;
            println!(
                "crate-edges: wrote SUGGESTED layer map (hand-adjust before merging!) -> {}",
                layers_path.display()
            );
        }
        return tighten(root, &live);
    }
    let allow_path = root.join(ALLOW_REL);
    if !allow_path.exists() {
        bail!("missing {ALLOW_REL} — bootstrap with `vox ci crate-edges --tighten`");
    }
    let allow: AllowFile = serde_json::from_str(&std::fs::read_to_string(&allow_path)?)
        .context("parse crate-edges.allow.v1.json")?;
    if !layers_path.exists() {
        bail!("missing {LAYERS_REL} — bootstrap with `vox ci crate-edges --tighten`");
    }
    let layers: LayersFile = serde_json::from_str(&std::fs::read_to_string(&layers_path)?)
        .context("parse crate-layers.v1.json")?;

    let (violations, stale) = check(&live, &allow, &layers);
    for (f, t) in &stale {
        println!("warning: stale baseline edge {f} -> {t} (gone; run `vox ci crate-edges --tighten`)");
    }
    if violations.is_empty() {
        println!("crate-edges: OK ({} live in-tree edges within baseline)", live.len());
        return Ok(());
    }
    for v in &violations {
        match v {
            Violation::NewEdge { from, to } => eprintln!("NEW EDGE not in baseline: {from} -> {to}"),
            Violation::UpwardEdge { from, from_layer, to, to_layer } => eprintln!(
                "UPWARD EDGE (layer rule): {from} (L{from_layer}) -> {to} (L{to_layer}) — deps must point same-layer or down"
            ),
            Violation::MissingLayer { krate } => eprintln!(
                "UNASSIGNED LAYER: {krate} missing from {LAYERS_REL} — assign one per where-things-live.md"
            ),
        }
    }
    bail!("crate-edges: {} violation(s)\n\n{HEAL}", violations.len());
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

    #[test]
    fn tighten_is_removal_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("contracts/ci")).unwrap();
        write_allow_file(
            &dir.path().join(ALLOW_REL),
            &allow(&[("app", "lib")], &[]),
        )
        .unwrap();
        let err = tighten(dir.path(), &edges(&[("app", "lib"), ("app", "extra")])).unwrap_err();
        assert!(err.to_string().contains("removal-only"), "{err}");
        tighten(dir.path(), &edges(&[])).unwrap();
        let f: AllowFile = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(ALLOW_REL)).unwrap(),
        )
        .unwrap();
        assert!(f.edges.is_empty());
    }

    #[test]
    fn tighten_bootstraps_when_missing_and_keeps_live_exceptions() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("contracts/ci")).unwrap();
        tighten(dir.path(), &edges(&[("app", "lib")])).unwrap();
        let f: AllowFile = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(ALLOW_REL)).unwrap(),
        )
        .unwrap();
        assert_eq!(f.edges, vec![["app".to_string(), "lib".to_string()]]);
        write_allow_file(
            &dir.path().join(ALLOW_REL),
            &allow(&[("app", "lib")], &[("app", "lib")]),
        )
        .unwrap();
        tighten(dir.path(), &edges(&[("app", "lib")])).unwrap();
        let f: AllowFile = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(ALLOW_REL)).unwrap(),
        )
        .unwrap();
        assert!(f.edges.is_empty(), "excepted pair must not also sit in edges");
        assert_eq!(f.exceptions.len(), 1);
    }

    #[test]
    fn suggest_layers_depth_capped() {
        let l = suggest_layers(&edges(&[
            ("a", "b"), ("b", "c"), ("c", "d"), ("d", "e"), ("e", "f"),
        ]));
        assert_eq!(l.layers["f"], 0);
        assert_eq!(l.layers["e"], 1);
        assert_eq!(l.layers["a"], 4);
    }
}
