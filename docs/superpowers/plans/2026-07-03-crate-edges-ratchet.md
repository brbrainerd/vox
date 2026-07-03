# Crate-Edges Ratchet Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land `vox ci crate-edges` — an exact edge-set ratchet + layer rule over the workspace dependency graph, with a human-gated exceptions ledger, AGENTS.md rules for LLM readers, and blocking CI wiring.

**Architecture:** A new guard module in `vox-cli-ci` (where all guards live) reads the live graph from `cargo metadata` (NOT the regenerated `crate-graph.v1.json` mirror, so mirror regen can't admit edges), compares it against `contracts/ci/crate-edges.allow.v1.json` (frozen baseline + user-authorized-only exceptions) and `contracts/ci/crate-layers.v1.json` (downward-only layer rule). `--tighten` regenerates removal-only. Spec: `docs/superpowers/specs/2026-07-03-crate-disentanglement-ratchet-design.md`.

**Tech Stack:** Rust; `cargo_metadata` 0.23 (workspace dep exists); serde/serde_json; the vox-cli-ci guard idiom (`pub fn run(root, …) -> Result<()>`, `CiCmd` variant, `run_body.rs` dispatch arm, ci.yml step).

**Toolchain notes for the engineer (repo-specific, non-obvious):**
- Always invoke cargo as `env -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER CARGO_BUILD_RUSTC_WRAPPER="" cargo …` (bypasses the build-broker shim). Abbreviated below as `CARGO`.
- Never `cargo fmt --all` on Windows (arg-limit overflow); `cargo fmt -p vox-cli-ci` is fine.
- Commit trailer: `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- Work in an isolated worktree off `origin/main`; other sessions push to main frequently — rebase before push.

---

## File map

| File | Role |
|---|---|
| Create `crates/vox-cli-ci/src/crate_edges.rs` | The guard: schema structs, pure `check`, `collect_live_edges`, `tighten`, `suggest_layers`, `run`, heal text, unit tests |
| Modify `crates/vox-cli-ci/src/lib.rs` | `pub mod crate_edges;` |
| Modify `crates/vox-cli-ci/Cargo.toml` | add `cargo_metadata = { workspace = true }` |
| Modify `crates/vox-cli-ci/src/cmd_enums.rs` | `CiCmd::CrateEdges { tighten }` variant (do NOT touch `gate_policy_id` — untracked gates record "honest grey" by design) |
| Modify `crates/vox-cli/src/commands/ci/run_body.rs:592` area | dispatch arm next to `FanInBudget` |
| Create `contracts/ci/crate-edges.allow.v1.json` | bootstrap via `--tighten` (Task 5) |
| Create `contracts/ci/crate-layers.v1.json` | bootstrap via `--tighten`, then hand-adjust (Task 5) |
| Modify `AGENTS.md` | new `## Dependency Discipline (Required, SSOT)` section before `## Perennial Bug Patterns` |
| Modify `.github/workflows/ci.yml` | step directly after the `fan-in-budget` step (line ~956) |

---

### Task 1: Schema structs + pure check logic (TDD)

**Files:**
- Create: `crates/vox-cli-ci/src/crate_edges.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (add `pub mod crate_edges;` in the alphabetical `pub mod` block)

- [ ] **Step 1: Create the module with types + a stub `check` + failing tests**

```rust
//! `vox ci crate-edges` — exact edge-set ratchet + layer rule for workspace deps.
//!
//! Reads the LIVE graph from `cargo metadata` (never the regenerated
//! `crate-graph.v1.json` mirror — a mirror regen must not be able to admit edges).
//! Baseline + human-gated exceptions: `contracts/ci/crate-edges.allow.v1.json`.
//! Layer map (downward-only rule): `contracts/ci/crate-layers.v1.json`.
//! Spec: docs/superpowers/specs/2026-07-03-crate-disentanglement-ratchet-design.md

use anyhow::{Context, Result, bail};
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
    /// Frozen baseline, sorted `[from, to]` pairs. Machine-tightened only.
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
    /// crate name -> layer (0 = leaf foundation … 4 = apps/shells).
    pub layers: BTreeMap<String, u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Violation {
    NewEdge { from: String, to: String },
    UpwardEdge { from: String, from_layer: u8, to: String, to_layer: u8 },
    MissingLayer { krate: String },
}

/// Pure rule engine: violations + stale-baseline warnings. No IO.
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
        if !baseline.contains(&pair) && !excepted.contains(&pair) {
            violations.push(Violation::NewEdge { from: from.clone(), to: to.clone() });
        }
        if excepted.contains(&pair) {
            continue; // grandfathered: exempt from the layer rule too
        }
        match (layers.layers.get(from), layers.layers.get(to)) {
            (Some(&fl), Some(&tl)) => {
                if fl < tl {
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
    let stale: Vec<(String, String)> = baseline.into_iter().filter(|e| !live.contains(e)).collect();
    (violations, stale)
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
    fn exception_admits_edge_and_skips_layer_rule() {
        // upward edge (lib L0 -> app L4) but grandfathered via ledger
        let (v, _) = check(
            &edges(&[("lib", "app")]),
            &allow(&[], &[("lib", "app")]),
            &layers(&[("app", 4), ("lib", 0)]),
        );
        assert!(v.is_empty());
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
}
```

- [ ] **Step 2: Register the module and run the tests**

Add `pub mod crate_edges;` to `crates/vox-cli-ci/src/lib.rs` (in the existing `pub mod` block, after `pub mod coverage_gates;`).

Run: `CARGO test -p vox-cli-ci crate_edges`
Expected: **8 passed**.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli-ci/src/crate_edges.rs crates/vox-cli-ci/src/lib.rs
git commit -m "feat(vox-cli-ci): crate-edges rule engine (edge ratchet + layer rule, pure)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Live-graph collection via cargo metadata

**Files:**
- Modify: `crates/vox-cli-ci/Cargo.toml` (add dep, after the `chrono` line: `cargo_metadata = { workspace = true }`)
- Modify: `crates/vox-cli-ci/src/crate_edges.rs` (append below `check`)

- [ ] **Step 1: Add the dependency and the collector**

```rust
/// Live in-tree edge set from `cargo metadata`: workspace members only,
/// normal + build dependency kinds (dev-deps excluded in v1 — they don't ship
/// in the binary closure; `vox ci dep-cycles` covers dev-dep back-edges).
pub fn collect_live_edges(root: &Path) -> Result<BTreeSet<(String, String)>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .exec()
        .context("run `cargo metadata`")?;
    let members: BTreeSet<String> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| metadata.packages.iter().find(|p| &p.id == id))
        .map(|p| p.name.to_string())
        .collect();
    let mut edges = BTreeSet::new();
    for pkg in &metadata.packages {
        if !members.contains(pkg.name.as_str()) {
            continue;
        }
        for dep in &pkg.dependencies {
            use cargo_metadata::DependencyKind;
            if !matches!(dep.kind, DependencyKind::Normal | DependencyKind::Build) {
                continue;
            }
            if members.contains(dep.name.as_str()) {
                edges.insert((pkg.name.to_string(), dep.name.clone()));
            }
        }
    }
    Ok(edges)
}
```

(If `pkg.name.as_str()`/`to_string()` friction arises — cargo_metadata 0.23 wraps the
name in a newtype — follow the compiler; the intent is the package name as `String`.)

- [ ] **Step 2: Integration test proving it sees the real workspace**

Append to the `tests` module:

```rust
    /// Integration: real workspace. vox-cli depends on vox-cli-ci (PR-4 split);
    /// if this fails the collector or the workspace layout changed.
    #[test]
    fn live_graph_contains_known_edge() {
        let root = crate::repo_root();
        let live = collect_live_edges(&root).expect("cargo metadata");
        assert!(live.contains(&("vox-cli".to_string(), "vox-cli-ci".to_string())));
        assert!(live.len() > 400, "expected hundreds of in-tree edges, got {}", live.len());
    }
```

Run: `CARGO test -p vox-cli-ci crate_edges`
Expected: **9 passed** (the integration test takes a few seconds — cargo metadata).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli-ci/Cargo.toml crates/vox-cli-ci/src/crate_edges.rs
git commit -m "feat(vox-cli-ci): crate-edges live-graph collector (cargo metadata, normal+build deps)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: tighten + suggest_layers + run orchestration

**Files:**
- Modify: `crates/vox-cli-ci/src/crate_edges.rs` (append)

- [ ] **Step 1: Write failing tests for tighten semantics**

Append to the `tests` module (uses `tempfile`, already a dev-dep of vox-cli-ci):

```rust
    #[test]
    fn tighten_is_removal_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("contracts/ci")).unwrap();
        // Prior baseline: one edge.
        write_allow_file(
            &dir.path().join(ALLOW_REL),
            &allow(&[("app", "lib")], &[]),
        )
        .unwrap();
        // Live has a NEW edge -> tighten must refuse.
        let err = tighten(dir.path(), &edges(&[("app", "lib"), ("app", "extra")])).unwrap_err();
        assert!(err.to_string().contains("removal-only"), "{err}");
        // Live shrank -> tighten succeeds and the file now has 0 edges.
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
        // Bootstrap: no prior file -> writes live set.
        tighten(dir.path(), &edges(&[("app", "lib")])).unwrap();
        let f: AllowFile = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join(ALLOW_REL)).unwrap(),
        )
        .unwrap();
        assert_eq!(f.edges, vec![["app".to_string(), "lib".to_string()]]);
        // Add an exception for a live edge; a later tighten keeps it and
        // EXCLUDES the excepted pair from `edges` (no duplication).
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
        // chain a->b->c->d->e->f : f is a leaf (0), a caps at 4
        let l = suggest_layers(&edges(&[
            ("a", "b"), ("b", "c"), ("c", "d"), ("d", "e"), ("e", "f"),
        ]));
        assert_eq!(l.layers["f"], 0);
        assert_eq!(l.layers["e"], 1);
        assert_eq!(l.layers["a"], 4);
    }
```

Run: `CARGO test -p vox-cli-ci crate_edges`
Expected: FAIL — `tighten`, `write_allow_file`, `suggest_layers` not defined.

- [ ] **Step 2: Implement**

Append above the `tests` module:

```rust
pub fn write_allow_file(path: &Path, file: &AllowFile) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(file)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

/// Regenerate the baseline from the live set. Removal-only: refuses if any live
/// edge is absent from the prior baseline ∪ exceptions. Keeps exceptions whose
/// edge still exists; excepted pairs are NOT duplicated into `edges`.
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
        memo.insert(n, 0); // provisional; normal-dep graph is acyclic
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
```

- [ ] **Step 3: Run all module tests**

Run: `CARGO test -p vox-cli-ci crate_edges`
Expected: **12 passed**.

- [ ] **Step 4: Clippy + commit**

Run: `CARGO clippy -p vox-cli-ci --lib` — expected 0 warnings (fix any).

```bash
git add crates/vox-cli-ci/src/crate_edges.rs
git commit -m "feat(vox-cli-ci): crate-edges tighten/bootstrap/run orchestration with LLM heal text" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: CLI wiring (variant + dispatch arm)

**Files:**
- Modify: `crates/vox-cli-ci/src/cmd_enums.rs` (insert the variant directly after the `FanInBudget` variant block, ~line 816)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (insert the arm directly after the `FanInBudget` arm, ~line 594)

- [ ] **Step 1: Add the `CiCmd` variant**

```rust
    /// Exact edge-set ratchet + layer rule for workspace crate dependencies.
    /// Live graph from `cargo metadata` vs `contracts/ci/crate-edges.allow.v1.json`
    /// (+ `crate-layers.v1.json`). New edges require a user-authorized ledger entry.
    #[command(name = "crate-edges")]
    CrateEdges {
        /// Regenerate the baseline from the live graph (removal-only) and drop
        /// stale exceptions. Bootstraps both contract files when missing.
        #[arg(long)]
        tighten: bool,
    },
```

Do NOT add a `gate_policy_id` entry — untracked gates record "honest grey" per the
`run_body` gate-status comments, and the id registry parity test only checks `Some` ids.

- [ ] **Step 2: Add the dispatch arm**

```rust
        CiCmd::CrateEdges { tighten } => vox_cli_ci::crate_edges::run(&root, tighten),
```

- [ ] **Step 3: Build both crates + clap goldens**

Run: `CARGO build -p vox-cli-ci -p vox-cli`
Expected: exit 0.

Run: `CARGO test -p vox-cli --test ci_workflow_contract --test vox_cli_root_parsing`
Expected: same pass/fail set as before this change (note: `cross_platform_gate_is_required_three_os_matrix` fails PRE-EXISTING on main — unrelated; every other test passes).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli-ci/src/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs
git commit -m "feat(vox-cli): wire vox ci crate-edges (variant + dispatch)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Bootstrap the contract files (baseline + layers)

**Files:**
- Create: `contracts/ci/crate-edges.allow.v1.json` (generated)
- Create: `contracts/ci/crate-layers.v1.json` (generated, then hand-adjusted)

- [ ] **Step 1: Bootstrap both files**

Run: `CARGO run -p vox-cli --bin vox -- ci crate-edges --tighten`
Expected: prints the suggested-layers line and `baseline tightened to ~640 edges (+0 exceptions)`. (If the freshness gate blocks a stale binary, this exact `cargo run` invocation IS the fresh binary.)

- [ ] **Step 2: Hand-adjust the seed layers**

Edit `contracts/ci/crate-layers.v1.json`, forcing these assignments (heuristic values
stay for unlisted crates):

- L4: `vox-cli`, `vox-gui`, `vox-ml-cli`, `vox-orchestrator-mcp`, `vox-integration-tests`
- L3: `vox-orchestrator`, `vox-actor-runtime`, `vox-plugin-host`, `vox-search`, `vox-cli-ci`
- L2: `vox-db`, `vox-compiler`, `vox-codegen`, `vox-corpus`, `vox-publisher`, `vox-populi`
- L1: `vox-config`, `vox-secrets`, `vox-http-client`, `vox-telemetry`, `vox-repository`, `vox-git`, `vox-cli-core`, `vox-cli-contracts`
- L0: `vox-foundation`, `vox-bounded-fs`, `vox-crypto`, `vox-db-types`, `vox-plugin-types`, `vox-orchestrator-types`, `vox-mesh-types`

- [ ] **Step 3: Check, then grandfather any upward edges**

Run: `CARGO run -p vox-cli --bin vox -- ci crate-edges`

If UPWARD EDGE violations print (expected: a handful — e.g. `vox-config(L1)→vox-llm-egress`
if the heuristic put llm-egress higher): move each printed pair from `edges` into
`exceptions` with exactly:

```json
{ "from": "<from>", "to": "<to>",
  "reason": "grandfathered at 2026-07-03 bootstrap (pre-existing upward edge; Phase 2 decoupling target)",
  "date": "2026-07-03",
  "authorized_by": "brbrainerd (bootstrap grandfather, plan-approved)" }
```

(This one-time grandfather set was authorized by the user approving this plan. After
this, exceptions are strictly user-authorized per entry.)

Re-run the check until: `crate-edges: OK`.

- [ ] **Step 4: Commit the contracts**

```bash
git add contracts/ci/crate-edges.allow.v1.json contracts/ci/crate-layers.v1.json
git commit -m "feat(contracts): crate-edges baseline (~643 edges) + layer map, upward edges grandfathered" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: AGENTS.md Dependency Discipline section

**Files:**
- Modify: `AGENTS.md` — insert immediately BEFORE the `## Perennial Bug Patterns (catch early)` heading

- [ ] **Step 1: Insert the section**

```markdown
## Dependency Discipline (Required, SSOT)

Workspace crate-dependency edges are CI-gated by `vox ci crate-edges` (exact edge-set
ratchet + downward-only layer rule; contracts: `contracts/ci/crate-edges.allow.v1.json`,
`contracts/ci/crate-layers.v1.json`).

1. **Before adding a dep on another workspace crate:** prefer a narrower `-types`/`-core`
   crate; or apply the defactor policy (rule 3). If the edge is genuinely needed,
   PROPOSE an `exceptions` ledger entry in your PR description and stop.
2. **`exceptions` entries are USER-AUTHORIZED-ONLY.** Never write one yourself, and
   never regenerate a baseline (`crate-edges.allow.v1.json`, `fan-in-snapshot.v1.json`)
   to admit an edge you introduced. Tightening (`vox ci crate-edges --tighten`) is
   always allowed.
3. **Defactor policy:** a helper under ~50 lines may be duplicated into the consumer
   with a `// vox:defactored-from <crate> <date>` comment instead of taking a crate
   edge. Larger shared surfaces get split into `-types`/`-core` crates. Never fork
   100+ line chunks.
4. **New crates** must be assigned a layer in `contracts/ci/crate-layers.v1.json` at
   creation (L0 leaf foundation … L4 apps/shells; see
   `docs/src/architecture/where-things-live.md`). Dependencies point same-layer or down.
```

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs(AGENTS): dependency-discipline rules (edge ratchet, human-gated ledger, defactor policy)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: CI wiring + end-to-end verification

**Files:**
- Modify: `.github/workflows/ci.yml` — insert directly after the fan-in-budget step (`run: ./target/debug/vox --quiet ci fan-in-budget`, ~line 956), copying that step's exact structure (same indentation, same `if:`/`name:` shape as its siblings):

- [ ] **Step 1: Add the CI step**

```yaml
      - name: Crate edge ratchet + layer rule
        run: ./target/debug/vox --quiet ci crate-edges
```

- [ ] **Step 2: Full local verification**

```bash
CARGO test -p vox-cli-ci crate_edges          # 12 passed
CARGO clippy -p vox-cli-ci --lib               # 0 warnings
CARGO run -p vox-cli --bin vox -- ci crate-edges   # crate-edges: OK
```

- [ ] **Step 3: Prove the ratchet bites (manual negative test, then revert)**

Temporarily add `vox-gamify = { workspace = true }` to `crates/vox-bounded-fs/Cargo.toml`
(an absurd upward edge), run `CARGO run -p vox-cli --bin vox -- ci crate-edges`, and
confirm it fails with BOTH `NEW EDGE` and `UPWARD EDGE` plus the heal text. Then
`git checkout -- crates/vox-bounded-fs/Cargo.toml` and re-run to green.

- [ ] **Step 4: Commit + push**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: wire vox ci crate-edges as a blocking gate" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git fetch origin && git rebase origin/main
# If the rebase pulls in new crates/edges, re-run: vox ci crate-edges --tighten … wait —
# tighten cannot ADD. If a new edge arrived on main, re-bootstrap the baseline:
# delete contracts/ci/crate-edges.allow.v1.json, re-run --tighten (preserving the
# exceptions by restoring them from the deleted file), re-verify, amend the Task 5 commit.
git push origin HEAD:main
```

---

## Self-review notes (done at authoring)

- **Spec coverage:** §3.1 guard (Tasks 1–3), §3.2 allow file (Tasks 3, 5), §3.3 layers +
  rule + grandfathering (Tasks 1, 3, 5), §3.4 AGENTS.md (Task 6, text kept in sync with
  `HEAL`), §3.5 disposition (no code change needed — fan-in-budget untouched;
  dep-backedges fold-in deferred to the retirement cleanup, noted in spec), §6 testing
  (Tasks 1–3 unit, Task 2 integration, Task 7 CI + negative test), §7 baseline-churn
  risk (Task 7 Step 4 re-bootstrap procedure).
- **Placeholders:** none; all code complete.
- **Type consistency:** `check(live, allow, layers) -> (Vec<Violation>, Vec<(String,String)>)`,
  `tighten(root, live)`, `run(root, tighten_mode)` used consistently across tasks.
