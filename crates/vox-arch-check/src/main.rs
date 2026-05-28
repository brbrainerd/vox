//! Architecture check: enforces workspace-wide structural rules.
//!
//! Reads `docs/src/architecture/layers.toml` and runs thirteen rules over the
//! current `cargo metadata` snapshot:
//!
//!   1. **Layer ordering** (strict by default) — a crate at layer N may depend
//!      only on crates at layer ≤ N. Inversions in `[[known_inversions]]` are
//!      tolerated.
//!   2. **Fan-in tracker** (warn) — workspace dependents per crate vs.
//!      `max_dependents`.
//!   3. **LoC budget** (warn) — `wc -l` over `src/**/*.rs` vs. `max_loc`.
//!   4. **Orphan detector** (warn) — flags crates with 0 in-tree consumers
//!      AND `kind != "plugin" | "binary" | "test-only"`.
//!   5. **Docstring lint** (strict for L0-L2, warn for L3+) — flags `lib.rs`
//!      files that don't open with `//!`.
//!   6. **Description present** (warn) — flags L1+ library crates with no
//!      Cargo.toml `description` or one shorter than 40 characters.
//!   7. **Where-things-live coverage** (warn) — flags workspace members not
//!      listed in `docs/src/architecture/where-things-live.md`.
//!   8. **Staleness** (warn) — flags crates with no commits since the last
//!      release date in `CHANGELOG.md`. Mark stable utility crates with
//!      `staleness_exempt = true` in `layers.toml` to silence the warning.
//!      Implemented with one batched `git log` over the repo when possible,
//!      falling back to per-crate `git log` if the batch query fails.
//!   9. **Generated-file drift** (warn) — files containing a
//!      `@generated-hash <hex>` header whose content hash no longer matches,
//!      indicating a hand-edit of a machine-generated file.
//!  10. **Forbidden direct dependencies** (error) — crates must not directly
//!      depend on any deps listed under `[[forbidden_deps]]` in `layers.toml`.
//!  11. **Forbidden code patterns** (warn) — patterns in `[[forbidden_pattern]]`
//!      that must not appear in source (e.g. raw `Command::new("git")`).
//!  12. **WTL / layers.toml / disk three-way parity** (error) — (a) every
//!      `[crates]` entry in `layers.toml` whose directory is absent from
//!      `crates/` AND is not in the `[planned]` table; (b) every
//!      `crates/<name>/` reference in `where-things-live.md` whose directory
//!      is absent from disk AND is not in `[planned]`. Add ghost entries to
//!      `[planned]` with a `plan =` pointer to suppress the warning.
//!  13. **LoC delta regression** (warn) — for crates with `max_loc` budgets,
//!      warns if the current LoC is more than 15% higher than the LoC at the
//!      last tagged release (`v{version}` from `CHANGELOG.md`). Only fires for
//!      crates >2000 LoC to avoid noise. Catches regrowth at PR time rather
//!      than at the hard ceiling.
//!  14. **No-cdylib-as-normal-dep** (error by default) — a workspace crate
//!      must not take a non-optional, non-dev compile-time dependency on a
//!      workspace crate whose output is a `cdylib`. Plugin cdylibs are loaded
//!      dynamically at runtime via `vox-plugin-host`; linking them statically
//!      breaks the plugin boundary and bloats the binary.
//!  15. **Workspace-dep budget** (warn) — caps how many workspace-member
//!      crates a given crate may take as normal (compile-time) dependencies.
//!      Set `max_workspace_deps` in `layers.toml` per-crate. Useful for
//!      detecting "kitchen-sink" aggregator crates before they hit the hard
//!      fan-in / LoC ceilings (e.g. `vox-cli = 60`).
//!
//! Layer ordering is the only rule that fails the build by default; the other
//! fourteen rules are warn-only unless promoted via `[guards]` in `layers.toml`.
//!
//! Modes:
//!   default        — strict layer-ordering; warn-only on the other eight
//!   --warn-only    — warn on layer-ordering too (used during transition phases)
//!
//! Exit codes:
//!   0 — clean (or warn-only)
//!   1 — strict rule failed, OR config error

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, anyhow};
use cargo_metadata::MetadataCommand;
use serde::Deserialize;

mod cache;
mod forbidden_patterns;
use forbidden_patterns::{ForbiddenPatternRule, scan_all as scan_forbidden_patterns_all};

/// Rule 14: evidence-ledger integrity check.
/// See `evidence_ledger.rs` for the contract.
mod evidence_ledger;
use evidence_ledger::{EvidenceFinding, FindingKind};

#[derive(Debug, Deserialize)]
struct LayersConfig {
    crates: HashMap<String, CrateEntry>,
    #[serde(default)]
    known_inversions: Vec<KnownInversion>,
    #[serde(default)]
    forbidden_deps: Vec<ForbiddenDepRule>,
    /// Rule 11 (P3-T7): forbidden code patterns with optional allow annotations.
    #[serde(default)]
    forbidden_pattern: Vec<ForbiddenPatternRule>,
    #[serde(default)]
    guards: GuardsConfig,
    #[serde(default)]
    arch_check: ArchCheckConfig,
    /// Rule 12: crates that are documented (in WTL / layers.toml) but not yet on
    /// disk. Entries here suppress the parity warning; each must point to the plan
    /// doc that owns the work via `plan = "..."`.
    #[serde(default)]
    planned: HashMap<String, PlannedEntry>,
}

/// A crate that is planned but not yet landed on disk. Used by Rule 12 to suppress
/// WTL/layers.toml parity warnings for in-flight designs.
#[derive(Debug, Deserialize)]
struct PlannedEntry {
    /// Path to the architecture doc that owns this planned crate.
    #[allow(dead_code)]
    plan: String,
    /// Intended layer when the crate lands.
    #[allow(dead_code)]
    layer: Option<u8>,
}

/// Optional knobs for `vox-arch-check` itself (see `[arch_check.walk_prune]` in layers.toml).
#[derive(Debug, Default, Deserialize)]
struct ArchCheckConfig {
    #[serde(default)]
    walk_prune: WalkPruneConfig,
}

#[derive(Debug, Default, Deserialize)]
struct WalkPruneConfig {
    /// Extra directory *names* (not full paths) to skip when recursing.
    #[serde(default)]
    extra_skip_dir_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CrateEntry {
    layer: u8,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    max_dependents: Option<usize>,
    #[serde(default)]
    max_loc: Option<usize>,
    /// Opt out of Rule 8 staleness check for intentionally stable crates.
    #[serde(default)]
    staleness_exempt: bool,
    /// Opt out of Rule 4 orphan check — for libraries consumed by generated
    /// code or Tauri app scaffolding that has no in-tree Rust dep edge.
    #[serde(default)]
    orphan_exempt: bool,
    /// Rule 15: cap on normal workspace-member deps for this crate.
    #[serde(default)]
    max_workspace_deps: Option<usize>,
    /// Other crates this one is structurally a sibling of. Consumed by
    /// `vox-drift-check`'s `sweep/duplicate-body` rule to tolerate intentional
    /// code duplication across vendor splits and extraction migrations. Edges
    /// are undirected (closure over the declared graph).
    #[serde(default)]
    #[allow(dead_code)]
    sibling_of: Vec<String>,
}

fn default_kind() -> String {
    "library".to_string()
}

#[derive(Debug, Deserialize)]
struct KnownInversion {
    from: String,
    to: String,
    #[allow(dead_code)]
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ForbiddenDepRule {
    #[serde(rename = "crate")]
    krate: String,
    forbidden: Vec<String>,
    #[allow(dead_code)]
    reason: String,
}

#[derive(Debug, Default, Deserialize)]
struct GuardsConfig {
    /// "error" or "warn"; defaults to "warn" for all but layer ordering.
    #[serde(default)]
    fan_in: Option<String>,
    #[serde(default)]
    loc_budget: Option<String>,
    #[serde(default)]
    orphan: Option<String>,
    #[serde(default)]
    docstring: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    where_things_live: Option<String>,
    #[serde(default)]
    staleness: Option<String>,
    #[serde(default)]
    generated_file_drift: Option<String>,
    #[serde(default)]
    forbidden_deps: Option<String>,
    #[serde(default)]
    forbidden_pattern: Option<String>,
    /// Rule 12: WTL / layers.toml / disk three-way parity.
    #[serde(default)]
    wtl_parity: Option<String>,
    /// Rule 13: LoC delta regression check.
    #[serde(default)]
    loc_delta: Option<String>,
    /// Rule 14: cdylib-as-normal-dep guard (default: error).
    #[serde(default)]
    cdylib_dep: Option<String>,
    /// Rule 15: workspace-dep budget guard (default: warn).
    #[serde(default)]
    workspace_deps: Option<String>,
}

fn main() -> ExitCode {
    let warn_only = std::env::args().any(|a| a == "--warn-only");

    match run(warn_only) {
        Ok(report) => {
            report.print_summary();
            if report.strict_failed() && !warn_only {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!("vox-arch-check: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Default)]
struct Report {
    inversions: Vec<(String, String, u8, u8)>,
    fan_in_warns: Vec<(String, usize, usize)>,
    loc_warns: Vec<(String, usize, usize)>,
    orphan_warns: Vec<String>,
    /// Docstring findings split by strictness: (name, strict)
    docstring_warns: Vec<(String, bool)>,
    description_warns: Vec<String>,
    where_things_live_warns: Vec<String>,
    /// Rule 8: (crate_name, last_commit_date YYYY-MM-DD).
    staleness_warns: Vec<(String, String)>,
    /// "vX.Y.Z (YYYY-MM-DD)" used in the staleness summary line.
    staleness_since: String,
    /// Rule 9: (file_path_relative, expected_hash, actual_hash).
    generated_file_drift_warns: Vec<(PathBuf, String, String)>,
    /// Rule 10: (crate, forbidden_dep) pairs for direct forbidden-dep violations.
    forbidden_dep_violations: Vec<(String, String)>,
    /// Rule 11 (P3-T7): (rule_name, file, line, matched, reason) tuples.
    forbidden_pattern_hits: Vec<(String, PathBuf, usize, String, String)>,
    /// Rule 12: WTL / layers.toml / disk three-way parity violations.
    wtl_parity_warns: Vec<String>,
    /// Rule 13: (crate, current_loc, baseline_loc, pct_growth).
    loc_delta_warns: Vec<(String, usize, usize, f64)>,
    /// Rule 14: (consumer, cdylib_dep) pairs where a normal dep links a cdylib.
    cdylib_dep_warns: Vec<(String, String)>,
    /// Rule 15: (crate, ws_dep_count, budget) tuples over the workspace-dep budget.
    workspace_dep_warns: Vec<(String, usize, usize)>,
    /// Rule 16: evidence-ledger integrity findings (missing / stale artifacts).
    evidence_findings: Vec<EvidenceFinding>,
    /// Whether each rule's failure should be treated as strict (vs. warn-only).
    strict_layer: bool,
    strict_fan_in: bool,
    strict_loc: bool,
    strict_orphan: bool,
    strict_docstring: bool,
    strict_description: bool,
    strict_where_things_live: bool,
    strict_staleness: bool,
    strict_generated_file_drift: bool,
    strict_forbidden_deps: bool,
    strict_forbidden_pattern: bool,
    strict_wtl_parity: bool,
    strict_loc_delta: bool,
    strict_cdylib_dep: bool,
    strict_workspace_deps: bool,
    /// Rule 16 strictness. Default: false — ledger is freshly seeded and many
    /// rows point at not-yet-existing artifacts. Flip to true once
    /// `vox audit --gate all --strict-block-ga` exits 0 (i.e. when block-GA
    /// gates are met by real measurements rather than corpus-inventory).
    strict_evidence_ledger: bool,
}

impl Report {
    fn strict_failed(&self) -> bool {
        (self.strict_layer && !self.inversions.is_empty())
            || (self.strict_fan_in && !self.fan_in_warns.is_empty())
            || (self.strict_loc && !self.loc_warns.is_empty())
            || (self.strict_orphan && !self.orphan_warns.is_empty())
            || self.docstring_warns.iter().any(|(_, strict)| *strict)
            || (self.strict_description && !self.description_warns.is_empty())
            || (self.strict_where_things_live && !self.where_things_live_warns.is_empty())
            || (self.strict_staleness && !self.staleness_warns.is_empty())
            || (self.strict_generated_file_drift && !self.generated_file_drift_warns.is_empty())
            || (self.strict_forbidden_deps && !self.forbidden_dep_violations.is_empty())
            || (self.strict_forbidden_pattern && !self.forbidden_pattern_hits.is_empty())
            || (self.strict_wtl_parity && !self.wtl_parity_warns.is_empty())
            || (self.strict_loc_delta && !self.loc_delta_warns.is_empty())
            || (self.strict_cdylib_dep && !self.cdylib_dep_warns.is_empty())
            || (self.strict_workspace_deps && !self.workspace_dep_warns.is_empty())
            || (self.strict_evidence_ledger
                && self
                    .evidence_findings
                    .iter()
                    .any(|f| f.kind.severity() == "ERROR"))
    }

    fn print_summary(&self) {
        let mut any = false;
        if !self.inversions.is_empty() {
            any = true;
            let label = if self.strict_layer { "ERROR" } else { "warn" };
            eprintln!("[{label}] layer inversions ({}):", self.inversions.len());
            for (from, to, fl, tl) in &self.inversions {
                eprintln!("  {from} (L{fl}) → {to} (L{tl})");
            }
        }
        if !self.fan_in_warns.is_empty() {
            any = true;
            let label = if self.strict_fan_in { "ERROR" } else { "warn" };
            eprintln!(
                "[{label}] fan-in over budget ({}):",
                self.fan_in_warns.len()
            );
            for (name, count, budget) in &self.fan_in_warns {
                eprintln!("  {name}: {count} dependents (budget {budget})");
            }
        }
        if !self.loc_warns.is_empty() {
            any = true;
            let label = if self.strict_loc { "ERROR" } else { "warn" };
            eprintln!("[{label}] LoC budget exceeded ({}):", self.loc_warns.len());
            for (name, loc, budget) in &self.loc_warns {
                eprintln!("  {name}: {loc} LoC (budget {budget})");
            }
        }
        if !self.orphan_warns.is_empty() {
            any = true;
            let label = if self.strict_orphan { "ERROR" } else { "warn" };
            eprintln!(
                "[{label}] orphan crates ({}) — 0 in-tree consumers and kind=library:",
                self.orphan_warns.len()
            );
            for name in &self.orphan_warns {
                eprintln!("  {name}");
            }
        }
        // Docstring: split strict vs. warn findings for display
        let docstring_strict: Vec<&str> = self
            .docstring_warns
            .iter()
            .filter(|(_, s)| *s)
            .map(|(n, _)| n.as_str())
            .collect();
        let docstring_warn: Vec<&str> = self
            .docstring_warns
            .iter()
            .filter(|(_, s)| !*s)
            .map(|(n, _)| n.as_str())
            .collect();
        if !docstring_strict.is_empty() {
            any = true;
            eprintln!(
                "[ERROR] lib.rs without `//!` opening docstring — L0-L2 (strict) ({}):",
                docstring_strict.len()
            );
            for name in &docstring_strict {
                eprintln!("  {name}");
            }
        }
        if !docstring_warn.is_empty() {
            any = true;
            let label = if self.strict_docstring {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] lib.rs without `//!` opening docstring — L3+ ({}):",
                docstring_warn.len()
            );
            for name in &docstring_warn {
                eprintln!("  {name}");
            }
        }
        if !self.description_warns.is_empty() {
            any = true;
            let label = if self.strict_description {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] Cargo.toml description missing or too short ({}):",
                self.description_warns.len()
            );
            for msg in &self.description_warns {
                eprintln!("  {msg}");
            }
        }
        if !self.where_things_live_warns.is_empty() {
            any = true;
            let label = if self.strict_where_things_live {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] crates not listed in where-things-live.md ({}):",
                self.where_things_live_warns.len()
            );
            for msg in &self.where_things_live_warns {
                eprintln!("  {msg}");
            }
        }
        if !self.staleness_warns.is_empty() {
            any = true;
            let label = if self.strict_staleness {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] crates unchanged since {} ({}) — add `staleness_exempt = true` in layers.toml to silence:",
                self.staleness_since,
                self.staleness_warns.len()
            );
            for (name, date) in &self.staleness_warns {
                eprintln!("  {name}: last changed {date}");
            }
        }
        if !self.generated_file_drift_warns.is_empty() {
            any = true;
            let label = if self.strict_generated_file_drift {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] generated-file drift — hand-edited @generated files ({}):",
                self.generated_file_drift_warns.len()
            );
            for (path, expected, actual) in &self.generated_file_drift_warns {
                eprintln!(
                    "  {}: expected hash {expected}, got {actual}  (re-run the generator)",
                    path.display()
                );
            }
        }
        if !self.forbidden_dep_violations.is_empty() {
            any = true;
            let label = if self.strict_forbidden_deps {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] forbidden direct dependencies ({}):",
                self.forbidden_dep_violations.len()
            );
            for (krate, forbidden) in &self.forbidden_dep_violations {
                eprintln!("  {krate} → {forbidden}  (see [[forbidden_deps]] in layers.toml)");
            }
        }
        if !self.forbidden_pattern_hits.is_empty() {
            any = true;
            let label = if self.strict_forbidden_pattern {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] forbidden_pattern violations ({}):",
                self.forbidden_pattern_hits.len()
            );
            for (rule, file, line, matched, reason) in &self.forbidden_pattern_hits {
                eprintln!(
                    "  [{}] {}:{} — {}\n    reason: {reason}",
                    rule,
                    file.display(),
                    line,
                    matched
                );
            }
        }
        if !self.wtl_parity_warns.is_empty() {
            any = true;
            let label = if self.strict_wtl_parity {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] WTL/layers.toml/disk parity violations ({}):",
                self.wtl_parity_warns.len()
            );
            for msg in &self.wtl_parity_warns {
                eprintln!("  {msg}");
            }
            eprintln!(
                "  → Add missing entries to [planned] in layers.toml with a `plan =` pointer, or create the crate directory."
            );
        }
        if !self.loc_delta_warns.is_empty() {
            any = true;
            let label = if self.strict_loc_delta {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] LoC delta >15% since last release ({}) — review before merging:",
                self.loc_delta_warns.len()
            );
            for (name, current, baseline, pct) in &self.loc_delta_warns {
                eprintln!("  {name}: {current} LoC (was {baseline} at last release, +{pct:.0}%)");
            }
            eprintln!("  → Large deltas indicate scope creep. Consider extracting a sub-crate.");
        }
        if !self.cdylib_dep_warns.is_empty() {
            any = true;
            let label = if self.strict_cdylib_dep {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] cdylib linked as normal compile-time dep ({}) — use vox-plugin-host instead:",
                self.cdylib_dep_warns.len()
            );
            for (consumer, plugin) in &self.cdylib_dep_warns {
                eprintln!(
                    "  {consumer} → {plugin}  (cdylib must be loaded dynamically, not linked)"
                );
            }
        }
        if !self.workspace_dep_warns.is_empty() {
            any = true;
            let label = if self.strict_workspace_deps {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[{label}] workspace-dep budget exceeded ({}):",
                self.workspace_dep_warns.len()
            );
            for (name, count, budget) in &self.workspace_dep_warns {
                eprintln!("  {name}: {count} workspace deps (budget {budget})");
            }
        }
        if !self.evidence_findings.is_empty() {
            any = true;
            let strict_label = if self.strict_evidence_ledger {
                "ERROR"
            } else {
                "warn"
            };
            eprintln!(
                "[evidence-ledger] {} finding(s) — claims in `docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md` point at:",
                self.evidence_findings.len()
            );
            for f in &self.evidence_findings {
                let kind_label = match &f.kind {
                    FindingKind::MissingArtifact => "missing".to_string(),
                    FindingKind::DirectoryHasNoDatedReports => "no dated reports".to_string(),
                    FindingKind::Stale {
                        age_days,
                        max_age_days,
                    } => {
                        format!("stale ({age_days}d > {max_age_days}d budget)")
                    }
                    FindingKind::UnknownArtifactKind(k) => format!("unknown kind `{k}`"),
                };
                let sev = f.kind.severity();
                let row_label = if sev == "ERROR" { strict_label } else { sev };
                eprintln!(
                    "  [{row_label}] {claim} → {kind} @ {path}",
                    claim = f.claim_id,
                    kind = kind_label,
                    path = f.artifact_path.display(),
                );
            }
            eprintln!(
                "  → See contracts/reports/evidence-ledger.v1.json and the honest plan at docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md §1.2."
            );
        }
        if !any {
            eprintln!(
                "vox-arch-check {}: clean ✓",
                concat!(
                    env!("CARGO_PKG_VERSION"),
                    "+build.",
                    env!("VOX_BUILD_NUMBER"),
                    " (",
                    env!("VOX_GIT_HASH"),
                    ")"
                )
            );
        }
    }
}

fn parse_strictness(setting: Option<&String>, default_strict: bool) -> bool {
    match setting.map(|s| s.as_str()) {
        Some("error") | Some("strict") => true,
        Some("warn") | Some("warning") => false,
        _ => default_strict,
    }
}

/// Built-in directory *names* (final path component) never recursed into by Rule 3/9/11.
const WALK_PRUNE_DEFAULT_DIR_NAMES: &[&str] = &[
    "target",
    ".git",
    "node_modules",
    ".pnpm-store",
    "__pycache__",
    ".venv",
    ".mypy_cache",
    ".turbo",
    ".next",
    ".parcel-cache",
    ".cargo",
];

fn built_in_walk_prune_names() -> HashSet<String> {
    WALK_PRUNE_DEFAULT_DIR_NAMES
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

fn walk_prune_dir_names(cfg: &LayersConfig) -> HashSet<String> {
    let mut s = built_in_walk_prune_names();
    for extra in &cfg.arch_check.walk_prune.extra_skip_dir_names {
        let t = extra.trim();
        if !t.is_empty() {
            s.insert(t.to_string());
        }
    }
    s
}

fn dir_entry_should_be_pruned(path: &Path, prune_dir_names: &HashSet<String>) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| prune_dir_names.contains(name))
}

/// Recursive file listing for repo scans; skips heavy artifact trees (see `walk_prune_dir_names`).
fn walk_repo_files(root: &Path, prune_dir_names: &HashSet<String>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let entries = match std::fs::read_dir(&p) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if !dir_entry_should_be_pruned(&path, prune_dir_names) {
                    stack.push(path);
                }
            } else {
                out.push(path);
            }
        }
    }
    out
}

/// Parent directory of `manifest_path`, relative to `repo`, using `/` separators.
fn manifest_parent_rel_to_repo(repo: &Path, manifest_path: &Path) -> Option<String> {
    let parent = manifest_path.parent()?;
    let rel = parent.strip_prefix(repo).ok()?;
    if rel.as_os_str().is_empty() {
        return Some(String::new());
    }
    let mut s = rel.to_string_lossy().replace('\\', "/");
    while s.ends_with('/') {
        s.pop();
    }
    Some(s)
}

/// True if a path reported by `git log --name-only` lies under the crate root `rel_dir`.
fn git_path_touches_crate_root(git_path: &str, rel_dir: &str) -> bool {
    let p = git_path.trim_start_matches("./").replace('\\', "/");
    let rel = rel_dir.trim_matches('/');
    if rel.is_empty() {
        // Root package (manifest at workspace root): touches are top-level source paths.
        return p == "Cargo.toml" || p.starts_with("src/") || p.starts_with("benches/");
    }
    p == rel || p.starts_with(&format!("{rel}/"))
}

/// Paths touched in commits selected by `git log --since {release_date}T00:00:00Z`
/// (`--name-only`, empty `--pretty`); matches Git's author-date `--since` semantics.
fn git_paths_touched_since(repo: &Path, release_date: &str) -> Option<HashSet<String>> {
    let since = format!("{release_date}T00:00:00Z");
    let out = Command::new("git")
        .current_dir(repo)
        .args(["log", "--since", &since, "--name-only", "--pretty=format:"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut paths = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        paths.insert(line.replace('\\', "/"));
    }
    Some(paths)
}

fn run(warn_only_flag: bool) -> Result<Report> {
    // `--no-deps` skips transitive dependency resolution. arch-check never reads
    // `metadata.resolve`; each package's declared `dependencies` is still
    // populated so Rules 10/11/14/15 work unchanged.
    let metadata_full = MetadataCommand::new()
        .no_deps()
        .exec()
        .context("cargo metadata failed")?;

    let workspace_root: PathBuf = metadata_full.workspace_root.clone().into();

    // Load or prime the git-paths cache. Non-fatal: a miss just means we run git normally.
    let cache_key = cache::compute_key(&workspace_root).ok();
    let cached = cache_key
        .as_deref()
        .and_then(|k| cache::load(&workspace_root, k));

    let layers_path = workspace_root.join("docs/src/architecture/layers.toml");

    let layers_text = std::fs::read_to_string(&layers_path)
        .with_context(|| format!("reading {}", layers_path.display()))?;
    let layers: LayersConfig = toml::from_str(&layers_text)
        .with_context(|| format!("parsing {}", layers_path.display()))?;
    let prune_dirs = walk_prune_dir_names(&layers);

    let workspace_members: HashSet<&str> = metadata_full
        .workspace_packages()
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // Layer ordering is strict by default; the others default to warn-only.
    // --warn-only flag downgrades layer ordering to warn too.
    let mut report = Report {
        strict_layer: !warn_only_flag,
        strict_fan_in: parse_strictness(layers.guards.fan_in.as_ref(), false),
        strict_loc: parse_strictness(layers.guards.loc_budget.as_ref(), false),
        strict_orphan: parse_strictness(layers.guards.orphan.as_ref(), false),
        strict_docstring: parse_strictness(layers.guards.docstring.as_ref(), false),
        strict_description: parse_strictness(layers.guards.description.as_ref(), false),
        strict_where_things_live: parse_strictness(layers.guards.where_things_live.as_ref(), false),
        strict_staleness: parse_strictness(layers.guards.staleness.as_ref(), false),
        strict_generated_file_drift: parse_strictness(
            layers.guards.generated_file_drift.as_ref(),
            false,
        ),
        strict_forbidden_deps: parse_strictness(layers.guards.forbidden_deps.as_ref(), false),
        strict_forbidden_pattern: parse_strictness(layers.guards.forbidden_pattern.as_ref(), false),
        strict_wtl_parity: parse_strictness(layers.guards.wtl_parity.as_ref(), false),
        strict_loc_delta: parse_strictness(layers.guards.loc_delta.as_ref(), false),
        strict_cdylib_dep: parse_strictness(layers.guards.cdylib_dep.as_ref(), true),
        strict_workspace_deps: parse_strictness(layers.guards.workspace_deps.as_ref(), false),
        ..Report::default()
    };

    let profile_on = std::env::var("VOX_ARCH_CHECK_PROFILE").is_ok();
    let profile_start = std::time::Instant::now();
    let mut prof_last = profile_start;
    let prof = |label: &str, prev: &mut std::time::Instant| {
        if profile_on {
            let now = std::time::Instant::now();
            eprintln!(
                "[profile] {}: {}ms",
                label,
                now.duration_since(*prev).as_millis()
            );
            *prev = now;
        }
    };
    prof("setup (metadata+layers+cache)", &mut prof_last);

    // ── Rule 1: Layer ordering + Rule 2: Fan-in + Rule 15: Workspace-dep budget (single pass) ──
    let mut dependent_count: HashMap<String, usize> = HashMap::new();
    let mut workspace_dep_count: HashMap<String, usize> = HashMap::new();
    let mut unlisted: Vec<String> = Vec::new();

    for pkg in metadata_full.workspace_packages() {
        let from_name = pkg.name.as_str();
        let from_layer = match layers.crates.get(from_name) {
            Some(e) => e.layer,
            None => {
                unlisted.push(from_name.to_string());
                continue;
            }
        };
        for dep in &pkg.dependencies {
            let to_name = dep.name.as_str();
            if !workspace_members.contains(to_name) {
                continue;
            }
            *dependent_count.entry(to_name.to_string()).or_insert(0) += 1;
            if dep.kind == cargo_metadata::DependencyKind::Normal {
                *workspace_dep_count
                    .entry(from_name.to_string())
                    .or_insert(0) += 1;
            }

            // Layer-inversion check only applies to normal (production) deps.
            // Dev-deps and build-deps don't affect the runtime binary's layer purity.
            if dep.kind != cargo_metadata::DependencyKind::Normal {
                continue;
            }
            let to_layer = match layers.crates.get(to_name) {
                Some(e) => e.layer,
                None => continue,
            };
            if to_layer > from_layer {
                let is_known = layers
                    .known_inversions
                    .iter()
                    .any(|k| k.from == from_name && k.to == to_name);
                if !is_known {
                    report.inversions.push((
                        from_name.to_string(),
                        to_name.to_string(),
                        from_layer,
                        to_layer,
                    ));
                }
            }
        }
    }

    if !unlisted.is_empty() {
        unlisted.sort();
        unlisted.dedup();
        return Err(anyhow!(
            "{} workspace crate(s) missing from layers.toml: {}",
            unlisted.len(),
            unlisted.join(", ")
        ));
    }

    // Rule 2: fan-in budget
    for (name, entry) in &layers.crates {
        if let Some(budget) = entry.max_dependents {
            let count = dependent_count.get(name).copied().unwrap_or(0);
            if count > budget {
                report.fan_in_warns.push((name.clone(), count, budget));
            }
        }
    }

    prof("rules 1+2+15 (layer/fan-in/wsdep)", &mut prof_last);
    // ── Rule 3: LoC budget ──
    for pkg in metadata_full.workspace_packages() {
        let name = pkg.name.as_str();
        let entry = match layers.crates.get(name) {
            Some(e) => e,
            None => continue,
        };
        let budget = match entry.max_loc {
            Some(b) => b,
            None => continue,
        };
        let manifest_dir = Path::new(pkg.manifest_path.as_str())
            .parent()
            .unwrap_or(Path::new("."));
        let src_dir = manifest_dir.join("src");
        let loc = count_loc(&src_dir, &prune_dirs).unwrap_or(0);
        if loc > budget {
            report.loc_warns.push((name.to_string(), loc, budget));
        }
    }

    prof("rule 3 (LoC budget)", &mut prof_last);
    // ── Rule 4: Orphan detector ──
    for (name, entry) in &layers.crates {
        if entry.kind != "library" || entry.orphan_exempt {
            continue;
        }
        let count = dependent_count.get(name).copied().unwrap_or(0);
        if count == 0 && workspace_members.contains(name.as_str()) {
            report.orphan_warns.push(name.clone());
        }
    }
    report.orphan_warns.sort();

    prof("rule 4 (orphan)", &mut prof_last);
    // ── Rule 5: Docstring lint (strict for L0-L2, warn for L3+) ──
    for pkg in metadata_full.workspace_packages() {
        let name = pkg.name.as_str();
        let layer = match layers.crates.get(name) {
            Some(e) => e.layer,
            None => continue,
        };
        let manifest_dir = Path::new(pkg.manifest_path.as_str())
            .parent()
            .unwrap_or(Path::new("."));
        let lib_rs = manifest_dir.join("src").join("lib.rs");
        if !lib_rs.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&lib_rs) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let first_nonempty = content.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        if !first_nonempty.trim_start().starts_with("//!") {
            // L0-L2: strict (always fail); L3+: governed by strict_docstring guard
            let is_strict = layer <= 2;
            report.docstring_warns.push((name.to_string(), is_strict));
        }
    }
    report.docstring_warns.sort_by(|a, b| a.0.cmp(&b.0));

    prof("rule 5 (docstring lint)", &mut prof_last);
    // ── Rule 6: Description present ──
    report.description_warns = check_description_present(&metadata_full, &layers);

    prof("rule 6 (description)", &mut prof_last);
    // ── Rule 7: Where-things-live coverage ──
    report.where_things_live_warns =
        check_where_things_live_coverage(&metadata_full, &layers, &workspace_root).unwrap_or_else(
            |e| {
                eprintln!("warn: where-things-live check skipped: {e:#}");
                Vec::new()
            },
        );

    prof("rule 7 (WTL coverage)", &mut prof_last);
    // ── Rule 8: Staleness ──
    // Flags crates with no commits since the last release date in CHANGELOG.md.
    // Plugins (independent versioning) and staleness_exempt crates are skipped.
    let changelog_path = workspace_root.join("CHANGELOG.md");
    // Cache-aware: use cached git paths on hit; run git and cache the result on miss.
    let mut touched_paths_for_cache: Option<Vec<String>> = None;
    if let Some((release_version, release_date)) = parse_release_date(&changelog_path) {
        report.staleness_since = format!("v{release_version} ({release_date})");
        let touched_from_cache = cached
            .as_ref()
            .and_then(|c| c.git_touched_paths.as_ref())
            .map(|paths| paths.iter().cloned().collect::<HashSet<String>>());
        let touched_result =
            touched_from_cache.or_else(|| git_paths_touched_since(&workspace_root, &release_date));
        if let Some(ref touched_paths) = touched_result {
            touched_paths_for_cache = Some(touched_paths.iter().cloned().collect());
        }
        if let Some(ref touched) = touched_result {
            for pkg in metadata_full.workspace_packages() {
                let name = pkg.name.as_str();
                let entry = match layers.crates.get(name) {
                    Some(e) => e,
                    None => continue,
                };
                if entry.staleness_exempt || entry.kind == "plugin" {
                    continue;
                }
                let manifest_path = Path::new(pkg.manifest_path.as_str());
                let Some(rel_dir) = manifest_parent_rel_to_repo(&workspace_root, manifest_path)
                else {
                    continue;
                };
                let touched_this_crate = touched
                    .iter()
                    .any(|p| git_path_touches_crate_root(p, &rel_dir));
                if !touched_this_crate {
                    report.staleness_warns.push((
                        name.to_string(),
                        format!("no commits touching crate on/after {release_date}"),
                    ));
                }
            }
        } else {
            // vox-arch-check: allow git-exec — batched log failed; fall back to per-crate probe.
            for pkg in metadata_full.workspace_packages() {
                let name = pkg.name.as_str();
                let entry = match layers.crates.get(name) {
                    Some(e) => e,
                    None => continue,
                };
                if entry.staleness_exempt || entry.kind == "plugin" {
                    continue;
                }
                let manifest_dir = Path::new(pkg.manifest_path.as_str())
                    .parent()
                    .unwrap_or(Path::new("."));
                if let Some(last_commit) = git_last_commit_date(manifest_dir) {
                    if last_commit < release_date {
                        report.staleness_warns.push((name.to_string(), last_commit));
                    }
                }
            }
        }
        report.staleness_warns.sort();
    }

    // Persist cache on miss so next run is faster (non-fatal on failure).
    if let (Some(key), None) = (&cache_key, &cached) {
        let data = cache::CachedData {
            key: key.clone(),
            git_touched_paths: touched_paths_for_cache,
        };
        let _ = cache::store(&workspace_root, &data);
    }

    prof("rule 8 (staleness/git, with cache)", &mut prof_last);
    // ── Rule 9: Generated-file drift ──
    report.generated_file_drift_warns = check_generated_file_drift(&workspace_root, &prune_dirs)
        .unwrap_or_else(|e| {
            eprintln!("warn: generated-file-drift check skipped: {e:#}");
            Vec::new()
        });

    prof("rule 9 (generated-file drift)", &mut prof_last);
    // ── Rule 10: Forbidden direct dependencies ──
    if !layers.forbidden_deps.is_empty() {
        let forbidden_set: Vec<(&str, Vec<&str>)> = layers
            .forbidden_deps
            .iter()
            .map(|r| {
                (
                    r.krate.as_str(),
                    r.forbidden.iter().map(|s| s.as_str()).collect(),
                )
            })
            .collect();
        for pkg in metadata_full.workspace_packages() {
            let krate_name = pkg.name.as_str();
            let rules_for_crate: Vec<&Vec<&str>> = forbidden_set
                .iter()
                .filter(|(k, _)| *k == krate_name)
                .map(|(_, f)| f)
                .collect();
            if rules_for_crate.is_empty() {
                continue;
            }
            for dep in &pkg.dependencies {
                let dep_name = dep.name.as_str();
                for forbidden_list in &rules_for_crate {
                    if forbidden_list.contains(&dep_name) {
                        report
                            .forbidden_dep_violations
                            .push((krate_name.to_string(), dep_name.to_string()));
                    }
                }
            }
        }
        report.forbidden_dep_violations.sort();
        report.forbidden_dep_violations.dedup();
    }

    prof("rule 10 (forbidden deps)", &mut prof_last);
    // ── Rule 11: Forbidden code patterns (P3-T7) ──
    // Single batched walk of the workspace; all 9 patterns evaluated per file.
    if !layers.forbidden_pattern.is_empty() {
        match scan_forbidden_patterns_all(&workspace_root, &layers.forbidden_pattern, &prune_dirs) {
            Ok(hits) => {
                for hit in hits {
                    report.forbidden_pattern_hits.push((
                        hit.rule,
                        hit.file,
                        hit.line,
                        hit.matched,
                        hit.reason,
                    ));
                }
            }
            Err(e) => {
                eprintln!("warn: forbidden_pattern scan skipped: {e:#}");
            }
        }
        report
            .forbidden_pattern_hits
            .sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));
    }

    prof("rule 11 (forbidden patterns, batched)", &mut prof_last);
    // ── Rule 12: WTL / layers.toml / disk three-way parity ──
    report.wtl_parity_warns = check_wtl_parity(&layers, &workspace_root);

    prof("rule 12 (WTL parity)", &mut prof_last);
    // ── Rule 13: LoC delta regression ──
    // Warn if any budgeted crate has grown >15% vs. the last tagged release.
    // Only fires for crates >2000 LoC to avoid noise from tiny utilities.
    if let Some((release_version, _)) = parse_release_date(&workspace_root.join("CHANGELOG.md")) {
        let tag = format!("v{release_version}");
        // First pass: collect (name, manifest_dir, current_loc) for budgeted crates
        // whose current LoC is above the 2000-line floor.
        let mut candidates: Vec<(String, PathBuf, usize)> = Vec::new();
        for pkg in metadata_full.workspace_packages() {
            let name = pkg.name.as_str();
            let entry = match layers.crates.get(name) {
                Some(e) => e,
                None => continue,
            };
            if entry.max_loc.is_none() {
                continue;
            }
            let manifest_dir = Path::new(pkg.manifest_path.as_str())
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            let src_dir = manifest_dir.join("src");
            let current_loc = count_loc(&src_dir, &prune_dirs).unwrap_or(0);
            if current_loc < 2000 {
                continue;
            }
            candidates.push((name.to_string(), manifest_dir, current_loc));
        }
        // Second pass: fetch baseline LoC for ALL candidates in a single
        // `git cat-file --batch` invocation, then compute deltas.
        let manifest_dirs: Vec<PathBuf> = candidates.iter().map(|(_, m, _)| m.clone()).collect();
        if let Some(baselines) = git_loc_at_tag_batch(&tag, &workspace_root, &manifest_dirs) {
            for (name, manifest_dir, current_loc) in &candidates {
                let Some(&baseline) = baselines.get(manifest_dir) else {
                    continue;
                };
                if baseline == 0 {
                    continue;
                }
                let growth = (*current_loc as f64 - baseline as f64) / baseline as f64 * 100.0;
                if growth > 15.0 {
                    report
                        .loc_delta_warns
                        .push((name.clone(), *current_loc, baseline, growth));
                }
            }
        }
        report
            .loc_delta_warns
            .sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
    }

    prof("rule 13 (LoC delta, per-crate git show)", &mut prof_last);
    // ── Rule 14: No cdylib-as-normal-dep ──
    // Plugin cdylibs are loaded dynamically at runtime via vox-plugin-host.
    // Linking them statically as a normal compile-time dep breaks the plugin
    // boundary. Only non-optional, non-dev deps are checked.
    {
        let cdylib_pkg_names: HashSet<&str> = metadata_full
            .workspace_packages()
            .iter()
            .filter(|p| {
                p.targets
                    .iter()
                    .any(|t| t.kind.iter().any(|k| k == "cdylib"))
            })
            .map(|p| p.name.as_str())
            .collect();

        if !cdylib_pkg_names.is_empty() {
            for pkg in metadata_full.workspace_packages() {
                if pkg
                    .targets
                    .iter()
                    .any(|t| t.kind.iter().any(|k| k == "cdylib"))
                {
                    continue; // cdylib can depend on another cdylib (rare but not our concern here)
                }
                for dep in &pkg.dependencies {
                    if dep.kind != cargo_metadata::DependencyKind::Normal || dep.optional {
                        continue;
                    }
                    if cdylib_pkg_names.contains(dep.name.as_str()) {
                        report
                            .cdylib_dep_warns
                            .push((pkg.name.clone(), dep.name.clone()));
                    }
                }
            }
            report.cdylib_dep_warns.sort();
            report.cdylib_dep_warns.dedup();
        }
    }

    prof("rule 14 (cdylib dep)", &mut prof_last);
    // ── Rule 15: Workspace-dep budget ──
    for (name, entry) in &layers.crates {
        if let Some(budget) = entry.max_workspace_deps {
            let count = workspace_dep_count.get(name).copied().unwrap_or(0);
            if count > budget {
                report
                    .workspace_dep_warns
                    .push((name.clone(), count, budget));
            }
        }
    }
    report.workspace_dep_warns.sort_by(|a, b| b.1.cmp(&a.1));
    prof("rule 15 (workspace-dep budget)", &mut prof_last);
    if profile_on {
        eprintln!("[profile] TOTAL: {}ms", profile_start.elapsed().as_millis());
    }

    // ── Rule 14: evidence-ledger integrity ─────────────────────────────────
    // Runs against `contracts/reports/evidence-ledger.v1.json`; emits
    // findings about missing/stale artifacts. Defaults to warn-only — the
    // strict flip happens once block-GA gates are all met (per the honest
    // plan §10). The lint is `strict_evidence_ledger = false` until then.
    if workspace_root
        .join("contracts/reports/evidence-ledger.v1.json")
        .exists()
    {
        match evidence_ledger::check_evidence_ledger(&workspace_root) {
            Ok(findings) => {
                report.evidence_findings = findings;
            }
            Err(e) => {
                eprintln!("[evidence-ledger] WARN: failed to load ledger: {e:#}");
            }
        }
    }
    // Default strictness: false. Tighten via `--strict-evidence` flag once
    // §10 acceptance fires.
    report.strict_evidence_ledger = std::env::args().any(|a| a == "--strict-evidence");

    Ok(report)
}

/// Count lines in a byte slice with the same semantics as
/// `std::str::Lines::count()`: lines are split on `\n` (or `\r\n`), and a
/// trailing newline does NOT add an empty trailing line.
///
/// Used by Rule 13 to match what the original per-file `git show` + `lines().count()`
/// produced, now that we read blobs directly via `git cat-file --batch`.
fn count_lines_in_bytes(body: &[u8]) -> usize {
    if body.is_empty() {
        return 0;
    }
    let nl = body.iter().filter(|&&b| b == b'\n').count();
    if body.last() == Some(&b'\n') {
        nl
    } else {
        nl + 1
    }
}

/// Rule 13 helper — count LoC in every budgeted crate's `src/` tree at the given
/// git tag, using a single `git cat-file --batch` invocation.
///
/// Previously this was one `git show` *per file* per crate (potentially 900+
/// process spawns on this workspace). The batched approach uses two git
/// invocations total: one `git ls-tree -r` to enumerate paths at the tag, and
/// one `git cat-file --batch` fed all blob refs on stdin.
///
/// Returns `None` if git is unavailable or the tag doesn't exist; otherwise a
/// map from `manifest_dir` (the crate's directory) to its LoC at `tag`.
/// Crates whose `src/` had no `.rs` files at `tag` are absent from the map.
fn git_loc_at_tag_batch(
    tag: &str,
    workspace_root: &Path,
    manifest_dirs: &[PathBuf],
) -> Option<HashMap<PathBuf, usize>> {
    if manifest_dirs.is_empty() {
        return Some(HashMap::new());
    }
    // Build a {src_rel_path → manifest_dir} index so we can attribute lines.
    let mut src_to_manifest: HashMap<String, PathBuf> = HashMap::new();
    for md in manifest_dirs {
        let rel_src = md.join("src");
        if let Ok(stripped) = rel_src.strip_prefix(workspace_root) {
            if let Some(s) = stripped.to_str() {
                src_to_manifest.insert(s.replace('\\', "/"), md.clone());
            }
        }
    }
    if src_to_manifest.is_empty() {
        return None;
    }

    // 1) Single `git ls-tree -r --name-only <tag>` over the whole tree.
    // vox-arch-check: allow git-exec
    let ls = Command::new("git")
        .args(["ls-tree", "-r", "--name-only", tag])
        .current_dir(workspace_root)
        .output()
        .ok()?;
    if !ls.status.success() {
        return None;
    }

    // 2) Collect `.rs` paths whose prefix matches a budgeted src dir.
    let mut blob_refs: Vec<String> = Vec::new();
    let mut blob_to_manifest: Vec<PathBuf> = Vec::new();
    for line in String::from_utf8_lossy(&ls.stdout).lines() {
        if !line.ends_with(".rs") {
            continue;
        }
        // Find longest matching src dir prefix.
        let mut owner: Option<&PathBuf> = None;
        for (src_rel, md) in &src_to_manifest {
            let prefix = format!("{src_rel}/");
            if line.starts_with(&prefix) {
                owner = Some(md);
                break;
            }
        }
        if let Some(md) = owner {
            blob_refs.push(format!("{tag}:{line}"));
            blob_to_manifest.push(md.clone());
        }
    }
    if blob_refs.is_empty() {
        return Some(HashMap::new());
    }

    // 3) One `git cat-file --batch`, feed all refs on stdin.
    // Drain stdout in a separate thread to avoid a deadlock when git's stdout
    // pipe buffer fills up faster than we can drain it (Windows pipes are ~4KB).
    // vox-arch-check: allow git-exec
    use std::io::{Read, Write};
    let mut child = Command::new("git")
        .args(["cat-file", "--batch"])
        .current_dir(workspace_root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    let mut stdin = child.stdin.take()?;
    let mut stdout = child.stdout.take()?;
    let blob_refs_for_thread = blob_refs.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        for r in &blob_refs_for_thread {
            stdin.write_all(r.as_bytes())?;
            stdin.write_all(b"\n")?;
        }
        // Drop closes stdin → signals EOF to git
        Ok(())
    });

    let mut buf = Vec::new();
    stdout.read_to_end(&mut buf).ok()?;
    writer.join().ok()?.ok()?;
    let _ = child.wait();

    // 4) Parse output: each blob is `<sha> blob <size>\n<size bytes>\n`.
    // Count newlines within each blob's body and attribute to its manifest_dir.
    let mut result: HashMap<PathBuf, usize> = HashMap::new();
    let mut idx = 0usize;
    let mut blob_i = 0usize;
    while idx < buf.len() && blob_i < blob_to_manifest.len() {
        // Read header line up to '\n'
        let nl = match buf[idx..].iter().position(|&b| b == b'\n') {
            Some(p) => idx + p,
            None => break,
        };
        let header = match std::str::from_utf8(&buf[idx..nl]) {
            Ok(s) => s,
            Err(_) => break,
        };
        idx = nl + 1;
        // Header is like "<sha> blob <size>" or "<ref> missing"
        let parts: Vec<&str> = header.split_whitespace().collect();
        if parts.len() == 3 && parts[1] == "blob" {
            let size: usize = parts[2].parse().ok()?;
            let body_end = (idx + size).min(buf.len());
            let lines = count_lines_in_bytes(&buf[idx..body_end]);
            let md = &blob_to_manifest[blob_i];
            *result.entry(md.clone()).or_insert(0) += lines;
            idx = body_end + 1; // skip trailing '\n' after blob body
        }
        blob_i += 1;
    }
    Some(result)
}

/// Rule 12 — three-way parity between `[crates]` in layers.toml, the
/// `crates/` directory on disk, and `where-things-live.md`.
///
/// Two directions are checked:
///
/// (a) Every `[crates]` entry in `layers.toml` whose `crates/<name>/`
///     directory does not exist on disk, unless the name is in `[planned]`.
/// (b) Every `crates/<name>/` occurrence in `where-things-live.md` whose
///     directory does not exist on disk, unless `<name>` is in `[planned]`
///     or in `[crates]` (the latter is already flagged by direction a).
fn check_wtl_parity(cfg: &LayersConfig, workspace_root: &Path) -> Vec<String> {
    use regex::Regex;

    let mut warns = Vec::new();
    let crates_dir = workspace_root.join("crates");

    // (a) layers.toml [crates] entries without a matching directory
    for name in cfg.crates.keys() {
        if name == "workspace-hack" {
            continue;
        }
        if !crates_dir.join(name).exists() && !cfg.planned.contains_key(name.as_str()) {
            warns.push(format!(
                "layers.toml [crates] has `{name}` but `crates/{name}/` does not exist on disk \
                 (add to [planned] if intended, or remove the entry)"
            ));
        }
    }

    // (b) where-things-live.md crate references without a matching directory
    let wtl_path = workspace_root.join("docs/src/architecture/where-things-live.md");
    if let Ok(wtl_body) = std::fs::read_to_string(&wtl_path) {
        // Match `crates/<name>/` patterns (name = alphanumeric + hyphens)
        let re = match Regex::new(r"crates/([a-zA-Z][a-zA-Z0-9-]+)/") {
            Ok(r) => r,
            Err(_) => return warns,
        };
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for cap in re.captures_iter(&wtl_body) {
            let cname = &cap[1];
            if !seen.insert(cname.to_string()) {
                continue;
            }
            if crates_dir.join(cname).exists() {
                continue;
            }
            if cfg.planned.contains_key(cname) {
                continue;
            }
            // Also skip if the layers.toml [crates] entry already covers it (direction a)
            if cfg.crates.contains_key(cname) {
                continue;
            }
            warns.push(format!(
                "where-things-live.md references `crates/{cname}/` but the directory does not \
                 exist and `{cname}` is not in [planned] (move row to 'Planned but not landed' \
                 section and add a [planned] entry)"
            ));
        }
    }

    warns.sort();
    warns
}

/// Return the YYYY-MM-DD **author** date of the last commit touching `dir`, or `None` if git is unavailable.
/// Uses author date so it matches `git log --since` filtering used by Rule 8 batching.
fn git_last_commit_date(dir: &Path) -> Option<String> {
    // vox-arch-check: allow git-exec
    let out = Command::new("git")
        .args(["log", "-n", "1", "--format=%ad", "--date=short"])
        .arg("--")
        .arg(dir)
        .output()
        .ok()?;
    let s = String::from_utf8(out.stdout).ok()?;
    let date = s.trim().to_string();
    // Expect exactly "YYYY-MM-DD" (10 chars); ignore empty output (no commits touching dir).
    if date.len() == 10 { Some(date) } else { None }
}

/// Parse the most recent released version and its date from `CHANGELOG.md`.
///
/// Looks for lines matching `## [X.Y.Z] - YYYY-MM-DD`, skipping `[Unreleased]`.
/// Returns `(version, date)` of the first match, or `None` if the file is absent
/// or has no released entries yet.
fn parse_release_date(changelog: &Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(changelog).ok()?;
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("## [") || t.contains("Unreleased") {
            continue;
        }
        // "## [0.5.0] - 2026-04-18"
        let inner = t.strip_prefix("## [")?;
        let ver_end = inner.find(']')?;
        let version = inner[..ver_end].to_string();
        let rest = inner[ver_end..].strip_prefix("] - ")?;
        let date = rest.trim();
        if date.len() == 10 && date.as_bytes()[4] == b'-' {
            return Some((version, date.to_string()));
        }
    }
    None
}

/// Warn (or fail) if a workspace member at L1+ has no `description` field
/// in its Cargo.toml or has one shorter than 40 characters. Binary-only
/// crates (`kind = "binary"`) and `workspace-hack` are exempt.
fn check_description_present(meta: &cargo_metadata::Metadata, cfg: &LayersConfig) -> Vec<String> {
    let mut findings = Vec::new();
    let workspace_ids: HashSet<&cargo_metadata::PackageId> =
        meta.workspace_members.iter().collect();
    for pkg in meta
        .packages
        .iter()
        .filter(|p| workspace_ids.contains(&p.id))
    {
        let name = pkg.name.as_str();
        let Some(entry) = cfg.crates.get(name) else {
            continue;
        };
        if entry.layer < 1 {
            continue;
        }
        if entry.kind == "binary" {
            continue;
        }
        if name == "workspace-hack" {
            continue;
        }
        let desc = pkg.description.as_deref().unwrap_or("");
        if desc.len() < 40 {
            findings.push(format!(
                "{}: Cargo.toml description is missing or shorter than 40 chars (\"{}\")",
                name, desc,
            ));
        }
    }
    findings.sort();
    findings
}

/// Warn if a workspace member is not mentioned in
/// `docs/src/architecture/where-things-live.md`.
fn check_where_things_live_coverage(
    meta: &cargo_metadata::Metadata,
    cfg: &LayersConfig,
    repo_root: &std::path::Path,
) -> anyhow::Result<Vec<String>> {
    let path = repo_root.join("docs/src/architecture/where-things-live.md");
    let body =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut findings = Vec::new();
    let workspace_ids: HashSet<&cargo_metadata::PackageId> =
        meta.workspace_members.iter().collect();
    for pkg in meta
        .packages
        .iter()
        .filter(|p| workspace_ids.contains(&p.id))
    {
        let name = pkg.name.as_str();
        if !cfg.crates.contains_key(name) {
            continue;
        }
        if name == "workspace-hack" {
            continue;
        }
        let needle = format!("crates/{}/", name);
        if !body.contains(&needle) {
            findings.push(format!(
                "{}: not listed in where-things-live.md (no `{}` substring)",
                name, needle,
            ));
        }
    }
    findings.sort();
    Ok(findings)
}

/// Count non-blank, non-comment-only lines under `dir/**/*.rs` (best-effort).
fn count_loc(dir: &Path, prune_dir_names: &HashSet<String>) -> Result<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut total = 0usize;
    for entry in walk_repo_files(dir, prune_dir_names) {
        if entry.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&entry) {
            total += content.lines().count();
        }
    }
    Ok(total)
}

/// FNV-1a 64-bit hash of `bytes` — stable across Rust versions, no extra deps.
fn fnv1a_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 14695981039346656037u64;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(1099511628211u64);
    }
    format!("{hash:016x}")
}

/// Check for files that declare `@generated-hash <hex>` in their first five lines but
/// whose content (with that header line blanked out) no longer matches the recorded hash.
///
/// The header format is flexible: any line containing `@generated-hash ` followed by a
/// 16-character hex string is treated as the marker, regardless of comment prefix
/// (`//`, `#`, `<!--`, etc.).
fn check_generated_file_drift(
    workspace_root: &Path,
    prune_dir_names: &HashSet<String>,
) -> anyhow::Result<Vec<(PathBuf, String, String)>> {
    const MARKER: &str = "@generated-hash ";
    const HASH_LEN: usize = 16;
    // Extensions that may carry generated-hash headers.
    const TRACKED_EXTS: &[&str] = &["rs", "ts", "tsx", "js", "vox", "md", "toml", "json"];

    let mut warns = Vec::new();

    for path in walk_repo_files(workspace_root, prune_dir_names) {
        let rel = match path.strip_prefix(workspace_root) {
            Ok(r) => r,
            Err(_) => path.as_path(),
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !TRACKED_EXTS.contains(&ext) {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        // Only scan the first five lines for the marker.
        let mut header_line_idx: Option<usize> = None;
        let mut recorded_hash = String::new();
        for (i, line) in content.lines().enumerate().take(5) {
            if let Some(pos) = line.find(MARKER) {
                let after = &line[pos + MARKER.len()..];
                let candidate: &str = after.split_whitespace().next().unwrap_or("");
                if candidate.len() == HASH_LEN && candidate.chars().all(|c| c.is_ascii_hexdigit()) {
                    header_line_idx = Some(i);
                    recorded_hash = candidate.to_string();
                    break;
                }
            }
        }

        let Some(marker_line) = header_line_idx else {
            continue;
        };

        // Recompute hash over file content with the marker line blanked.
        let body_for_hash: String = content
            .lines()
            .enumerate()
            .map(|(i, line)| if i == marker_line { "" } else { line })
            .collect::<Vec<_>>()
            .join("\n");
        let actual_hash = fnv1a_hex(body_for_hash.as_bytes());

        if actual_hash != recorded_hash {
            warns.push((rel.to_path_buf(), recorded_hash, actual_hash));
        }
    }

    warns.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(warns)
}

#[cfg(test)]
mod walk_and_staleness_tests {
    use super::*;

    #[test]
    fn walk_prune_skips_target_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("crates/foo/src")).unwrap();
        std::fs::write(root.join("crates/foo/src/lib.rs"), "x").unwrap();
        std::fs::create_dir_all(root.join("crates/foo/target/debug")).unwrap();
        std::fs::write(root.join("crates/foo/target/debug/huge.bin"), [0u8; 4096]).unwrap();
        let prune = built_in_walk_prune_names();
        let files: Vec<_> = walk_repo_files(root, &prune)
            .into_iter()
            .map(|p| p.strip_prefix(root).unwrap().to_path_buf())
            .collect();
        assert!(
            files
                .iter()
                .any(|p| p == Path::new("crates/foo/src/lib.rs")),
            "{files:?}"
        );
        assert!(
            !files.iter().any(|p| p.to_string_lossy().contains("target")),
            "must not descend into target/: {files:?}"
        );
    }

    /// Locks in the Rule 13 line-count semantics. The batched
    /// `git cat-file --batch` reader counts lines manually from raw bytes, so
    /// we explicitly assert it matches `str::lines().count()` for every edge
    /// case that mattered in the original per-file `git show` path.
    #[test]
    fn count_lines_in_bytes_matches_str_lines() {
        for s in &[
            "",
            "\n",
            "foo",
            "foo\n",
            "foo\nbar",
            "foo\nbar\n",
            "foo\nbar\nbaz\n",
            "\n\n\n",
            "a\r\nb",
            "a\r\nb\r\n",
        ] {
            assert_eq!(
                count_lines_in_bytes(s.as_bytes()),
                s.lines().count(),
                "line count mismatch for {s:?}"
            );
        }
    }

    #[test]
    fn walk_prune_extra_from_layers_toml() {
        let cfg: LayersConfig = toml::from_str(
            r#"
[crates.dummy]
layer = 0
[arch_check.walk_prune]
extra_skip_dir_names = ["my_vendor"]
"#,
        )
        .expect("parse minimal layers");
        let prune = walk_prune_dir_names(&cfg);
        assert!(prune.contains("target"));
        assert!(prune.contains("my_vendor"));
    }

    #[test]
    fn git_path_touches_crate_root_prefix() {
        assert!(git_path_touches_crate_root(
            "crates/vox-cli/src/main.rs",
            "crates/vox-cli"
        ));
        assert!(git_path_touches_crate_root(
            "crates/vox-cli/Cargo.toml",
            "crates/vox-cli"
        ));
        assert!(!git_path_touches_crate_root(
            "crates/vox-other/src/lib.rs",
            "crates/vox-cli"
        ));
    }

    #[test]
    fn manifest_parent_rel_to_repo_normalizes() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let mf = repo.join("crates/foo/Cargo.toml");
        assert_eq!(
            manifest_parent_rel_to_repo(repo, &mf).as_deref(),
            Some("crates/foo")
        );
    }
}
