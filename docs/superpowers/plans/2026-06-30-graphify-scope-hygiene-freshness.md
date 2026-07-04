# Graphify Scope Hygiene + Freshness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the native graph deterministic + clean (gitignore-driven walk) and safe to auto-refresh on a timer, fixing the two critical findings from the SP-1 review.

**Architecture:** Part A swaps `walk_source_files` from `walkdir` to the `ignore` crate so `.gitignore` (which already covers every polluter) is the single exclusion SSOT, with sorted output for determinism. Part B adds a timestamp-based concurrency lock around both rebuild call sites and a regression test confirming the auto path already skips uncommitted-edit drift, then documents a one-time hourly Task Scheduler registration. No extractor/schema change.

**Tech Stack:** Rust, `ignore` crate (already a workspace dep), `chrono`, Windows Task Scheduler.

**Spec:** `docs/superpowers/specs/2026-06-30-graphify-scope-hygiene-freshness-design.md`

**Verified invariants (do not re-derive):**
- `ignore = "0.4.25"` is already in workspace `Cargo.toml:332`.
- `.gitignore` already excludes `.claude/worktrees/`, `dist/`, `.worktrees/`, `node_modules/`, `target/`, `**/.vox/cache/` (confirmed via `git check-ignore`). `.git`/`.vox`/`.claude`/`.worktrees` are dotdirs → skipped by the `ignore` crate's default `hidden(true)`.
- `refresh_action` (`graphify/mod.rs:121-130`) ALREADY maps `worktree_drift`-only → `Skip`. Part B point 2 is a regression test, NOT a logic change.
- Two rebuild call sites: `refresh --auto` Rebuild arm (`graphify/mod.rs:664`) and manual `GraphifyCmd::Rebuild` (`graphify/mod.rs:502`).

---

## File Structure

- `crates/vox-graph-reader/Cargo.toml` — add `ignore = { workspace = true }`; remove `walkdir` only if unused (it is still used by `manifest.rs`, so KEEP it).
- `crates/vox-graph-reader/src/rebuild.rs` — rewrite `walk_source_files` (Part A) + walk tests.
- `crates/vox-cli/src/commands/graphify/mod.rs` — `with_graph_lock` helper, wrap both rebuild sites, lock + `refresh_action` tests (Part B).
- Host step (Part B trigger): documented inline in this plan; run once.

---

## Task 0: Branch

- [ ] **Step 1: Create the working branch.**

Run: `git switch -c sp2-graphify-hygiene-freshness`
Expected: `Switched to a new branch 'sp2-graphify-hygiene-freshness'`.

---

## Task 1: Scope hygiene — gitignore-driven walk (ship first)

**Files:**
- Modify: `crates/vox-graph-reader/Cargo.toml`
- Modify: `crates/vox-graph-reader/src/rebuild.rs:43-61`
- Test: `crates/vox-graph-reader/src/rebuild.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Add the `ignore` dependency**

In `crates/vox-graph-reader/Cargo.toml`, under `[dependencies]`, add:

```toml
ignore = { workspace = true }
```

(Keep `walkdir` — `manifest.rs` still uses it.)

- [ ] **Step 2: Write the failing walk tests**

Append to `crates/vox-graph-reader/src/rebuild.rs` (inside a `#[cfg(test)] mod walk_tests`):

```rust
#[cfg(test)]
mod walk_tests {
    use super::walk_source_files;
    use std::fs;

    #[test]
    fn excludes_gitignored_hidden_and_build_dirs_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        for d in ["src", "dist", ".hidden", "node_modules"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        // .gitignore lists ONLY dist/ — node_modules/ must be excluded by filter_entry,
        // NOT by a gitignore rule (locks the external-repo belt-and-suspenders).
        fs::write(root.join(".gitignore"), "dist/\n").unwrap();
        fs::write(root.join("src/b.rs"), "fn b() {}").unwrap();
        fs::write(root.join("src/a.rs"), "fn a() {}").unwrap();
        fs::write(root.join("dist/bundle.js"), "1").unwrap();
        fs::write(root.join(".hidden/c.rs"), "fn c() {}").unwrap();
        fs::write(root.join("node_modules/dep.js"), "1").unwrap();

        let got: Vec<String> = walk_source_files(root)
            .iter()
            .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"))
            .collect();

        assert_eq!(
            got,
            vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
            "gitignored dist/, hidden .hidden/, and filter_entry node_modules/ all excluded; sorted"
        );
    }
}
```

- [ ] **Step 3: Run the test to confirm it fails**

Run: `cargo test -p vox-graph-reader walk_tests`
Expected: FAIL — current `walkdir` impl includes `dist/bundle.js`'s dir contents and `.hidden/c.rs`, and is unsorted.

- [ ] **Step 4: Rewrite `walk_source_files` with the `ignore` crate**

Replace `crates/vox-graph-reader/src/rebuild.rs:43-61`:

```rust
/// Collect the source files the extractor understands, honoring `.gitignore` (the single
/// exclusion SSOT) and skipping hidden dirs (`.git`, `.vox`, `.claude`, `.github`, `.worktrees`).
/// `require_git(false)` makes `.gitignore` apply even in a checkout without `.git` (an external
/// target repo) — verified against ignore 0.4.25's `gitignore_allowed_no_git` test. `target`
/// and `node_modules` are NOT hidden, so a `filter_entry` prunes them by name as a
/// belt-and-suspenders for sub-scopes / external repos whose `.gitignore` may not list them.
/// Output is sorted (`sort_by_file_path`) for deterministic graph builds.
pub(crate) fn walk_source_files(source_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    ignore::WalkBuilder::new(source_dir)
        .require_git(false)
        .hidden(true) // default; kept explicit — skips .git/.vox/.claude/.github/.worktrees
        .sort_by_file_path(|a, b| a.cmp(b))
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "target" && n != "node_modules"
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()))
        .filter(|e| {
            matches!(
                e.path().extension().and_then(|x| x.to_str()),
                Some("rs" | "ts" | "tsx" | "js" | "jsx" | "py")
            )
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}
```

Notes: `WalkBuilder` defaults `hidden(true)` (skips dotdirs) and `git_ignore(true)` (honors
`.gitignore`); `require_git(false)` is the load-bearing addition so `.gitignore` is honored
without a `.git` dir. `sort_by_file_path` is only honored by the serial `.build()` (not
`build_parallel()`), which we use. `filter_entry` prevents descending into `target`/`node_modules`
regardless of `.gitignore` content.

- [ ] **Step 5: Run the test to confirm it passes**

Run: `cargo test -p vox-graph-reader walk_tests`
Expected: PASS.

- [ ] **Step 6: Confirm the whole reader crate still builds + tests green**

Run: `cargo test -p vox-graph-reader`
Expected: PASS (SP-1 `directed_tests` etc. unaffected).

- [ ] **Step 7: Format and commit**

```bash
cargo fmt -p vox-graph-reader
git add crates/vox-graph-reader/Cargo.toml crates/vox-graph-reader/src/rebuild.rs
git commit -m "fix(graph-reader): gitignore-driven source walk (kills worktree/dist pollution)"
```

---

## Task 2: Rebuild concurrency lock + freshness regression test

**Files:**
- Modify: `crates/vox-cli/src/commands/graphify/mod.rs` (helper + both rebuild sites + tests)

- [ ] **Step 1: Write the failing lock + refresh_action tests**

In `crates/vox-cli/src/commands/graphify/mod.rs`, add a `#[cfg(test)] mod tests` (or extend the existing one). These reference `with_graph_lock` (added in Step 3):

```rust
#[cfg(test)]
mod lock_tests {
    use super::{refresh_action, with_graph_lock, RefreshAction};

    #[test]
    fn refresh_action_skips_worktree_drift_only() {
        assert_eq!(refresh_action(&["worktree_drift".into()]), RefreshAction::Skip);
        assert_eq!(refresh_action(&["git_drift".into()]), RefreshAction::Rebuild);
        assert_eq!(
            refresh_action(&["worktree_drift".into(), "git_drift".into()]),
            RefreshAction::Rebuild
        );
        assert_eq!(refresh_action(&["lexical_lag".into()]), RefreshAction::Ingest);
    }

    #[test]
    fn lock_runs_and_releases_when_free() {
        let tmp = tempfile::tempdir().unwrap();
        let r = with_graph_lock(tmp.path(), || Ok(42)).unwrap();
        assert_eq!(r, Some(42));
        assert!(!tmp.path().join("refresh.lock").exists(), "lock released after run");
    }

    #[test]
    fn lock_skips_when_fresh_lock_held() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("refresh.lock"), "held").unwrap(); // mtime = now
        let mut ran = false;
        let r = with_graph_lock(tmp.path(), || {
            ran = true;
            Ok(())
        })
        .unwrap();
        assert!(r.is_none(), "must skip when a fresh lock is held");
        assert!(!ran, "guarded closure must not run while locked");
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p vox-cli lock_tests`
Expected: FAIL — `with_graph_lock` does not exist yet.

- [ ] **Step 3: Implement `with_graph_lock`**

Add near the `refresh_action` definition in `crates/vox-cli/src/commands/graphify/mod.rs`:

```rust
/// Advisory rebuild lock in `corpus_dir/refresh.lock`. Returns `Ok(Some(result))` when the
/// guarded closure ran, or `Ok(None)` when a *fresh* lock (mtime < 1h) is already held — so the
/// caller skips instead of racing concurrent writes to `graph.json`. A lock older than 1h (or
/// with an unreadable mtime) is treated as stale and reclaimed; rebuilds take seconds, so 1h is
/// a safe upper bound that also self-heals after a `kill -9`/power loss (where Drop can't run).
///
/// This is an *advisory* lock: the check→write window has a benign TOCTOU race, mitigated in
/// practice by the scheduler's `MultipleInstances IgnoreNew` (only a hand-run rebuild could
/// race, and both writes are deterministic). An RAII guard releases the lock on normal return
/// AND on panic/early-`?`, so a crashed rebuild does not wedge the corpus for an hour.
pub(crate) fn with_graph_lock<T>(
    corpus_dir: &std::path::Path,
    f: impl FnOnce() -> anyhow::Result<T>,
) -> anyhow::Result<Option<T>> {
    let lock_path = corpus_dir.join("refresh.lock");
    if let Ok(meta) = std::fs::metadata(&lock_path) {
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|age| age < std::time::Duration::from_secs(3600))
            .unwrap_or(false);
        if fresh {
            return Ok(None);
        }
    }
    std::fs::create_dir_all(corpus_dir).ok();
    std::fs::write(&lock_path, chrono::Utc::now().to_rfc3339())?;

    // RAII: release on normal return, on `?` early-return, and on panic-unwind.
    struct LockGuard(std::path::PathBuf);
    impl Drop for LockGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _guard = LockGuard(lock_path);

    f().map(Some)
}
```

- [ ] **Step 4: Run to confirm the tests pass**

Run: `cargo test -p vox-cli lock_tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Wrap the `refresh --auto` rebuild site**

In the `RefreshAction::Rebuild` arm (`graphify/mod.rs:664`), replace the direct
`rebuild_graph(...)?; println!("  rebuilt {}", c.id);` with a lock-guarded call. The
`output_file` is already in scope; its parent is the corpus dir:

```rust
                        let corpus_dir = output_file
                            .parent()
                            .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                            .to_path_buf();
                        let ran = with_graph_lock(&corpus_dir, || {
                            vox_graph_reader::rebuild::rebuild_graph(
                                repo_root,
                                &source_dir,
                                &output_file,
                                &cache_dir,
                                &meta,
                            )
                            .map_err(|e| anyhow::anyhow!("refresh rebuild {}: {e}", c.id))
                        })?;
                        match ran {
                            Some(()) => println!("  rebuilt {}", c.id),
                            None => println!("  skipped {} (rebuild lock held)", c.id),
                        }
```

- [ ] **Step 6: Wrap the manual `Rebuild` site**

In `GraphifyCmd::Rebuild` (`graphify/mod.rs:502`), wrap the `rebuild_graph(...)` call (after the
snapshot block) the same way. `output_file` is in scope:

```rust
            let corpus_dir = output_file
                .parent()
                .ok_or_else(|| anyhow::anyhow!("graph_path has no parent"))?
                .to_path_buf();
            let ran = with_graph_lock(&corpus_dir, || {
                vox_graph_reader::rebuild::rebuild_graph(
                    repo_root,
                    &source_dir,
                    &output_file,
                    &cache_dir,
                    &meta,
                )
                .map_err(|e| anyhow::anyhow!("Rebuild failed: {}", e))
            })?;
            match ran {
                Some(()) => println!("Graphify rebuild successful!"),
                None => println!("Rebuild skipped: another rebuild is in progress (lock held)."),
            }
```

- [ ] **Step 7: Build + test the crate**

Run: `cargo test -p vox-cli graphify`
Expected: PASS, including pre-existing graphify tests and the new `lock_tests`.

- [ ] **Step 8: Format and commit**

```bash
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/graphify/mod.rs
git commit -m "feat(graphify): rebuild concurrency lock + freshness-skip regression test"
```

---

## Task 3: Documented hourly refresh trigger (host step)

**Files:** none (one-time host action; command documented here).

- [ ] **Step 1: Register the scheduled task (run once on the host).**

The trigger is a one-time Windows Task Scheduler registration — no committed script (AGENTS.md:
VoxScript-only automation, no new `.ps1/.sh/.py`). Run this in PowerShell, substituting the real
`vox.exe` path (`(Get-Command vox).Source`) and repo path:

```powershell
$vox  = (Get-Command vox).Source            # e.g. C:\Users\Owner\.cargo\bin\vox.exe
$repo = "C:\Users\Owner\vox"
$action   = New-ScheduledTaskAction -Execute $vox -Argument "graphify refresh --auto" -WorkingDirectory $repo
$trigger  = New-ScheduledTaskTrigger -Once -At (Get-Date) `
    -RepetitionInterval (New-TimeSpan -Hours 1) `
    -RepetitionDuration (New-TimeSpan -Days 3650)   # ~indefinite; do NOT use [TimeSpan]::MaxValue (serializer overflow on some builds)
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew -Hidden
Register-ScheduledTask -TaskName "VoxGraphifyRefresh" -Action $action -Trigger $trigger -Settings $settings -Description "Hourly graphify corpus refresh (vox graphify refresh --auto)"
```

Notes:
- **`-RepetitionDuration` is required** — `-RepetitionInterval` alone does not repeat reliably
  (fires once or expires after ~1 day depending on the OS build). Use the large finite duration
  above, not `[TimeSpan]::MaxValue`.
- **First tick is ~1h after registration** (the `-Once` anchor + interval); Step 3 below runs the
  refresh once by hand so the corpus isn't idle for the first hour.
- **Repo root resolves from `-WorkingDirectory`** — `graphify refresh` has no `--repo` flag;
  resolution walks up from cwd (`resolve.rs:14`), so `-WorkingDirectory $repo` is sufficient. If a
  future setup loses the working dir, set a `VOX_REPO_ROOT=<repo>` user env var as the documented
  override (`resolve.rs:10`).
- `-Hidden` + non-interactive run suppresses the console for the brief `git rev-parse` the refresh
  shells; `-MultipleInstances IgnoreNew` is belt-and-suspenders with the Task 2 lock.

**Cross-platform equivalents** (same `vox graphify refresh --auto`; the Task 2 lock is pure
`std::fs`, so the no-overlap guarantee carries to all of them):
- **Linux (systemd timer):** a `vox-graphify-refresh.service` (`Type=oneshot`,
  `ExecStart=<vox> graphify refresh --auto`, `WorkingDirectory=<repo>`) + a `.timer`
  (`OnUnitActiveSec=1h`, `Persistent=true` for missed-run catch-up).
- **Linux/macOS (cron):** `0 * * * * cd <repo> && <vox> graphify refresh --auto`.
- **macOS (launchd):** a LaunchAgent plist with `StartInterval = 3600` and `WorkingDirectory`
  set to the repo.

- [ ] **Step 2: Verify the task is registered**

Run: `schtasks /Query /TN VoxGraphifyRefresh /V /FO LIST | findstr /C:"TaskName" /C:"Schedule" /C:"Status"`
Expected: shows `VoxGraphifyRefresh`, an hourly repetition, status Ready.

- [ ] **Step 3: Smoke-test the command the task runs**

Run: `vox graphify refresh --auto`
Expected: prints per-corpus `fresh`/`Rebuild`/`Skip` lines; if it rebuilds, ends with
`rebuilt <id>` and leaves no `refresh.lock` behind in `.vox/cache/graphify/<corpus>/`.

- [ ] **Step 4: Confirm determinism (fixes the non-deterministic-count finding)**

Run `vox graphify rebuild --corpus repo-code-graph` twice in a row (no edits between).
Expected: identical node/edge counts both times, and no `.claude/worktrees/`, `dist/`, or
`node_modules/` node ids in `.vox/cache/graphify/repo-code-graph/graph.json`.

---

## Verification (whole feature)

- [ ] `cargo test -p vox-graph-reader` — PASS (walk + SP-1 tests).
- [ ] `cargo test -p vox-cli graphify` — PASS (lock + refresh tests).
- [ ] `cargo clippy -p vox-graph-reader -p vox-cli -- -D warnings` — no new warnings from touched files (per `feedback_admin_merge_clippy_gap`, run before any admin-merge).
- [ ] Determinism check (Task 3 Step 4) green.

---

## Self-Review

- **Spec coverage:** Part A gitignore-walk + determinism → Task 1; Part B lock → Task 2 Steps 3/5/6; worktree-drift "already correct" regression → Task 2 Step 1; documented hourly trigger (hidden) → Task 3; repo-agnostic (refresh iterates corpora) → unchanged, noted. Edge cases (empty tree, stale lock, fresh corpus, `.gitignore` absent) → covered by `require_git(false)` + the 1h stale-lock reclaim + existing Skip logic.
- **Placeholder scan:** none — every code step is complete; Task 3 is a host command with a real, runnable PowerShell block (paths parameterized via `(Get-Command vox).Source`, not a TODO).
- **Type/name consistency:** `with_graph_lock(&Path, FnOnce -> anyhow::Result<T>) -> anyhow::Result<Option<T>>` defined once (Task 2 Step 3) and called identically at both rebuild sites (Steps 5-6) and in tests (Step 1). `refresh_action`/`RefreshAction` unchanged (only tested). `walk_source_files(&Path) -> Vec<PathBuf>` signature preserved, so callers in `rebuild_graph` are unaffected.
- **Laziness check:** no `refresh_action` logic change (already correct); no new script file (host command documented); `walkdir` kept (still used by `manifest.rs`); lock is mtime-based (no new dep, no PID-liveness syscall).
- **Adversarial-audit hardening (verified against the codebase):** `ignore = { workspace = true }` confirmed under `[workspace.dependencies]` (Cargo.toml:332; precedent in vox-orchestrator{,-mcp}); `require_git(false)` proven to honor a bare `.gitignore` by ignore-0.4.25's own `gitignore_allowed_no_git` test (so the tempdir test must NOT `git init`); over-exclusion audit found only 6 inert `.design-sync/previews/*.tsx` fixtures dropped (0.133%, no inbound imports) → no carve-out needed; `target`/`node_modules` are non-hidden so a `filter_entry` prunes them (Task 1) and the test locks it without a gitignore rule; lock made panic-safe via RAII guard while keeping the 1h mtime reclaim for `kill -9`/power-loss; PowerShell trigger fixed with `-RepetitionDuration` (interval-alone does not repeat reliably); repo-root resolution confirmed cwd-based (no `--repo` flag); cross-platform scheduler equivalents documented (systemd/cron/launchd) per the repo's cross-platform SSOT.
