//! Worktree-aware build-artifact GC.
//!
//! Extends `artifact-prune` to (a) clean per-worktree `target/` dirs and
//! (b) remove whole stale worktrees — with one hard rule: **never delete a
//! target that a build is writing into right now.** The safety gate protects a
//! worktree when it is the current one, git-`locked`, has an active build
//! process, or carries uncommitted *source* changes.
//!
//! Pure decision logic ([`decide_target_clean`], [`decide_worktree_remove`]) is
//! split from the IO (git plumbing, `sysinfo` process scan, mtime walk) so the
//! policy is unit-testable without a live workspace.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use sysinfo::System;
use walkdir::WalkDir;

use super::retention::{StaleWorktreesPolicy, WorktreeTargetsPolicy, age_days};

/// Process names that indicate a build/test is in flight for a worktree.
const BUILD_PROC_NEEDLES: &[&str] = &[
    "cargo",
    "rustc",
    "rustdoc",
    "lld-link",
    "cc1",
    "build-script",
    "sccache",
    "vox",
];

/// One worktree from `git worktree list --porcelain`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worktree {
    pub path: PathBuf,
    pub locked: bool,
    /// The primary checkout (first record) — never GC'd here.
    pub is_main: bool,
}

/// One planned GC action (delete or protect), for both audit and prune.
pub struct PlannedItem {
    /// The path acted on (a `target/` dir, an `incremental/` dir, or a whole worktree).
    pub path: PathBuf,
    /// The owning worktree (used to re-verify the active-build gate at execute time).
    pub worktree: PathBuf,
    pub class: &'static str,
    pub bytes: u64,
    pub age_days: u32,
    /// `"delete"` or `"protect"`.
    pub action: &'static str,
    pub reason: String,
}

/// Options threaded from the `artifact-prune` CLI.
#[derive(Debug, Clone, Default)]
pub struct WorktreeGcOpts {
    pub include_worktrees: bool,
    pub remove_stale_worktrees: bool,
    pub include_dirty_targets: bool,
    /// Clean only `target/{debug,release}/incremental/` instead of the whole target.
    pub incremental_only: bool,
    /// Overrides the policy `max_age_days` for both target-clean and stale-worktree.
    pub max_age_days: Option<u32>,
}

// ---------------------------------------------------------------------------
// Pure decision logic (unit-tested)
// ---------------------------------------------------------------------------

/// Verdict for cleaning a worktree's target dir.
#[derive(Debug, PartialEq, Eq)]
pub enum TargetVerdict {
    Clean,
    /// Protected — never touched; carries the reason.
    Protect(&'static str),
    /// Eligible but not stale enough yet.
    NotStale,
}

/// Decide whether a worktree's `target/` may be cleaned. Pure.
pub fn decide_target_clean(
    is_current: bool,
    locked: bool,
    active: bool,
    dirty: bool,
    age_days: u32,
    max_age_days: u32,
    include_dirty: bool,
) -> TargetVerdict {
    if is_current {
        return TargetVerdict::Protect("current-worktree");
    }
    if locked {
        return TargetVerdict::Protect("git-locked");
    }
    if active {
        return TargetVerdict::Protect("active-build");
    }
    if dirty && !include_dirty {
        return TargetVerdict::Protect("dirty-source");
    }
    if age_days < max_age_days {
        return TargetVerdict::NotStale;
    }
    TargetVerdict::Clean
}

/// Decide whether a whole worktree may be removed. Returns `None` when OK to
/// remove, or `Some(reason)` to protect. Pure. Stricter than target cleaning:
/// a dirty worktree is never removed (it may hold uncommitted work).
pub fn decide_worktree_remove(
    is_current: bool,
    is_main: bool,
    locked: bool,
    active: bool,
    dirty: bool,
    age_days: u32,
    max_age_days: u32,
) -> Option<&'static str> {
    if is_main {
        return Some("main-worktree");
    }
    if is_current {
        return Some("current-worktree");
    }
    if locked {
        return Some("git-locked");
    }
    if active {
        return Some("active-build");
    }
    if dirty {
        return Some("dirty-source");
    }
    if age_days < max_age_days {
        return Some("not-stale");
    }
    None
}

/// True when an untracked `git status` path is rebuildable build junk (not real work).
pub fn is_build_junk(rel: &str) -> bool {
    let r = rel.trim().to_lowercase();
    r.starts_with("target/")
        || r.contains("/target/")
        || r.starts_with("build/")
        || r.contains("/build/")
        // vox-arch-check: allow dynlib-ext
        || r.ends_with(".dll")
        || r.ends_with(".exe")
        || r.ends_with(".pdb")
        || r.ends_with(".rlib")
        || r.contains("snapshot")
        || r.contains("-reports")
}

/// Normalize a path string for cross-tool substring matching (lowercase, `/` sep).
fn norm(s: &str) -> String {
    s.to_lowercase().replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Git / process / fs IO
// ---------------------------------------------------------------------------

/// Parse `git worktree list --porcelain`. The first record is the main checkout.
fn parse_worktrees(porcelain: &str) -> Vec<Worktree> {
    let mut out: Vec<Worktree> = Vec::new();
    let mut cur: Option<PathBuf> = None;
    let mut locked = false;
    let mut seen_any = false;
    let flush = |out: &mut Vec<Worktree>, cur: &mut Option<PathBuf>, locked: &mut bool| {
        if let Some(path) = cur.take() {
            let is_main = out.is_empty();
            out.push(Worktree {
                path,
                locked: *locked,
                is_main,
            });
            *locked = false;
        }
    };
    for line in porcelain.lines() {
        if let Some(p) = line.strip_prefix("worktree ") {
            flush(&mut out, &mut cur, &mut locked);
            cur = Some(PathBuf::from(p.trim()));
            seen_any = true;
        } else if line == "locked" || line.starts_with("locked ") {
            locked = true;
        }
    }
    if seen_any {
        flush(&mut out, &mut cur, &mut locked);
    }
    out
}

fn list_worktrees(root: &Path) -> Result<Vec<Worktree>> {
    // vox-arch-check: allow git-exec
    let out = Command::new("git")
        .current_dir(root)
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("run git worktree list")?;
    if !out.status.success() {
        anyhow::bail!("git worktree list failed");
    }
    Ok(parse_worktrees(&String::from_utf8_lossy(&out.stdout)))
}

/// Collect lowercased path haystacks (cwd + exe + cmd) from every build process.
fn build_process_haystacks() -> Vec<String> {
    let sys = System::new_all();
    let mut hay = Vec::new();
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_lowercase();
        let is_build = BUILD_PROC_NEEDLES
            .iter()
            .any(|n| name == *n || name == format!("{n}.exe"));
        if !is_build {
            continue;
        }
        if let Some(cwd) = p.cwd() {
            hay.push(norm(&cwd.to_string_lossy()));
        }
        if let Some(exe) = p.exe() {
            hay.push(norm(&exe.to_string_lossy()));
        }
        for arg in p.cmd() {
            hay.push(norm(&arg.to_string_lossy()));
        }
    }
    hay
}

/// True when any build process references a path under `wt`.
fn worktree_active(wt: &Path, haystacks: &[String]) -> bool {
    let needle = norm(&wt.to_string_lossy());
    haystacks.iter().any(|h| h.contains(&needle))
}

/// Newest mtime of any non-`target`/non-`.git` file in the worktree — the real
/// "last touched" signal (HEAD date misses uncommitted edits and vice-versa).
fn worktree_last_touched(wt: &Path) -> SystemTime {
    let mut newest = UNIX_EPOCH;
    let walker = WalkDir::new(wt).into_iter().filter_entry(|e| {
        if e.file_type().is_dir() {
            let n = e.file_name().to_string_lossy();
            return n != "target" && n != ".git";
        }
        true
    });
    for e in walker.filter_map(Result::ok) {
        if let Ok(m) = e.metadata() {
            if let Ok(t) = m.modified() {
                if t > newest {
                    newest = t;
                }
            }
        }
    }
    newest
}

/// True when the worktree has uncommitted *source* changes (build junk ignored).
///
/// This is the discrimination `protect_dirty` needs to not be a permanent,
/// blanket "some worktree is always dirty" veto: an untracked path is checked
/// against [`is_build_junk`] and does not count as dirty on its own (a fresh
/// `target/` sitting untracked in a worktree with no other changes must not
/// block cleaning *that same* `target/`); any tracked add/modify/delete/rename
/// always counts as real, uncommitted work and always protects. See
/// `worktree_dirty_source_tests` below for git-repo-backed coverage of both
/// branches — this function was previously exercised only indirectly through
/// [`is_build_junk`]'s own unit tests, never directly.
fn worktree_dirty_source(wt: &Path) -> bool {
    // vox-arch-check: allow git-exec
    let Ok(out) = Command::new("git")
        .current_dir(wt)
        .args(["status", "--porcelain"])
        .output()
    else {
        return false;
    };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.len() < 3 {
            continue;
        }
        let (status, path) = line.split_at(2);
        // `git status --porcelain` quotes (and octal-escapes) a path containing
        // non-ASCII or other special characters, e.g. `?? "caf\303\251.dll"`.
        // Stripping only the leading quote left a trailing one on the string,
        // which defeated `is_build_junk`'s `ends_with(".ext")` checks below.
        let path = path.trim().trim_matches('"');
        if status == "??" {
            if !is_build_junk(path) {
                return true;
            }
        } else {
            // any tracked add/modify/delete/rename counts as real work
            return true;
        }
    }
    false
}

fn dir_bytes(path: &Path) -> u64 {
    let mut n = 0u64;
    for e in WalkDir::new(path).into_iter().filter_map(Result::ok) {
        if e.path_is_symlink() {
            continue;
        }
        if let Ok(m) = e.metadata() {
            if m.is_file() {
                n = n.saturating_add(m.len());
            }
        }
    }
    n
}

/// Paths a target-clean would remove for `wt` (whole target, or incremental dirs).
fn target_clean_paths(wt: &Path, incremental_only: bool) -> Vec<PathBuf> {
    if incremental_only {
        ["debug", "release"]
            .iter()
            .map(|p| wt.join("target").join(p).join("incremental"))
            .filter(|p| p.is_dir())
            .collect()
    } else {
        let t = wt.join("target");
        if t.is_dir() { vec![t] } else { vec![] }
    }
}

// ---------------------------------------------------------------------------
// Planner
// ---------------------------------------------------------------------------

/// Build the full plan (delete + protect items) for every worktree.
pub fn plan(
    root: &Path,
    wt_policy: &WorktreeTargetsPolicy,
    stale_policy: &StaleWorktreesPolicy,
    opts: &WorktreeGcOpts,
) -> Result<Vec<PlannedItem>> {
    let worktrees = list_worktrees(root)?;
    let haystacks = build_process_haystacks();
    let root_canon = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    let target_max_age = opts.max_age_days.unwrap_or(wt_policy.max_age_days);
    let stale_max_age = opts.max_age_days.unwrap_or(stale_policy.max_age_days);
    // Clean dirty-but-eligible targets when the user opts in, or when policy
    // declares dirty worktrees unprotected.
    let include_dirty = opts.include_dirty_targets || !wt_policy.protect_dirty;

    let mut items = Vec::new();

    for wt in &worktrees {
        // The primary checkout used to be skipped here unconditionally — its
        // `target/` is the biggest one on the machine and was permanently out of
        // scope "by design". It is now scoped in like any other worktree, gated
        // on the caller having opted into worktree scanning at all (`plan` only
        // runs under `--include-worktrees`, never for a bare `artifact-audit` /
        // `artifact-prune`). Nothing below is weaker for main than for any other
        // worktree: `decide_target_clean` still applies the same active-build /
        // locked / dirty / age gates, and `decide_worktree_remove` still hard-codes
        // `Some("main-worktree")` so the *whole worktree* removal path can never
        // touch it — only its `target/` dir is now eligible, and only when every
        // other safety predicate agrees. In the common case of running the tool
        // from inside the main checkout itself, `is_current` protects it anyway
        // (the "current worktree" gate), so this mainly changes what the report
        // *shows*, not what a bare in-tree invocation can delete.
        let wt_canon = std::fs::canonicalize(&wt.path).unwrap_or_else(|_| wt.path.clone());
        let is_current = wt_canon == root_canon;
        let active = worktree_active(&wt.path, &haystacks);
        let dirty = worktree_dirty_source(&wt.path);
        let touched = worktree_last_touched(&wt.path);
        let age = age_days(touched);

        // Decide whole-worktree removal first: when a tree will be removed
        // entirely, removing it subsumes cleaning its target — so we skip the
        // target item (avoids double work and double-counted reclaim bytes).
        let remove_verdict = if opts.remove_stale_worktrees {
            Some(decide_worktree_remove(
                is_current,
                wt.is_main,
                wt.locked,
                active,
                dirty,
                age,
                stale_max_age,
            ))
        } else {
            None
        };
        if remove_verdict == Some(None) {
            items.push(PlannedItem {
                bytes: dir_bytes(&wt.path),
                age_days: age,
                class: "StaleWorktree",
                action: "delete",
                reason: format!("clean, stale>={stale_max_age}d, unlocked, no active build"),
                worktree: wt.path.clone(),
                path: wt.path.clone(),
            });
            continue;
        }

        // --- target cleaning ---
        let verdict = decide_target_clean(
            is_current,
            wt.locked,
            active,
            dirty,
            age,
            target_max_age,
            include_dirty,
        );
        match verdict {
            TargetVerdict::Clean => {
                for p in target_clean_paths(&wt.path, opts.incremental_only) {
                    let bytes = dir_bytes(&p);
                    items.push(PlannedItem {
                        bytes,
                        age_days: age,
                        class: if opts.incremental_only {
                            "WorktreeIncremental"
                        } else {
                            "WorktreeTarget"
                        },
                        action: "delete",
                        reason: format!("stale>={target_max_age}d, no active build"),
                        worktree: wt.path.clone(),
                        path: p,
                    });
                }
            }
            TargetVerdict::Protect(reason) => {
                let t = wt.path.join("target");
                if t.is_dir() {
                    items.push(PlannedItem {
                        bytes: 0,
                        age_days: age,
                        class: "WorktreeTarget",
                        action: "protect",
                        reason: reason.to_string(),
                        worktree: wt.path.clone(),
                        path: t,
                    });
                }
            }
            TargetVerdict::NotStale => {}
        }

        // --- whole-worktree removal (protect rows only; deletes handled above) ---
        if let Some(Some(reason)) = remove_verdict {
            items.push(PlannedItem {
                bytes: 0,
                age_days: age,
                class: "StaleWorktree",
                action: "protect",
                reason: reason.to_string(),
                worktree: wt.path.clone(),
                path: wt.path.clone(),
            });
        }
    }

    Ok(items)
}

/// Remove a whole worktree: `git worktree remove --force`, with an `rm` + `prune`
/// fallback for the common Windows "Directory not empty" failure.
fn remove_worktree(root: &Path, wt: &Path, dry_run: bool) -> Result<u64> {
    let bytes = dir_bytes(wt);
    if dry_run {
        println!(
            "[dry-run] class=StaleWorktree bytes={bytes} path={}",
            wt.display()
        );
        return Ok(bytes);
    }
    eprintln!("[remove-worktree] bytes={bytes} path={}", wt.display());
    // vox-arch-check: allow git-exec
    let ok = Command::new("git")
        .current_dir(root)
        .args(["worktree", "remove", "--force"])
        .arg(wt)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = std::fs::remove_dir_all(wt);
        // vox-arch-check: allow git-exec
        let _ = Command::new("git")
            .current_dir(root)
            .args(["worktree", "prune"])
            .status();
        eprintln!("[remove-worktree] used rm+prune fallback: {}", wt.display());
    }
    Ok(bytes)
}

/// Execute the plan. Re-checks the active-build gate immediately before deleting
/// (a fresh process snapshot) to close the plan→execute race.
pub fn execute(
    root: &Path,
    dry_run: bool,
    wt_policy: &WorktreeTargetsPolicy,
    stale_policy: &StaleWorktreesPolicy,
    opts: &WorktreeGcOpts,
) -> Result<(u64, std::collections::BTreeMap<String, u32>)> {
    let items = plan(root, wt_policy, stale_policy, opts)?;
    // Fresh snapshot to catch a build that started after planning.
    let haystacks = build_process_haystacks();

    let mut reclaimed = 0u64;
    let mut counts: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();

    for item in &items {
        if item.action != "delete" {
            continue;
        }
        if worktree_active(&item.worktree, &haystacks) {
            eprintln!(
                "[skip] became active since planning: {}",
                item.worktree.display()
            );
            continue;
        }
        let bytes = if item.class == "StaleWorktree" {
            remove_worktree(root, &item.path, dry_run)?
        } else {
            super::delete_path_logged(&item.path, dry_run, item.class, &item.reason)?
        };
        reclaimed = reclaimed.saturating_add(bytes);
        *counts.entry(item.class.to_string()).or_insert(0) += 1;
    }
    Ok((reclaimed, counts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_worktrees_marks_first_as_main_and_reads_locked() {
        let txt = "\
worktree C:/repo
HEAD abc
branch refs/heads/main

worktree C:/repo/.claude/worktrees/foo
HEAD def
branch refs/heads/feat

worktree C:/repo/.claude/worktrees/bar
HEAD 012
detached
locked some reason
";
        let wts = parse_worktrees(txt);
        assert_eq!(wts.len(), 3);
        assert!(wts[0].is_main);
        assert!(!wts[1].is_main);
        assert!(!wts[1].locked);
        assert!(wts[2].locked);
        assert_eq!(wts[2].path, PathBuf::from("C:/repo/.claude/worktrees/bar"));
    }

    #[test]
    fn target_clean_gate_protects_in_priority_order() {
        // current beats everything
        assert_eq!(
            decide_target_clean(true, true, true, true, 999, 7, false),
            TargetVerdict::Protect("current-worktree")
        );
        assert_eq!(
            decide_target_clean(false, true, false, false, 999, 7, false),
            TargetVerdict::Protect("git-locked")
        );
        assert_eq!(
            decide_target_clean(false, false, true, false, 999, 7, false),
            TargetVerdict::Protect("active-build")
        );
        assert_eq!(
            decide_target_clean(false, false, false, true, 999, 7, false),
            TargetVerdict::Protect("dirty-source")
        );
    }

    #[test]
    fn target_clean_respects_age_and_include_dirty() {
        assert_eq!(
            decide_target_clean(false, false, false, false, 3, 7, false),
            TargetVerdict::NotStale
        );
        assert_eq!(
            decide_target_clean(false, false, false, false, 8, 7, false),
            TargetVerdict::Clean
        );
        // include_dirty lets a dirty-but-otherwise-eligible target be cleaned
        assert_eq!(
            decide_target_clean(false, false, false, true, 8, 7, true),
            TargetVerdict::Clean
        );
    }

    #[test]
    fn worktree_remove_is_strict() {
        assert_eq!(
            decide_worktree_remove(false, true, false, false, false, 99, 7),
            Some("main-worktree")
        );
        assert_eq!(
            decide_worktree_remove(true, false, false, false, false, 99, 7),
            Some("current-worktree")
        );
        assert_eq!(
            decide_worktree_remove(false, false, true, false, false, 99, 7),
            Some("git-locked")
        );
        assert_eq!(
            decide_worktree_remove(false, false, false, true, false, 99, 7),
            Some("active-build")
        );
        assert_eq!(
            decide_worktree_remove(false, false, false, false, true, 99, 7),
            Some("dirty-source")
        );
        assert_eq!(
            decide_worktree_remove(false, false, false, false, false, 3, 7),
            Some("not-stale")
        );
        assert_eq!(
            decide_worktree_remove(false, false, false, false, false, 8, 7),
            None
        );
    }

    #[test]
    fn build_junk_recognized() {
        assert!(is_build_junk("target/debug/foo.exe"));
        assert!(is_build_junk("crates/x/target/y"));
        // vox-arch-check: allow dynlib-ext
        assert!(is_build_junk("plugin.dll"));
        assert!(is_build_junk("cr-l-per-gate-reports-abc/x"));
        assert!(!is_build_junk("src/main.rs"));
        assert!(!is_build_junk("Cargo.toml"));
    }

    // --- worktree_dirty_source: real git-repo-backed discrimination ---------
    //
    // `is_build_junk` and the pure `decide_*` functions were already unit
    // tested, but `worktree_dirty_source` itself — the function that actually
    // combines `git status --porcelain` with `is_build_junk` to decide whether
    // "dirty" means real work or just rebuildable output — had no direct
    // coverage. These exercise it against a real temp git repo so task P6-5's
    // fix (b) claim ("uncommitted source protects a target; untracked build
    // junk does not") is verified, not just asserted in a doc comment.

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed in {dir:?}");
    }

    fn init_repo(dir: &Path) {
        git(dir, &["init", "-q"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
    }

    #[test]
    fn worktree_dirty_source_clean_repo_is_not_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        std::fs::write(tmp.path().join("README.md"), "hello").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-q", "-m", "init"]);

        assert!(!worktree_dirty_source(tmp.path()));
    }

    #[test]
    fn worktree_dirty_source_untracked_build_junk_is_not_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        std::fs::write(tmp.path().join("README.md"), "hello").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-q", "-m", "init"]);

        // An untracked build-junk path with nothing else changed must not
        // count as dirty — it is exactly the rebuildable output the target
        // itself is made of, not real uncommitted work.
        std::fs::create_dir_all(tmp.path().join("target/debug")).unwrap();
        std::fs::write(tmp.path().join("target/debug/foo.exe"), "binary").unwrap();

        assert!(!worktree_dirty_source(tmp.path()));
    }

    #[test]
    fn worktree_dirty_source_untracked_real_file_is_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        std::fs::write(tmp.path().join("README.md"), "hello").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-q", "-m", "init"]);

        // A new, untracked source file is real uncommitted work and must
        // protect the target even though it hasn't been `git add`ed yet.
        std::fs::write(tmp.path().join("src_new_feature.rs"), "fn x() {}").unwrap();

        assert!(worktree_dirty_source(tmp.path()));
    }

    #[test]
    fn worktree_dirty_source_tracked_modification_is_dirty() {
        let tmp = tempfile::tempdir().unwrap();
        init_repo(tmp.path());
        std::fs::write(tmp.path().join("README.md"), "hello").unwrap();
        git(tmp.path(), &["add", "."]);
        git(tmp.path(), &["commit", "-q", "-m", "init"]);

        // A modified tracked file is real WIP regardless of its name.
        std::fs::write(tmp.path().join("README.md"), "hello, edited").unwrap();

        assert!(worktree_dirty_source(tmp.path()));
    }

    #[test]
    fn target_clean_paths_modes() {
        // Non-existent dirs filter out; just assert the incremental path shape.
        let wt = Path::new("C:/repo/.claude/worktrees/foo");
        let whole = target_clean_paths(wt, false);
        // target dir doesn't exist in test → empty
        assert!(whole.is_empty());
        let inc = target_clean_paths(wt, true);
        assert!(inc.is_empty());
    }
}
