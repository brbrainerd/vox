//! Per-run flags (canary rollout, tests policy, prelude allowlist, feature flags) set by [`crate::engine::ToestubEngine`] before scanning.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// `crates/<workspace-member>/` key from a source path, if present.
pub(crate) fn workspace_crate_key(path: &Path) -> Option<String> {
    let s = path.to_string_lossy().replace('\\', "/");
    let parts: Vec<&str> = s.split('/').collect();
    for (i, seg) in parts.iter().enumerate() {
        if *seg == "crates" && i + 1 < parts.len() {
            return Some(parts[i + 1].to_string());
        }
    }
    None
}

/// How TOESTUB treats Rust files under `tests/` directories (integration / nested test trees).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ToestubTestsMode {
    /// Skip unresolved-ref (and similar single-file-noisy) scans under `.../tests/...` (historical default).
    #[default]
    Off,
    /// Include test trees at normal severity (audit noise).
    Include,
    /// Include test trees; policy hooks may escalate later (reserved).
    Strict,
}

/// When set to a non-empty list, enhanced detector paths apply only under `crates/<name>/`.
/// `None` or empty list ⇒ unrestricted (full rollout).
#[derive(Debug, Clone, Default)]
pub struct RunContext {
    pub canary_crates: Option<Vec<String>>,
    pub tests_mode: ToestubTestsMode,
    /// Extra idents treated as "well-known" for unresolved-ref (from `prelude-allowlist` contract).
    pub prelude_allow_idents: HashSet<String>,
    /// Feature flags for staged rollout (`J*`): e.g. `unwired-graph`, `scaling-ast-only`.
    pub feature_flags: HashSet<String>,
    /// Per-callee occurrence counts this run (for hotlist / diagnostics).
    pub(crate) unresolved_callee_counts: HashMap<String, usize>,
    /// For each workspace member under `crates/<name>/`, module names referenced as `crate::<name>` anywhere in that crate's scanned Rust sources (cross-file wiring for the `unwired/module` rule).
    pub workspace_crate_mod_refs: HashMap<String, HashSet<String>>,
    /// Fast pool of all words in the current crate (for skeleton reachability heuristics).
    pub workspace_crate_words: HashMap<String, HashSet<String>>,
}

static RUN_STATE: OnceLock<Mutex<RunContext>> = OnceLock::new();

fn state() -> &'static Mutex<RunContext> {
    RUN_STATE.get_or_init(|| Mutex::new(RunContext::default()))
}

/// Replace global run context for the duration of one [`crate::ToestubEngine::run`].
pub fn init(ctx: RunContext) {
    *state().lock().expect("run_context lock") = ctx;
}

/// Reset to defaults (test isolation).
pub fn reset() {
    *state().lock().expect("run_context lock") = RunContext::default();
}

/// Ensures [`reset`] runs when a scan finishes (including on panic after unwind).
pub struct RunContextGuard;

impl RunContextGuard {
    pub fn new(ctx: RunContext) -> Self {
        init(ctx);
        Self
    }
}

impl Drop for RunContextGuard {
    fn drop(&mut self) {
        reset();
    }
}

fn path_in_any_listed_crate(path: &Path, crates: &[String]) -> bool {
    let s = path.to_string_lossy().replace('\\', "/");
    crates.iter().any(|c| {
        let c = c.trim();
        !c.is_empty()
            && (s.contains(&format!("/crates/{c}/"))
                || s.contains(&format!("crates/{c}/"))
                || s.starts_with(&format!("crates/{c}/")))
    })
}

/// Enhanced unresolved-ref (AST-backed corroboration) applies to this file path.
pub fn enhanced_unresolved_for_path(path: &Path) -> bool {
    let g = state().lock().expect("run_context lock");
    match &g.canary_crates {
        None => true,
        Some(crates) if crates.is_empty() => true,
        Some(crates) => path_in_any_listed_crate(path, crates),
    }
}

pub fn tests_mode() -> ToestubTestsMode {
    state().lock().expect("run_context lock").tests_mode
}

/// True when `path` is under a `tests/` directory segment (integration tests or nested `src/.../tests/`).
pub fn path_under_tests_directory(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .contains("/tests/")
}

/// Unresolved-ref should skip this file entirely based on tests policy.
pub fn skip_unresolved_for_tests_path(path: &Path) -> bool {
    path_under_tests_directory(path) && tests_mode() == ToestubTestsMode::Off
}

pub fn prelude_allowlist_contains(name: &str) -> bool {
    state()
        .lock()
        .expect("run_context lock")
        .prelude_allow_idents
        .contains(name)
}

pub fn feature_enabled(flag: &str) -> bool {
    state()
        .lock()
        .expect("run_context lock")
        .feature_flags
        .contains(flag)
}

/// True if any scanned Rust file in the same workspace crate references `crate::<mod_name>::…` / `crate::<mod_name>;` / `use crate::<mod_name>`.
pub fn workspace_crate_refs_mod(declaring_file: &Path, mod_name: &str) -> bool {
    let Some(key) = workspace_crate_key(declaring_file) else {
        return false;
    };
    let g = state().lock().expect("run_context lock");
    g.workspace_crate_mod_refs
        .get(&key)
        .is_some_and(|s| s.contains(mod_name))
}

/// True if any scanned Rust file in the same workspace crate contains the given word.
pub fn workspace_crate_contains_word(declaring_file: &Path, word: &str) -> bool {
    let Some(key) = workspace_crate_key(declaring_file) else {
        return false;
    };
    let g = state().lock().expect("run_context lock");
    g.workspace_crate_words
        .get(&key)
        .is_some_and(|s| s.contains(word))
}

/// Record an unresolved callee for hotlist telemetry (best-effort).
pub fn record_unresolved_callee(name: &str) {
    let mut g = state().lock().expect("run_context lock");
    *g.unresolved_callee_counts
        .entry(name.to_string())
        .or_insert(0) += 1;
}

pub fn unresolved_callee_counts_snapshot() -> HashMap<String, usize> {
    state()
        .lock()
        .expect("run_context lock")
        .unresolved_callee_counts
        .clone()
}
