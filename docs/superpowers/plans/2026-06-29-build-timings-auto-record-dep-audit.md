# Build-Timings Auto-Record + Dep-Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every future build automatically record per-crate timing data, and provide a one-shot `vox ci dep-audit` report covering blast-radius, critical-path membership, and dep counts for every workspace crate.

**Architecture:** Four independently shippable components:
- **A. `--ingest` flag on `vox ci build-bench`** — parses the newest `target/cargo-timings/cargo-timing-*.html`, extracts the embedded `UNIT_DATA` JSON (per-crate wall-clock), and appends to `contracts/ci/build-timings-history.v1.jsonl`. No-op when mode is `todo` (sccache hit, meaning `duration: 0`).
- **B. `vox ci dep-audit`** — new subcommand: shells `cargo metadata`, computes `{workspace_dep_count, workspace_rdep_count, external_dep_count, on_vox_cli_critical_path}` per workspace crate, writes `contracts/ci/dep-audit.v1.json`. Uses the existing metadata-parsing pattern in `build_timings.rs`.
- **C. CI wiring** — adds `--timings` to the `cargo build -p vox-cli` step in `.github/workflows/ci.yml` (Check, Build, and Test job), then calls `vox ci build-bench --ingest` after it completes. Uploads the HTML artifact.
- **D. Baseline population** — runs `vox ci build-bench --repeat 3 --write contracts/ci/build-bench-baseline.v1.json` in a cold-build CI job (sccache disabled for this step only) to fill the currently all-zero baseline.

**Tech Stack:** Rust (vox-cli ci subcommand surface, `serde_json`, `regex`/string parsing for HTML extraction), `cargo metadata`, GitHub Actions.

**Safety:** Never `cargo fmt --all`. Never `--no-verify` unless pre-push hook false-positives on alias. Use `cargo fmt -p vox-cli`. All changes go via PR.

---

## Verified facts (do not re-derive)

| Fact | Evidence |
|---|---|
| `vox ci build-bench` exists at `crates/vox-cli/src/commands/ci/build_bench.rs` | Read |
| Baseline is all `wall_ms: 0` — never measured | `contracts/ci/build-bench-baseline.v1.json` read |
| Cargo `--timings` HTML embeds **`const UNIT_DATA = [...]`** JS array in a `<script>` block | Verified 2026-06-29 against real `cargo-timing-*.html` (NOT `var` — the original draft was wrong) |
| Each UNIT_DATA entry is `{i, name, version, mode, target, duration, start, ...}`; **`duration` is FLOAT SECONDS** (e.g. `9.37`), not integer ms | Verified 2026-06-29. Parse with `as_f64()`, convert `*1000.0`. `as_u64()` returns `None` on floats — do NOT use it. |
| Filter on `duration > 0`, **NOT on `mode`** | Verified 2026-06-29: in a cached/sccache build the real compile rows carry `mode:"todo"` WITH nonzero duration (`tauri` 9.37s). A `mode=="todo"` skip discards exactly the data we want. The earlier "todo = sccache hit, duration 0" claim was FALSE. |
| `dep_graph_fingerprint()` in `build_timings.rs:65` already shells `cargo metadata` correctly | Read |
| Subcommand dispatch pattern: `CiCmd::BuildBench { ... }` → `run_build_bench(root, ...)` in `run_body.rs` | Read |
| `cargo_bin()` is `pub(super)` in `ci/mod.rs` | Read |
| `vox ci dep-cycles` exists at `dep_cycles.rs` and already parses workspace edges | Confirmed |
| vox-cli has 86 workspace transitive deps; vox-config has 39 rdeps (highest blast-radius leaf) | Research from prev session |
| 34 workspace crates are independent of vox-cli (changes don't trigger vox-cli rebuild) | Research from prev session |

---

## Component A — `build-bench --ingest` flag

### Task A-1: Add `--ingest` flag to `BuildBench` CLI variant

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add `ingest: bool` to `BuildBench` variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (pass `ingest` through to `run_build_bench`)
- Modify: `crates/vox-cli/src/commands/ci/build_bench.rs` (add `ingest` param to `run_build_bench`, add `ingest_timings_html` function)

- [ ] **Step 1: Find the `BuildBench` variant in `cmd_enums.rs`**

Run:
```
grep -n "BuildBench" crates/vox-cli/src/commands/ci/cmd_enums.rs | head -10
```
Note the exact struct fields to know what to add `ingest: bool` next to.

- [ ] **Step 2: Write the failing test for HTML parsing**

In `crates/vox-cli/src/commands/ci/build_bench.rs`, add to the `#[cfg(test)]` block:

```rust
#[test]
fn extract_unit_data_parses_real_cargo_format() {
    // MUST mirror real cargo output: `const`, float-seconds duration,
    // mode:"todo" on rows that DID build. Zero-duration rows are dropped
    // by duration (NOT by mode).
    let html = r#"<html><body><script>
const UNIT_DATA = [
  {"i":1,"name":"vox-config","version":"0.1.0","mode":"todo","target":"lib","duration":9.37,"start":0.0},
  {"i":2,"name":"vox-cli","version":"0.1.0","mode":"todo","target":"lib","duration":0.0,"start":0.0},
  {"i":3,"name":"vox-db","version":"0.1.0","mode":"run-custom-build","target":" build-script","duration":2.5,"start":0.0}
];
</script></body></html>"#;
    let records = extract_unit_data(html);
    assert_eq!(records.len(), 2, "zero-duration (cached) entries dropped");
    let config = records.iter().find(|r| r.name == "vox-config").unwrap();
    assert_eq!(config.duration_ms, 9370); // 9.37s -> 9370ms
}
```

Run: `cargo test -p vox-cli build_bench::tests::ingest_extracts_nonzero_duration_units -- --nocapture`
Expected: FAIL with "cannot find function `extract_unit_data`"

- [ ] **Step 3: Implement `extract_unit_data` and `TimingEntry`**

Add to `build_bench.rs` (before the `#[cfg(test)]` block):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingEntry {
    pub name: String,
    pub duration_ms: u64,
}

/// Parse the embedded UNIT_DATA JS array from a cargo --timings HTML file.
/// cargo emits `const UNIT_DATA = [...]` with float-seconds `duration`.
/// Drop zero-duration (cached/no-rebuild) units; filter on duration, NOT mode.
pub fn extract_unit_data(html: &str) -> Vec<TimingEntry> {
    // Anchor on the assignment — accepts const/var/let UNIT_DATA = [.
    let marker = "UNIT_DATA = [";
    let start = match html.find(marker) {
        Some(i) => i + marker.len() - 1, // include the '['
        None => return vec![],
    };
    let slice = &html[start..];
    let end = match slice.find("];") {
        Some(i) => i + 1,
        None => return vec![],
    };
    let array_json = &slice[..end];
    let raw: Vec<serde_json::Value> = match serde_json::from_str(array_json) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    raw.into_iter()
        .filter_map(|v| {
            // duration is FLOAT SECONDS — as_u64() would return None on floats.
            let secs = v["duration"].as_f64()?;
            let duration_ms = (secs * 1000.0).round() as u64;
            if duration_ms == 0 {
                return None; // cached / no-rebuild unit
            }
            let name = v["name"].as_str()?.to_string();
            Some(TimingEntry { name, duration_ms })
        })
        .collect()
}
```

- [ ] **Step 4: Run the test — expect PASS**

Run: `cargo test -p vox-cli build_bench::tests::ingest_extracts_nonzero_duration_units -- --nocapture`

- [ ] **Step 5: Write test for `find_newest_timings_html`**

```rust
#[test]
fn find_newest_timings_html_returns_none_when_dir_missing() {
    let result = find_newest_timings_html(std::path::Path::new("/nonexistent/path/xyz"));
    assert!(result.is_none());
}
```

Run: `cargo test -p vox-cli build_bench::tests::find_newest_timings_html_returns_none_when_dir_missing`
Expected: FAIL with "cannot find function"

- [ ] **Step 6: Implement `find_newest_timings_html`**

```rust
/// Return the most-recently-modified `cargo-timing-*.html` under `target/cargo-timings/`.
pub fn find_newest_timings_html(repo_root: &Path) -> Option<std::path::PathBuf> {
    let dir = repo_root.join("target/cargo-timings");
    let entries = std::fs::read_dir(&dir).ok()?;
    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("cargo-timing-")
                && e.path().extension().map_or(false, |x| x == "html")
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}
```

- [ ] **Step 7: Run test — expect PASS**

Run: `cargo test -p vox-cli build_bench::tests::find_newest_timings_html_returns_none_when_dir_missing`

- [ ] **Step 8: Write test for append_to_history**

```rust
#[test]
fn append_to_history_creates_file_and_appends_jsonl() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("history.jsonl");
    let entries = vec![
        TimingEntry { name: "vox-config 0.1.0".into(), duration_ms: 100 },
    ];
    append_to_history(&path, "ci-run-1", &entries).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("vox-config"));
    assert!(content.contains("ci-run-1"));
    // Second append adds another line
    append_to_history(&path, "ci-run-2", &entries).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert_eq!(lines.len(), 1); // first write only has 1 line — re-read after second append
    let content2 = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content2.lines().count(), 2);
}
```

- [ ] **Step 9: Implement `append_to_history`**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub run_id: String,
    pub entries: Vec<TimingEntry>,
}

/// Append one JSONL record to the history file (create if absent).
pub fn append_to_history(
    history_path: &Path,
    run_id: &str,
    entries: &[TimingEntry],
) -> Result<()> {
    use std::io::Write;
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let record = HistoryRecord {
        run_id: run_id.to_string(),
        entries: entries.to_vec(),
    };
    let line = serde_json::to_string(&record)? + "\n";
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(history_path)
        .with_context(|| format!("open history {}", history_path.display()))?;
    f.write_all(line.as_bytes())
        .with_context(|| "write history line")?;
    Ok(())
}
```

- [ ] **Step 10: Run test — expect PASS**

Run: `cargo test -p vox-cli build_bench::tests::append_to_history_creates_file_and_appends_jsonl`

- [ ] **Step 11: Wire `--ingest` into `run_build_bench`**

Add `ingest: bool` parameter to `run_build_bench`. At the end of the function (after the compare block), add:

```rust
if ingest {
    let history_path = root.join("contracts/ci/build-timings-history.v1.jsonl");
    if let Some(html_path) = find_newest_timings_html(root) {
        match std::fs::read_to_string(&html_path) {
            Ok(html) => {
                let entries = extract_unit_data(&html);
                eprintln!("build-bench --ingest: {} non-cached units from {}", entries.len(), html_path.display());
                let run_id = label.as_deref().unwrap_or("unknown");
                if let Err(e) = append_to_history(&history_path, run_id, &entries) {
                    eprintln!("build-bench --ingest: WARN failed to write history: {e}");
                }
            }
            Err(e) => eprintln!("build-bench --ingest: WARN could not read HTML: {e}"),
        }
    } else {
        eprintln!("build-bench --ingest: no cargo-timing HTML found under target/cargo-timings/");
    }
}
```

- [ ] **Step 12: Add `ingest: bool` to `BuildBench` variant in `cmd_enums.rs`**

Find the `BuildBench` struct fields. Add:
```rust
/// After build: parse the newest cargo-timings HTML and append to history.
#[arg(long)]
pub ingest: bool,
```

- [ ] **Step 13: Update `run_body.rs` dispatch to pass `ingest`**

Find the `CiCmd::BuildBench { label, write, compare, repeat }` arm and add `ingest`:
```rust
CiCmd::BuildBench { label, write, compare, repeat, ingest } => {
    super::build_bench::run_build_bench(root, label, write, compare, repeat, ingest)
}
```

- [ ] **Step 14: Format and clippy**

```
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
```

Fix any warnings.

- [ ] **Step 15: Run all build_bench tests**

```
cargo test -p vox-cli build_bench:: -- --nocapture
```

Expected: all pass.

- [ ] **Step 16: Smoke test — run --ingest against the existing HTML**

```
cargo run -p vox-cli -- ci build-bench --ingest --label manual-smoke
```

Expected: prints "N non-cached units from target/cargo-timings/cargo-timing-*.html". If N=0 (all sccache hits), that's correct — says so and still exits 0. Check `contracts/ci/build-timings-history.v1.jsonl` was created (even if empty).

- [ ] **Step 17: Commit**

```
git add crates/vox-cli/src/commands/ci/build_bench.rs
git add crates/vox-cli/src/commands/ci/cmd_enums.rs
git add crates/vox-cli/src/commands/ci/run_body.rs
git add contracts/ci/build-timings-history.v1.jsonl
git commit -m "feat(ci): build-bench --ingest parses cargo-timings HTML → history JSONL

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Component B — `vox ci dep-audit`

### Task B-1: Add `dep-audit` subcommand

**Files:**
- Create: `crates/vox-cli/src/commands/ci/dep_audit.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add `DepAudit` variant)
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch arm)
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (declare module)

- [ ] **Step 1: Create `dep_audit.rs` with data structures and failing test**

Create `crates/vox-cli/src/commands/ci/dep_audit.rs`:

```rust
//! `vox ci dep-audit` — per-crate dependency metrics.
//!
//! Reports workspace_dep_count, workspace_rdep_count, external_dep_count,
//! and on_vox_cli_critical_path for every workspace crate.
//! Writes to contracts/ci/dep-audit.v1.json.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CrateAudit {
    pub name: String,
    /// Number of workspace crates this crate directly depends on (normal deps only).
    pub workspace_dep_count: usize,
    /// Number of workspace crates that directly depend on this crate (reverse deps).
    pub workspace_rdep_count: usize,
    /// Number of non-workspace (external) direct dependencies.
    pub external_dep_count: usize,
    /// True if this crate is on the transitive critical path from root to vox-cli.
    pub on_vox_cli_critical_path: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepAuditReport {
    pub schema_version: u32,
    pub crates: Vec<CrateAudit>,
}

/// Build the audit report from a `cargo metadata` JSON value.
/// `target_crate` is the name to compute critical path toward (usually "vox-cli").
pub fn build_audit(meta: &serde_json::Value, target_crate: &str) -> Vec<CrateAudit> {
    let packages = match meta["packages"].as_array() {
        Some(p) => p,
        None => return vec![],
    };
    let workspace_members: HashSet<String> = meta["workspace_members"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| v.as_str())
        .map(|s| s.to_string())
        .collect();

    // id → name for workspace members
    let id_to_name: HashMap<&str, &str> = packages
        .iter()
        .filter(|p| {
            p["id"].as_str().map_or(false, |id| workspace_members.contains(id))
        })
        .filter_map(|p| {
            let id = p["id"].as_str()?;
            let name = p["name"].as_str()?;
            Some((id, name))
        })
        .collect();

    let workspace_names: HashSet<&str> = id_to_name.values().copied().collect();

    // Build adjacency: name → set of workspace dep names (normal deps only)
    let nodes = match meta["resolve"]["nodes"].as_array() {
        Some(n) => n,
        None => return vec![],
    };

    // node id → its normal dep ids
    let mut node_deps: HashMap<&str, Vec<&str>> = HashMap::new();
    for node in nodes {
        let id = match node["id"].as_str() {
            Some(s) => s,
            None => continue,
        };
        if !workspace_members.contains(id) {
            continue;
        }
        let deps: Vec<&str> = node["deps"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter(|d| {
                // only normal deps (not dev, not build)
                d["dep_kinds"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .any(|k| k["kind"].is_null())
            })
            .filter_map(|d| d["pkg"].as_str())
            .collect();
        node_deps.insert(id, deps);
    }

    // Compute per-package metrics
    // For external dep count we look at the package's Cargo.toml dependencies
    // via the packages array (not the resolve graph)
    let pkg_by_id: HashMap<&str, &serde_json::Value> = packages
        .iter()
        .filter_map(|p| p["id"].as_str().map(|id| (id, p)))
        .collect();

    // reverse dep map: name → set of names that depend on it
    let mut rdeps: HashMap<&str, HashSet<&str>> = HashMap::new();
    for (id, deps) in &node_deps {
        let from_name = match id_to_name.get(id) {
            Some(n) => n,
            None => continue,
        };
        for dep_id in deps {
            if let Some(dep_name) = id_to_name.get(dep_id) {
                rdeps.entry(dep_name).or_default().insert(from_name);
            }
        }
    }

    // Critical path: BFS backwards from vox-cli
    let on_critical_path: HashSet<&str> = {
        let mut visited = HashSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(target_crate);
        visited.insert(target_crate);
        // Build dep map by name
        let name_deps: HashMap<&str, Vec<&str>> = id_to_name
            .iter()
            .filter_map(|(id, name)| {
                let deps: Vec<&str> = node_deps
                    .get(id)
                    .map(|ds| {
                        ds.iter()
                            .filter_map(|dep_id| id_to_name.get(dep_id).copied())
                            .collect()
                    })
                    .unwrap_or_default();
                Some((*name, deps))
            })
            .collect();
        while let Some(current) = queue.pop_front() {
            if let Some(deps) = name_deps.get(current) {
                for dep in deps {
                    if visited.insert(dep) {
                        queue.push_back(dep);
                    }
                }
            }
        }
        visited
    };

    // Compute external dep count per workspace member
    let ext_dep_count: HashMap<&str, usize> = packages
        .iter()
        .filter(|p| p["id"].as_str().map_or(false, |id| workspace_members.contains(id)))
        .filter_map(|p| {
            let name = p["name"].as_str()?;
            let deps = p["dependencies"].as_array()?;
            let ext_count = deps
                .iter()
                .filter(|d| {
                    // normal dep
                    d["kind"].is_null()
                    // not a workspace member by name
                    && !workspace_names.contains(d["name"].as_str().unwrap_or(""))
                })
                .count();
            Some((name, ext_count))
        })
        .collect();

    let mut result: Vec<CrateAudit> = id_to_name
        .iter()
        .filter_map(|(id, name)| {
            let ws_deps = node_deps
                .get(id)
                .map(|ds| ds.iter().filter(|dep_id| id_to_name.contains_key(*dep_id)).count())
                .unwrap_or(0);
            Some(CrateAudit {
                name: name.to_string(),
                workspace_dep_count: ws_deps,
                workspace_rdep_count: rdeps.get(name).map_or(0, |s| s.len()),
                external_dep_count: *ext_dep_count.get(name).unwrap_or(&0),
                on_vox_cli_critical_path: on_critical_path.contains(name),
            })
        })
        .collect();

    result.sort_by(|a, b| b.workspace_rdep_count.cmp(&a.workspace_rdep_count).then(a.name.cmp(&b.name)));
    result
}

pub fn run_dep_audit(root: &Path, output: Option<String>) -> Result<()> {
    eprintln!("dep-audit: running cargo metadata…");
    // NOTE: `--no-deps=false` is INVALID (cargo: "unexpected value 'false'").
    // Omit --no-deps entirely to include the full resolve graph. This matches
    // the verified pattern in build_timings.rs:68.
    let out = Command::new(super::cargo_bin())
        .current_dir(root)
        .args(["metadata", "--format-version", "1"])
        .output()
        .context("cargo metadata")?;
    if !out.status.success() {
        anyhow::bail!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).context("parse metadata")?;
    let crates = build_audit(&meta, "vox-cli");
    eprintln!("dep-audit: {} workspace crates audited", crates.len());

    // Print summary table
    let on_path = crates.iter().filter(|c| c.on_vox_cli_critical_path).count();
    eprintln!("  {} crates on vox-cli critical path", on_path);
    eprintln!("  Top 5 by rdep count:");
    for c in crates.iter().take(5) {
        eprintln!(
            "    {:<40} rdeps={} deps={} ext={}{}",
            c.name,
            c.workspace_rdep_count,
            c.workspace_dep_count,
            c.external_dep_count,
            if c.on_vox_cli_critical_path { " [critical-path]" } else { "" }
        );
    }

    let report = DepAuditReport {
        schema_version: 1,
        crates,
    };

    let out_path = output
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("contracts/ci/dep-audit.v1.json"));

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let json = serde_json::to_string_pretty(&report)? + "\n";
    std::fs::write(&out_path, json).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!("dep-audit: wrote {}", out_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_meta(ws_crates: &[(&str, &str, &[&str])]) -> serde_json::Value {
        // ws_crates: (name, id, [dep_ids])
        let packages: Vec<serde_json::Value> = ws_crates
            .iter()
            .map(|(name, id, _deps)| {
                serde_json::json!({
                    "name": name,
                    "id": id,
                    "version": "0.1.0",
                    "dependencies": []
                })
            })
            .collect();
        let workspace_members: Vec<serde_json::Value> =
            ws_crates.iter().map(|(_, id, _)| serde_json::json!(id)).collect();
        let nodes: Vec<serde_json::Value> = ws_crates
            .iter()
            .map(|(_, id, deps)| {
                let dep_entries: Vec<serde_json::Value> = deps
                    .iter()
                    .map(|dep_id| {
                        serde_json::json!({
                            "pkg": dep_id,
                            "dep_kinds": [{"kind": null, "target": null}]
                        })
                    })
                    .collect();
                serde_json::json!({"id": id, "deps": dep_entries})
            })
            .collect();
        serde_json::json!({
            "packages": packages,
            "workspace_members": workspace_members,
            "resolve": {"nodes": nodes}
        })
    }

    #[test]
    fn rdep_count_is_correct() {
        // a → b → c (b depends on c, a depends on b)
        let meta = make_meta(&[
            ("a", "a@0.1.0 (path)", &["b@0.1.0 (path)"]),
            ("b", "b@0.1.0 (path)", &["c@0.1.0 (path)"]),
            ("c", "c@0.1.0 (path)", &[]),
        ]);
        let report = build_audit(&meta, "a");
        let c = report.iter().find(|x| x.name == "c").unwrap();
        assert_eq!(c.workspace_rdep_count, 1, "only b depends on c");
        let b = report.iter().find(|x| x.name == "b").unwrap();
        assert_eq!(b.workspace_rdep_count, 1, "only a depends on b");
        let a = report.iter().find(|x| x.name == "a").unwrap();
        assert_eq!(a.workspace_rdep_count, 0, "nothing depends on a");
    }

    #[test]
    fn critical_path_includes_transitive_deps() {
        let meta = make_meta(&[
            ("vox-cli", "vox-cli@0.1.0 (path)", &["mid@0.1.0 (path)"]),
            ("mid", "mid@0.1.0 (path)", &["leaf@0.1.0 (path)"]),
            ("leaf", "leaf@0.1.0 (path)", &[]),
            ("unrelated", "unrelated@0.1.0 (path)", &[]),
        ]);
        let report = build_audit(&meta, "vox-cli");
        let on_path: Vec<&str> = report
            .iter()
            .filter(|c| c.on_vox_cli_critical_path)
            .map(|c| c.name.as_str())
            .collect();
        assert!(on_path.contains(&"vox-cli"));
        assert!(on_path.contains(&"mid"));
        assert!(on_path.contains(&"leaf"));
        assert!(!on_path.contains(&"unrelated"));
    }

    #[test]
    fn report_is_sorted_by_rdep_count_descending() {
        let meta = make_meta(&[
            ("a", "a@0.1.0 (path)", &["b@0.1.0 (path)", "c@0.1.0 (path)"]),
            ("b", "b@0.1.0 (path)", &["c@0.1.0 (path)"]),
            ("c", "c@0.1.0 (path)", &[]),
        ]);
        let report = build_audit(&meta, "a");
        // c has 2 rdeps (a and b), b has 1 (a), a has 0
        assert_eq!(report[0].name, "c");
        assert_eq!(report[1].name, "b");
        assert_eq!(report[2].name, "a");
    }
}
```

- [ ] **Step 2: Run failing tests**

```
cargo test -p vox-cli dep_audit::tests:: -- --nocapture
```

Expected: FAIL (module not found — not wired yet).

- [ ] **Step 3: Declare module in `mod.rs`**

In `crates/vox-cli/src/commands/ci/mod.rs`, find the list of `pub(super) mod` declarations and add:
```rust
pub(super) mod dep_audit;
```

- [ ] **Step 4: Add `DepAudit` variant to `cmd_enums.rs`**

Find where other simple variants like `DepCycles` are declared. Add:
```rust
/// Per-crate dependency audit: blast-radius, critical path, dep counts.
DepAudit {
    /// Override output path (default: contracts/ci/dep-audit.v1.json).
    #[arg(long)]
    output: Option<String>,
},
```

- [ ] **Step 5: Add dispatch arm to `run_body.rs`**

In the `match cmd` block, add next to the DepCycles arm:
```rust
CiCmd::DepAudit { output } => super::dep_audit::run_dep_audit(root, output),
```

- [ ] **Step 6: Run the tests — expect PASS**

```
cargo test -p vox-cli dep_audit::tests:: -- --nocapture
```

- [ ] **Step 7: Format and clippy**

```
cargo fmt -p vox-cli
cargo clippy -p vox-cli -- -D warnings
```

- [ ] **Step 8: Smoke test**

Build the binary first (avoids the Windows lock issue by running tests directly):
```
cargo test -p vox-cli dep_audit:: -- --nocapture
```

Then run against the real workspace (this shells out to cargo metadata — takes ~5s):
```
cargo run -p vox-cli -- ci dep-audit 2>&1 | head -20
```

Expected: prints rdep summary, writes `contracts/ci/dep-audit.v1.json`. Check that vox-config shows rdeps=39 (approximately) and vox-cli shows on_vox_cli_critical_path=true.

- [ ] **Step 9: Commit**

```
git add crates/vox-cli/src/commands/ci/dep_audit.rs
git add crates/vox-cli/src/commands/ci/cmd_enums.rs
git add crates/vox-cli/src/commands/ci/run_body.rs
git add crates/vox-cli/src/commands/ci/mod.rs
git add contracts/ci/dep-audit.v1.json
git commit -m "feat(ci): dep-audit subcommand — per-crate blast-radius + critical-path report

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Component C — CI wiring for auto `--timings` capture

### Task C-1: Add `--timings` and `--ingest` to the CI build step

**Files:**
- Modify: `.github/workflows/ci.yml` — in the "Check, Build, and Test (Rust)" job, add `--timings` to the `cargo build -p vox-cli` step, and add a post-step that calls `vox ci build-bench --ingest`.

- [ ] **Step 1: Locate the exact cargo build step**

Read `.github/workflows/ci.yml` and search for the `cargo build -p vox-cli` line. Note the job name and surrounding step structure.

- [ ] **Step 2: Add `--timings` flag**

Change:
```yaml
run: cargo build -p vox-cli
```
To:
```yaml
run: cargo build -p vox-cli --timings
```

(If the line uses env vars or additional flags, preserve them — just append `--timings`.)

- [ ] **Step 3: Add a post-step to ingest**

After the cargo build step, add a new step:
```yaml
- name: Ingest build timings
  if: always()
  run: cargo run -p vox-cli -- ci build-bench --ingest --label "ci-${{ github.run_id }}"
```

- [ ] **Step 4: Upload the HTML artifact**

Add after the ingest step:
```yaml
- name: Upload cargo-timings HTML
  if: always()
  uses: actions/upload-artifact@v4
  with:
    name: cargo-timings-${{ github.run_id }}
    path: target/cargo-timings/cargo-timing-*.html
    if-no-files-found: warn
```

- [ ] **Step 5: Validate with actionlint locally**

```
actionlint .github/workflows/ci.yml
```

Expected: no errors.

- [ ] **Step 6: Commit**

```
git add .github/workflows/ci.yml
git commit -m "ci: record --timings on vox-cli build + upload HTML artifact

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Component D — Baseline population (cold build)

### Task D-1: Populate the all-zero baseline with real measurements

The baseline at `contracts/ci/build-bench-baseline.v1.json` has all `wall_ms: 0`. Real numbers require a cold build (sccache cannot serve the crates). This task runs the bench in CI with sccache disabled for the bench step.

- [ ] **Step 1: Add a one-off CI job to populate the baseline**

Add to `.github/workflows/ci.yml` (or a temporary separate workflow file for this one run):

```yaml
populate-build-bench-baseline:
  name: Populate build-bench baseline (cold)
  runs-on: [self-hosted, linux, x64]
  if: github.event_name == 'workflow_dispatch'  # only on manual trigger
  steps:
    - uses: actions/checkout@v4
    - name: Install Rust toolchain
      uses: dtolnay/rust-toolchain@stable
    - name: Build vox-cli (to ensure binary exists)
      run: cargo build -p vox-cli
    - name: Run build-bench (cold — sccache disabled for this step)
      env:
        RUSTC_WRAPPER: ""
        SCCACHE_RECACHE: "1"
      run: |
        cargo run -p vox-cli -- ci build-bench \
          --repeat 3 \
          --label baseline \
          --write contracts/ci/build-bench-baseline.v1.json
    - name: Upload new baseline
      uses: actions/upload-artifact@v4
      with:
        name: build-bench-baseline
        path: contracts/ci/build-bench-baseline.v1.json
```

- [ ] **Step 2: Trigger the workflow manually from GitHub UI**

Go to Actions → (workflow name) → Run workflow. Download the artifact and copy it to `contracts/ci/build-bench-baseline.v1.json`.

- [ ] **Step 3: Commit the populated baseline**

```
git add contracts/ci/build-bench-baseline.v1.json
git commit -m "chore(ci): populate build-bench baseline with real cold-build measurements

Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>"
```

---

## Execution order

A → B → C → D. A and B are fully independent (edit different files) and can be done in parallel in separate worktrees. C depends on A (needs `--ingest` to exist). D depends on C (needs CI to run the bench) and is the only step requiring a manual CI trigger.

## Acceptance criteria

- `cargo test -p vox-cli build_bench:: dep_audit::` — all green
- `vox ci build-bench --ingest` — exits 0, creates/appends `contracts/ci/build-timings-history.v1.jsonl`
- `vox ci dep-audit` — exits 0, writes `contracts/ci/dep-audit.v1.json` with `workspace_rdep_count` > 0 for vox-config
- CI build step emits a `target/cargo-timings/*.html` artifact on every run
- Baseline JSON has no `wall_ms: 0` entries after D runs
