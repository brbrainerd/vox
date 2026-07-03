//! `vox ci harness-trust-guard` — T2.4 regression guard for "one daemon, one
//! state owner" (see
//! `docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md`
//! Phase 2 / the T2.4 exemption appendix, which this guard's allowlist
//! mirrors).
//!
//! Checks (all narrow, source-grep-based — no cargo build required):
//!
//! 1. No `args.get("user_approval")` read in Rust dispatch paths. This is the
//!    T0.1 HITL-bypass regression check. It is intentionally NOT
//!    re-implemented here: `contracts/documentation/retired-symbols.v1.yaml`'s
//!    `user-approval-arg-bypass` entry already has `scan_rust_source: true`,
//!    so `crates/vox-cli/src/commands/ci/retired_symbol_check.rs` already
//!    scans every `crates/**/*.rs` file for this pattern on every CI run
//!    (`vox ci check-codex-ssot`). Duplicating it here would just be a
//!    second, drifting copy of the same regex. Instead, `vox-cli`'s
//!    `run_body.rs` dispatch for `HarnessTrustGuard` runs
//!    `retired_symbol_check::run` immediately before this crate's [`run`], so
//!    `vox ci harness-trust-guard` is a single command that covers the full
//!    T2.4 checklist without this lower crate owning (or re-implementing)
//!    that pattern — `vox-cli-ci` sits below `vox-cli` in the dependency
//!    graph and cannot call up into it.
//! 2. No `ServerState`/`Orchestrator` construction in `crates/vox-gui/src` or
//!    `crates/vox-cli/src` outside the exemption appendix's allowlist.
//! 3. No `call_daemon` function-call references in `crates/vox-gui/src` (a
//!    bare doc-comment mention of the retired name, like
//!    `commands/daemon.rs`'s module doc narrating what `PersistentDaemon`
//!    replaced, does not count — see `is_call_daemon_call_site`).
//! 4. No active (non-comment) `EmbeddedOrchestratorDriver`,
//!    `build_repo_scoped_orchestrator_cli`, or
//!    `build_repo_scoped_orchestrator_for_repository` usages in
//!    `crates/vox-cli/src`.
//! 5. `setInterval`-based orchestrator-status polling in
//!    `crates/vox-gui/ui/src`: deliberately a NO-OP today (T3.1, a later
//!    task, removes the fallback poller entirely — see
//!    `docs/.../vox-axis-harness-reliability-spec-plan-2026-07-02.md` T3.1).
//!    Flipping this check to enforcing is T3.1's acceptance criterion, not
//!    T2.4's; adding a currently-failing check here would break CI before
//!    that work lands. Tracked via the `TODO(T3.1)` marker below.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

/// True when `path`'s filename *shape* matches a `#[cfg(test)] mod tests;`
/// -style separate-file test submodule (e.g. `tests.rs`, `foo_tests.rs`).
/// This is a **necessary but not sufficient** signal on its own — it only
/// narrows which files are even candidates for the (expensive-ish) parent
/// declaration lookup in [`is_declared_cfg_test_submodule`]. Do NOT use this
/// alone to decide exemption: a production file can be named `helper_tests.rs`
/// without being a genuine `#[cfg(test)]`-gated submodule (that was exactly
/// the T2.4 review's proven false-negative — see the module doc / commit
/// history for the injected-violation regression tests below).
fn is_test_submodule_filename_shape(path: &Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    matches!(stem, "tests" | "test") || stem.ends_with("_tests") || stem.ends_with("_test")
}

/// True when `path` (a candidate separate-file test submodule per
/// [`is_test_submodule_filename_shape`]) is *actually* declared as a
/// `#[cfg(test)]`-gated module by its parent file — i.e. the parent contains
/// a `#[cfg(test)]` attribute immediately followed (allowing intervening
/// `#[allow(...)]` / `#[path = "..."]` attributes) by `mod <stem>;` where
/// `<stem>` is this file's module name (or the `#[path = "..."]` target
/// matches this file's name).
///
/// This is the content-aware replacement for the old pure-filename check: a
/// file named `helper_tests.rs` is only exempted from the constructor /
/// `call_daemon` / retired-client scans if some real parent module file
/// genuinely declares it behind `#[cfg(test)]`. A same-named file dropped
/// into a directory whose parent never declares it this way (e.g. a probe
/// file, or any future accidental production file that happens to be named
/// `*_tests.rs`) is NOT exempted and falls through to the normal scan.
///
/// The parent file searched is `<dir>/mod.rs` (submodule of a directory
/// module) or `<parent_dir>.rs` (submodule of a same-named file module, e.g.
/// `foo/tests.rs` declared in `foo.rs` sitting next to the `foo/` dir) —
/// both conventions are used in this repo (see the grep of `mod tests;`
/// sites across `crates/**/*.rs` performed while writing this check).
fn is_declared_cfg_test_submodule(path: &Path) -> bool {
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return false,
    };
    let Some(dir) = path.parent() else {
        return false;
    };
    let dir_name = dir.file_name().and_then(|s| s.to_str()).unwrap_or("");

    let mut candidates = vec![dir.join("mod.rs")];
    if let Some(parent_of_dir) = dir.parent() {
        candidates.push(parent_of_dir.join(format!("{dir_name}.rs")));
    }

    for candidate in candidates {
        if candidate == path {
            continue;
        }
        let Ok(body) = fs::read_to_string(&candidate) else {
            continue;
        };
        if declares_cfg_test_mod(&body, stem, path.file_name().and_then(|s| s.to_str())) {
            return true;
        }
    }
    false
}

/// Scans `body` (a parent module file's source) for a `#[cfg(test)]`
/// attribute followed — allowing up to a few intervening attribute lines
/// (`#[allow(...)]`, `#[path = "..."]`) — by `mod <mod_name>;`. Also honors
/// an explicit `#[path = "<file_name>"]` override between the two, matching
/// against the raw file name rather than the module name.
fn declares_cfg_test_mod(body: &str, mod_name: &str, file_name: Option<&str>) -> bool {
    let lines: Vec<&str> = body.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[cfg(test)]" {
            continue;
        }
        // Look ahead a handful of lines for the `mod <name>;` this attribute
        // gates, allowing intervening attributes (`#[allow(...)]`,
        // `#[path = "..."]`) but not arbitrary other code — if we hit a
        // non-attribute, non-blank, non-comment line first, this
        // `#[cfg(test)]` gates something else (e.g. a fn or inline mod).
        let mut path_override: Option<String> = None;
        for candidate_line in lines.iter().skip(i + 1).take(6) {
            let t = candidate_line.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(rest) = t.strip_prefix("#[path") {
                // #[path = "some_file.rs"]
                if let (Some(start), Some(end)) = (rest.find('"'), rest.rfind('"')) {
                    if end > start {
                        path_override = Some(rest[start + 1..end].to_string());
                    }
                }
                continue;
            }
            if t.starts_with('#') {
                continue;
            }
            let target = format!("mod {mod_name};");
            let target_pub = format!("pub mod {mod_name};");
            let target_pub_crate = format!("pub(crate) mod {mod_name};");
            if t == target || t == target_pub || t == target_pub_crate {
                if let Some(want) = &path_override {
                    if Some(want.as_str()) != file_name {
                        break;
                    }
                }
                return true;
            }
            break;
        }
    }
    false
}

/// Rust source files under `root_rel` (relative to repo root), skipping
/// `target/`, `tests/`, and any `#[cfg(test)]`-annotated content is handled
/// by line-level filtering in the caller — this only collects files.
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let walker = walkdir::WalkDir::new(dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_str().unwrap_or("");
        !(e.file_type().is_dir() && matches!(name, "target" | "tests" | ".git"))
    });
    for entry in walker.filter_map(Result::ok) {
        let p = entry.path();
        if entry.file_type().is_file() && p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p.to_path_buf());
        }
    }
}

fn collect_ts_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let walker = walkdir::WalkDir::new(dir).into_iter().filter_entry(|e| {
        let name = e.file_name().to_str().unwrap_or("");
        !(e.file_type().is_dir() && matches!(name, "node_modules" | "dist" | "target" | ".git"))
    });
    for entry in walker.filter_map(Result::ok) {
        let p = entry.path();
        if entry.file_type().is_file()
            && matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("ts") | Some("tsx")
            )
        {
            out.push(p.to_path_buf());
        }
    }
}

/// True when `line` is a Rust comment line (`//` or block-comment content) —
/// mirrors `retired_symbol_check::should_skip_rust_line`'s convention so the
/// known dead `/* ... */` block in `dei.rs` (T2.3's disabled
/// `run_dei_analyze`) is never flagged.
fn is_comment_line(line: &str) -> bool {
    let t = line.trim_start();
    t.is_empty() || t.starts_with("//") || t.starts_with('*') || t.starts_with("/*")
}

/// True when `path` should be exempted **whole-file** from the constructor /
/// `call_daemon` / retired-client-construction scans because it is a
/// genuine, parent-declared `#[cfg(test)]` separate-file test submodule
/// (e.g. `llm_bridge/model_route_policy/tests.rs`) — content-aware
/// replacement for the old pure-filename `is_test_submodule_filename` check.
///
/// Deliberately two-gated: the filename-shape check
/// ([`is_test_submodule_filename_shape`]) is cheap and narrows candidates;
/// the parent-declaration check ([`is_declared_cfg_test_submodule`]) is what
/// actually proves the file is test-only. A file merely *named* like a test
/// file (e.g. an injected probe `helper_tests.rs` with no real
/// `#[cfg(test)] mod helper_tests;` declaration anywhere) fails the second
/// gate and is NOT exempted — this is precisely the T2.4 review's proven
/// false-negative, now closed.
fn is_exempt_test_submodule(path: &Path) -> bool {
    is_test_submodule_filename_shape(path) && is_declared_cfg_test_submodule(path)
}

/// Result of [`scan_constructor_violations`].
struct ConstructorScanResult {
    violations: Vec<String>,
}

/// Constructor patterns considered a T2.4 violation when found outside the
/// allowlist below. `Orchestrator::new` is included because the appendix's
/// test-file exemptions cover its only current hits.
const CONSTRUCTOR_PATTERNS: &[&str] = &[
    "Orchestrator::new(",
    "ServerState::new_full(",
    "ServerState::new_for_daemon(",
    "build_repo_scoped_orchestrator(",
];

/// Per-file allowlist: (path suffix, reason). A hit in a file whose path ends
/// with the given suffix is exempt file-wide — matches the appendix's
/// itemized exemptions exactly (see the appendix in
/// `vox-axis-harness-reliability-spec-plan-2026-07-02.md`, item 2 "daemon
/// bootstrap" and item 3 "protocol-level-only ServerState::new_full").
const FILE_ALLOWLIST: &[&str] = &[
    // vox-orchestrator-d's own daemon bootstrap: the single state owner.
    "vox-orchestrator-d/src/bin/vox_orchestrator_d.rs",
    // ServerState::new_full's own definition body (not a client call site).
    "vox-orchestrator-mcp/src/server_state.rs",
    // vox mcp's protocol-level-only ServerState::new_full (T2.2).
    "vox-orchestrator-mcp/src/lifecycle.rs",
    // Disclosed pre-existing gap, tracked as a follow-up (T2.4 appendix item 5):
    // `vox stop` still uses a throwaway local Orchestrator for emergency_stop.
    // Fixing this requires a new daemon RPC (backend protocol change), out of
    // scope for this CI-gate task.
    "vox-cli/src/commands/dei.rs",
];

/// Scans `files` for [`CONSTRUCTOR_PATTERNS`] outside [`FILE_ALLOWLIST`].
///
/// Rust-source scan for `#[cfg(test)]` modules: once a line matches
/// `#[cfg(test)]` we treat every subsequent line in the file as test-only.
/// This is intentionally coarse (a whole-file-suffix cutoff rather than
/// brace matching) — every current exemption case (`registry.rs`'s
/// `merged_registry_tests`) has its test module positioned so this is
/// correct, and coarseness fails safe (it can only ever under-flag a genuine
/// violation placed textually after a real `#[cfg(test)]` block in the same
/// file, which is not a pattern used anywhere in this codebase today).
/// Separate-file test submodules (`tests.rs` declared via `#[cfg(test)] mod
/// tests;` in the parent, which therefore carry no `#[cfg(test)]` attribute
/// of their own) are excluded whole-file via [`is_exempt_test_submodule`] —
/// content-aware (parent-declaration-verified), not filename-only.
fn scan_constructor_violations(files: &[PathBuf], repo_root: &Path) -> ConstructorScanResult {
    let mut violations = Vec::new();
    for path in files {
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if FILE_ALLOWLIST.iter().any(|sfx| rel.ends_with(sfx)) {
            continue;
        }
        if is_exempt_test_submodule(path) {
            continue;
        }
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        let mut in_test_cfg = false;
        for (idx, line) in body.lines().enumerate() {
            if line.contains("#[cfg(test)]") {
                in_test_cfg = true;
            }
            if in_test_cfg || is_comment_line(line) {
                continue;
            }
            for pat in CONSTRUCTOR_PATTERNS {
                if line.contains(pat) {
                    violations.push(format!(
                        "{}:{}: disallowed constructor `{}` outside the harness-trust-guard \
                         allowlist (see the T2.4 exemption appendix in \
                         docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md)",
                        rel,
                        idx + 1,
                        pat.trim_end_matches('(')
                    ));
                }
            }
        }
    }
    ConstructorScanResult { violations }
}

/// A genuine `call_daemon` function-call reference (`.call_daemon(` or
/// `fn call_daemon`), not a bare textual mention (e.g. a doc comment
/// narrating history, like `commands/daemon.rs`'s module doc: "...talked to a
/// *separate* daemon process spawned per `call_daemon`.").
fn is_call_daemon_call_site(line: &str) -> bool {
    if is_comment_line(line) {
        return false;
    }
    let t = line.trim_start();
    if t.starts_with("///") || t.starts_with("//!") {
        return false;
    }
    line.contains("call_daemon(")
}

fn scan_call_daemon(files: &[PathBuf], repo_root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for path in files {
        if is_exempt_test_submodule(path) {
            continue;
        }
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        for (idx, line) in body.lines().enumerate() {
            if is_call_daemon_call_site(line) {
                let rel = path.strip_prefix(repo_root).unwrap_or(path).display();
                violations.push(format!(
                    "{rel}:{}: `call_daemon` call site found in crates/vox-gui/src — T2.1 \
                     requires all GUI tool/status calls to go through PersistentDaemon + \
                     OrchDaemonClient instead",
                    idx + 1
                ));
            }
        }
    }
    violations
}

/// Retired client constructions (T2.3): `EmbeddedOrchestratorDriver`,
/// `build_repo_scoped_orchestrator_cli`, `build_repo_scoped_orchestrator_for_repository`.
/// Comment-only mentions (the known dead block in `dei.rs`, doc comments in
/// `attention.rs`/`safety.rs` narrating the fix) are not flagged.
const RETIRED_CLIENT_PATTERNS: &[&str] = &[
    "EmbeddedOrchestratorDriver",
    "build_repo_scoped_orchestrator_cli(",
    "build_repo_scoped_orchestrator_for_repository(",
];

fn scan_retired_client_constructions(files: &[PathBuf], repo_root: &Path) -> Vec<String> {
    let mut violations = Vec::new();
    for path in files {
        if is_exempt_test_submodule(path) {
            continue;
        }
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        let mut in_block_comment = false;
        for (idx, line) in body.lines().enumerate() {
            let t = line.trim_start();
            // Track the known `/* ... */` dead-code block convention (dei.rs).
            if t.starts_with("/*") {
                in_block_comment = true;
            }
            let was_in_block_comment = in_block_comment;
            if t.contains("*/") {
                in_block_comment = false;
            }
            if was_in_block_comment || is_comment_line(line) {
                continue;
            }
            for pat in RETIRED_CLIENT_PATTERNS {
                if line.contains(pat) {
                    let rel = path.strip_prefix(repo_root).unwrap_or(path).display();
                    violations.push(format!(
                        "{rel}:{}: retired client construction `{}` — T2.3 requires CLI \
                         commands to route through the daemon client instead (see \
                         OrchestratorDaemonEnsure)",
                        idx + 1,
                        pat.trim_end_matches('(')
                    ));
                }
            }
        }
    }
    violations
}

/// TODO(T3.1): flip this to an enforcing check once T3.1 removes the
/// `setInterval` orchestrator-status polling fallback from
/// `crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts` (see the T2.4
/// exemption appendix item 6). Returns diagnostic-only info today; never
/// fails the guard.
fn scan_setinterval_polling_diagnostic_only(files: &[PathBuf], repo_root: &Path) -> Vec<String> {
    let mut hits = Vec::new();
    for path in files {
        let Ok(body) = fs::read_to_string(path) else {
            continue;
        };
        if body.contains("setInterval") {
            let rel = path.strip_prefix(repo_root).unwrap_or(path).display();
            hits.push(rel.to_string());
        }
    }
    hits
}

/// Checks 2-5 of the module doc (constructor allowlist, `call_daemon`,
/// retired-client-construction, and the diagnostic-only `setInterval` note).
/// Check 1 (`args.get("user_approval")`) is run by the caller — see the
/// module doc's explanation of why it lives in `retired_symbol_check`
/// instead.
pub fn run(repo_root: &Path) -> Result<()> {
    // 2 + 4. Constructor / retired-client-construction sweeps.
    let mut gui_rs = Vec::new();
    collect_rs_files(&repo_root.join("crates/vox-gui/src"), &mut gui_rs);
    let mut cli_rs = Vec::new();
    collect_rs_files(&repo_root.join("crates/vox-cli/src"), &mut cli_rs);
    let mut mcp_rs = Vec::new();
    collect_rs_files(&repo_root.join("crates/vox-orchestrator-mcp/src"), &mut mcp_rs);
    let mut orch_d_rs = Vec::new();
    collect_rs_files(&repo_root.join("crates/vox-orchestrator-d/src"), &mut orch_d_rs);

    let mut constructor_targets = Vec::new();
    constructor_targets.extend(gui_rs.iter().cloned());
    constructor_targets.extend(cli_rs.iter().cloned());
    constructor_targets.extend(mcp_rs.iter().cloned());
    constructor_targets.extend(orch_d_rs.iter().cloned());

    let mut failures = Vec::new();
    failures.extend(scan_constructor_violations(&constructor_targets, repo_root).violations);

    // 3. call_daemon in vox-gui/src.
    failures.extend(scan_call_daemon(&gui_rs, repo_root));

    // 4b. Retired client constructions in vox-cli/src.
    failures.extend(scan_retired_client_constructions(&cli_rs, repo_root));

    // 5. setInterval polling — diagnostic only, never fails (T3.1 will flip this).
    let mut gui_ts = Vec::new();
    collect_ts_files(&repo_root.join("crates/vox-gui/ui/src"), &mut gui_ts);
    let polling_hits = scan_setinterval_polling_diagnostic_only(&gui_ts, repo_root);
    if !polling_hits.is_empty() {
        eprintln!(
            "harness-trust-guard: NOTE (non-blocking, TODO T3.1): setInterval found in {} \
             file(s) under crates/vox-gui/ui/src (expected until T3.1 removes the \
             orchestrator-status polling fallback): {}",
            polling_hits.len(),
            polling_hits.join(", ")
        );
    }

    if !failures.is_empty() {
        for f in &failures {
            eprintln!("{f}");
        }
        return Err(anyhow!(
            "harness-trust-guard: found {} violation(s) — see the T2.4 exemption appendix \
             in docs/src/architecture/vox-axis-harness-reliability-spec-plan-2026-07-02.md \
             for the allowlist, or fix the regression",
            failures.len()
        ));
    }

    println!("harness-trust-guard OK");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both tests below touch the REAL `crates/vox-gui/src` tree (one reads
    /// it clean, the other briefly injects a probe file into it) — this
    /// mutex serializes them against `cargo test`'s default parallel
    /// execution so the probe test's file write/cleanup can never race the
    /// clean-repo test's scan of the same directory.
    static REAL_TREE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn real_repo_root() -> PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    /// `harness-trust-guard` passes clean on the current (fixed) codebase —
    /// the guard's own "no false positives on today's tree" acceptance check.
    #[test]
    fn harness_trust_guard_passes_on_real_repo() {
        let _guard = REAL_TREE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        run(&real_repo_root()).expect("harness-trust-guard must pass on the current repo");
    }

    /// RED test: inject a known violation — a temporary probe file under
    /// `crates/vox-gui/src` containing a disallowed `Orchestrator::new(...)`
    /// construction — and confirm the guard catches it. Mirrors the
    /// probe-and-clean-up pattern used to verify the T0.1 retired-symbol
    /// check: write the probe into the REAL tree (so this exercises the same
    /// `crates/vox-gui/src` walk `run()` uses in production, not a synthetic
    /// fixture directory), assert the guard fails with the probe present,
    /// then remove the probe and assert the guard is clean again — using a
    /// `Drop` guard so the probe is removed even if an assertion panics.
    #[test]
    fn harness_trust_guard_catches_injected_constructor_violation() {
        let _guard = REAL_TREE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = real_repo_root();
        let probe_path = root
            .join("crates/vox-gui/src")
            .join("__harness_trust_guard_probe__.rs");

        struct CleanupProbe(PathBuf);
        impl Drop for CleanupProbe {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        assert!(
            !probe_path.exists(),
            "probe file must not already exist: {}",
            probe_path.display()
        );
        std::fs::write(
            &probe_path,
            "// T2.4 harness-trust-guard self-test probe — must be removed by the test.\n\
             fn probe() {\n    let _orch = Orchestrator::new(Default::default());\n}\n",
        )
        .expect("write probe file");
        let _cleanup = CleanupProbe(probe_path.clone());

        let result = run(&root);
        assert!(
            result.is_err(),
            "harness-trust-guard must fail with the probe's Orchestrator::new(...) present"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("violation"),
            "error message should mention violation(s), got: {msg}"
        );

        drop(_cleanup);
        assert!(!probe_path.exists(), "probe file must be removed after cleanup");
        run(&root).expect("harness-trust-guard must pass again once the probe is removed");
    }

    /// Small helper shared by the three T2.4-follow-up regression tests
    /// below: write `body` to a probe file named like a test submodule
    /// (`*_tests.rs`) under `crates/vox-gui/src`, run the guard both via the
    /// specific lower-level `scan_fn` (for a precise, per-violation-text
    /// assertion) and via the full end-to-end [`run`] (for integration
    /// coverage), then clean up and assert the guard is clean again. This
    /// reproduces the review's exact false-negative shape: production code
    /// with no `#[cfg(test)]` anywhere, sitting in a file whose *name* looks
    /// like a test file but which no parent module ever actually declares
    /// behind `#[cfg(test)]`.
    fn assert_probe_with_test_shaped_name_is_caught(
        probe_dir_rel: &str,
        probe_filename: &str,
        body: &str,
        expect_needle: &str,
        scan_fn: impl Fn(&[PathBuf], &Path) -> Vec<String>,
    ) {
        let _guard = REAL_TREE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let root = real_repo_root();
        let probe_path = root.join(probe_dir_rel).join(probe_filename);

        struct CleanupProbe(PathBuf);
        impl Drop for CleanupProbe {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }

        assert!(
            !probe_path.exists(),
            "probe file must not already exist: {}",
            probe_path.display()
        );
        std::fs::write(&probe_path, body).expect("write probe file");
        let _cleanup = CleanupProbe(probe_path.clone());

        // Sanity: this probe file is NOT declared by any parent module as a
        // `#[cfg(test)]` submodule, so it must not be exempted whole-file.
        assert!(
            !is_declared_cfg_test_submodule(&probe_path),
            "test bug: probe file must not be a genuinely-declared cfg(test) submodule"
        );

        // Precise check: the specific scan function must emit a violation
        // whose text names the pattern it caught.
        let scan_violations = scan_fn(&[probe_path.clone()], &root);
        assert_eq!(
            scan_violations.len(),
            1,
            "expected exactly one violation for probe `{probe_filename}`, got: {scan_violations:?}"
        );
        assert!(
            scan_violations[0].contains(expect_needle),
            "violation should mention `{expect_needle}`, got: {}",
            scan_violations[0]
        );

        // End-to-end check: the full `run()` (which is what `vox ci
        // harness-trust-guard` actually invokes) must also fail with the
        // probe present.
        let result = run(&root);
        assert!(
            result.is_err(),
            "harness-trust-guard must FAIL for production code with no #[cfg(test)] in a \
             test-shaped-name file `{probe_filename}` — a pure filename-based skip would \
             wrongly let this through (T2.4 follow-up regression)"
        );

        drop(_cleanup);
        assert!(!probe_path.exists(), "probe file must be removed after cleanup");
        run(&root).expect("harness-trust-guard must pass again once the probe is removed");
    }

    /// T2.4 review stress test #1: a file named like a test submodule
    /// (`helper_tests.rs`) containing plain production code —
    /// `Orchestrator::new(Default::default())` with zero `#[cfg(test)]`
    /// anywhere — must be caught, not silently skipped because of its name.
    #[test]
    fn harness_trust_guard_catches_constructor_violation_in_test_shaped_filename() {
        assert_probe_with_test_shaped_name_is_caught(
            "crates/vox-gui/src",
            "__t24_probe_helper_tests.rs",
            "// T2.4 follow-up regression probe -- production code, not test-gated.\n\
             pub fn probe() {\n    let _orch = Orchestrator::new(Default::default());\n}\n",
            "disallowed constructor",
            |files, root| scan_constructor_violations(files, root).violations,
        );
    }

    /// T2.4 review stress test #2: a `call_daemon(...)` call site in a
    /// similarly test-shaped-named file must be caught.
    #[test]
    fn harness_trust_guard_catches_call_daemon_in_test_shaped_filename() {
        assert_probe_with_test_shaped_name_is_caught(
            "crates/vox-gui/src",
            "__t24_probe_status_tests.rs",
            "// T2.4 follow-up regression probe -- production code, not test-gated.\n\
             pub async fn probe(cmd: &str) {\n    let _ = call_daemon(cmd).await;\n}\n",
            "call_daemon",
            scan_call_daemon,
        );
    }

    /// T2.4 review stress test #3: a retired
    /// `build_repo_scoped_orchestrator_cli(...)` construction in a
    /// similarly test-shaped-named file must be caught.
    #[test]
    fn harness_trust_guard_catches_retired_client_construction_in_test_shaped_filename() {
        assert_probe_with_test_shaped_name_is_caught(
            "crates/vox-cli/src",
            "__t24_probe_legacy_test.rs",
            "// T2.4 follow-up regression probe -- production code, not test-gated.\n\
             pub fn probe(config: Config) {\n    let _orch = build_repo_scoped_orchestrator_cli(config);\n}\n",
            "retired client construction",
            scan_retired_client_constructions,
        );
    }

    /// Confirms the *legitimate* exemption path still works: the real
    /// `model_route_policy/tests.rs` file (a genuine `#[cfg(test)] mod
    /// tests;`-declared separate-file test submodule with no `#[cfg(test)]`
    /// attribute of its own, and which does contain `Orchestrator::new(...)`
    /// calls in its test bodies) must be recognized as exempt — the
    /// content-aware check must not overcorrect into a false positive here.
    #[test]
    fn is_declared_cfg_test_submodule_recognizes_real_separate_file_submodule() {
        let root = real_repo_root();
        let path = root
            .join("crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs");
        assert!(
            path.exists(),
            "expected real fixture file to exist: {}",
            path.display()
        );
        assert!(
            is_declared_cfg_test_submodule(&path),
            "model_route_policy/tests.rs must be recognized as a genuine cfg(test) submodule"
        );
        assert!(is_exempt_test_submodule(&path));
    }

    #[test]
    fn is_comment_line_detects_line_and_block_comments() {
        assert!(is_comment_line("// vox-dei"));
        assert!(is_comment_line("   // trailing"));
        assert!(is_comment_line("/* block */"));
        assert!(is_comment_line("* continuation"));
        assert!(!is_comment_line(r#"let _ = "vox-dei";"#));
    }

    #[test]
    fn is_call_daemon_call_site_ignores_doc_comment_mentions() {
        assert!(!is_call_daemon_call_site(
            "//! talked to a *separate* daemon process spawned per `call_daemon`."
        ));
        assert!(!is_call_daemon_call_site(
            "/// See the old `call_daemon` helper for history."
        ));
        assert!(is_call_daemon_call_site("    let x = call_daemon(cmd).await?;"));
        assert!(is_call_daemon_call_site("async fn call_daemon(cmd: &str) {"));
    }

    #[test]
    fn scan_constructor_violations_flags_bare_hit() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("probe.rs");
        std::fs::write(&f, "let orch = Orchestrator::new(config);\n").unwrap();
        let result = scan_constructor_violations(&[f.clone()], &dir);
        assert_eq!(result.violations.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_constructor_violations_skips_cfg_test_modules() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-cfgtest-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("probe.rs");
        std::fs::write(
            &f,
            "#[cfg(test)]\nmod tests {\n    let orch = Orchestrator::new(config);\n}\n",
        )
        .unwrap();
        let result = scan_constructor_violations(&[f.clone()], &dir);
        assert!(result.violations.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_retired_client_constructions_skips_block_comments() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-blockcomment-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("probe.rs");
        std::fs::write(
            &f,
            "/*\nlet _orch = build_repo_scoped_orchestrator_cli(config);\n*/\n",
        )
        .unwrap();
        let violations = scan_retired_client_constructions(&[f.clone()], &dir);
        assert!(violations.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn scan_retired_client_constructions_flags_live_call() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-livecall-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("probe.rs");
        std::fs::write(
            &f,
            "let _orch = build_repo_scoped_orchestrator_cli(config);\n",
        )
        .unwrap();
        let violations = scan_retired_client_constructions(&[f.clone()], &dir);
        assert_eq!(violations.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn declares_cfg_test_mod_matches_direct_declaration() {
        let body = "use std::fmt;\n\n#[cfg(test)]\nmod tests;\n\nfn real_code() {}\n";
        assert!(declares_cfg_test_mod(body, "tests", Some("tests.rs")));
        assert!(!declares_cfg_test_mod(body, "other", Some("other.rs")));
    }

    #[test]
    fn declares_cfg_test_mod_matches_through_intervening_attributes() {
        let body = "#[cfg(test)]\n#[allow(unsafe_code)] // tests use set_var under a lock\nmod tests;\n";
        assert!(declares_cfg_test_mod(body, "tests", Some("tests.rs")));
    }

    #[test]
    fn declares_cfg_test_mod_honors_path_override() {
        let body = "#[cfg(test)]\n#[path = \"snap_tests.rs\"]\nmod tests;\n";
        assert!(declares_cfg_test_mod(body, "tests", Some("snap_tests.rs")));
        assert!(!declares_cfg_test_mod(body, "tests", Some("tests.rs")));
    }

    #[test]
    fn declares_cfg_test_mod_rejects_unrelated_cfg_test_attribute() {
        // A #[cfg(test)] that gates something other than `mod <name>;`
        // (e.g. a function) must not be mistaken for a submodule declaration.
        let body = "#[cfg(test)]\nfn helper_for_tests() {}\n";
        assert!(!declares_cfg_test_mod(body, "helper_for_tests", None));
    }

    #[test]
    fn is_declared_cfg_test_submodule_rejects_undeclared_probe_file() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-undeclared-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // No mod.rs / <dirname>.rs parent declares this file at all.
        let f = dir.join("helper_tests.rs");
        std::fs::write(&f, "fn probe() {}\n").unwrap();
        assert!(!is_declared_cfg_test_submodule(&f));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn is_declared_cfg_test_submodule_accepts_real_mod_rs_declaration() {
        let dir = std::env::temp_dir().join(format!(
            "harness-trust-guard-test-declared-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mod.rs"), "#[cfg(test)]\nmod tests;\n").unwrap();
        let f = dir.join("tests.rs");
        std::fs::write(&f, "fn probe() {}\n").unwrap();
        assert!(is_declared_cfg_test_submodule(&f));
        std::fs::remove_dir_all(&dir).ok();
    }
}
