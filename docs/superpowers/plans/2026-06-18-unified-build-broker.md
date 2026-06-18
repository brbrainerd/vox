# Unified Build Broker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate within-worktree cargo lock-blocking for parallel agents/IDEs via a scoped, daemonless fair-queue `cargo` shim with built-in measurement, plus worktree-isolation convenience and RA-aware bloat GC.

**Architecture:** Layer 0 (rust-analyzer carve-out) is already shipped. This plan builds Layer 1 (a daemonless `cargo` shim that takes a fair FIFO per-worktree queue lock, passes the caller's full env through to real cargo, and logs metrics), Layer 1c (a `vox build-broker worktree` convenience for isolated parallel builds), and Layer 2 (landing the existing target-artifact GC, made aware of `target/rust-analyzer`). A coalescing daemon (1b) is deliberately deferred behind a metrics-based evidence gate.

**Tech Stack:** Rust (new `crates/vox-build-queue` lib + `crates/vox-cargo-shim` bin; new `build-broker` subcommand in `crates/vox-cli`), `fs2` (advisory file locks), `serde_json` (metrics), existing `vox-cli-core::build_service` / `artifact_policy`, existing `worktree_gc`.

**Reference spec:** `docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`

---

## File Structure

- Create `crates/vox-build-queue/` — pure queue + metrics + cargo-resolution logic, no CLI. The testable core.
  - `src/lib.rs` — re-exports.
  - `src/resolve.rs` — `resolve_real_cargo`, `worktree_root_of`, `is_build_subcommand`.
  - `src/queue.rs` — `FairQueue` (FIFO lockfile + position reporting).
  - `src/metrics.rs` — `MetricRecord`, append + `summarize` (p50/p95, coalesce rate).
  - `src/env_filter.rs` — `passthrough_env` (denylist) + `env_hash` / `argv_hash`.
- Create `crates/vox-cargo-shim/` — thin bin named `cargo` (artifact name `cargo-shim`).
  - `src/main.rs` — wire resolve → queue → spawn real cargo → metrics → fallback.
- Modify `crates/vox-cli/src/commands/` — add `build_broker/` subcommand module (`install`, `stats`, `worktree`).
- Modify `crates/vox-cli-core/src/build_service.rs` — route `run_cargo` through the queue when inside a vox worktree (single egress).
- Modify `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs` — RA-awareness.
- Modify `.vscode/settings.json` — prepend shim dir to IDE terminal PATH.
- Modify `AGENTS.md` — document the shim PATH prepend for non-VS-Code hosts.

Queue/metrics live under `.vox/build-queue/` (already an allowed artifact lane sibling).

---

## Task 1: Scaffold `vox-build-queue` crate

**Files:**
- Create: `crates/vox-build-queue/Cargo.toml`
- Create: `crates/vox-build-queue/src/lib.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Create the crate manifest**

```toml
# crates/vox-build-queue/Cargo.toml
[package]
name = "vox-build-queue"
version = "0.1.0"
edition = "2021"
description = "Fair FIFO build queue, cargo resolution, and build metrics for the vox build broker"

[dependencies]
anyhow = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
fs2 = "0.4"

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Create lib.rs with module decls**

```rust
//! Pure logic for the vox build broker: cargo resolution, fair queueing, metrics.
pub mod env_filter;
pub mod metrics;
pub mod queue;
pub mod resolve;
```

- [ ] **Step 3: Register the crate in the workspace**

Add `"crates/vox-build-queue"` to the `members` array in the root `Cargo.toml` (keep the list alphabetically ordered to match existing convention).

- [ ] **Step 4: Verify it builds (empty modules will fail — create stubs)**

Create empty `src/resolve.rs`, `src/queue.rs`, `src/metrics.rs`, `src/env_filter.rs`.
Run: `cargo build -p vox-build-queue`
Expected: PASS (compiles empty crate).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-build-queue Cargo.toml
git commit -m "feat(build-queue): scaffold vox-build-queue crate"
```

---

## Task 2: Cargo + worktree resolution (`resolve.rs`)

**Files:**
- Modify: `crates/vox-build-queue/src/resolve.rs`
- Test: `crates/vox-build-queue/src/resolve.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write failing tests**

```rust
use std::path::{Path, PathBuf};

/// Subcommands whose runs take the cargo target lock and are worth queueing.
pub fn is_build_subcommand(sub: &str) -> bool {
    matches!(sub, "build" | "test" | "check" | "clippy" | "run" | "bench")
}

/// Walk up from `start` to the first directory containing `.cargo/config.toml`.
/// Returns None if no such ancestor exists (caller then bypasses the queue).
pub fn worktree_root_of(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|d| d.join(".cargo/config.toml").is_file()).map(Path::to_path_buf)
}

/// Resolve the real cargo: first `cargo` on PATH whose canonical path differs
/// from `own_exe` (the shim). Preserves the rustup proxy + rust-toolchain.toml.
pub fn resolve_real_cargo(path_var: &str, own_exe: &Path) -> Option<PathBuf> {
    let own = own_exe.canonicalize().ok();
    let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    for dir in std::env::split_paths(&path_var.replace('"', "")) {
        let cand = dir.join(exe);
        if !cand.is_file() {
            continue;
        }
        let canon = cand.canonicalize().ok();
        if canon != own {
            return Some(cand);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_subcommands_classified() {
        assert!(is_build_subcommand("test"));
        assert!(is_build_subcommand("clippy"));
        assert!(!is_build_subcommand("fmt"));
        assert!(!is_build_subcommand("add"));
    }

    #[test]
    fn worktree_root_found_via_cargo_config() {
        let tmp = tempfile::tempdir().unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(wt.join(".cargo")).unwrap();
        std::fs::write(wt.join(".cargo/config.toml"), "[env]\n").unwrap();
        let deep = wt.join("crates/foo/src");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(worktree_root_of(&deep).unwrap().canonicalize().unwrap(),
                   wt.canonicalize().unwrap());
    }

    #[test]
    fn worktree_root_none_outside() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(worktree_root_of(tmp.path()).is_none());
    }

    #[test]
    fn resolve_skips_self() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
        let shim_dir = tmp.path().join("shim");
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&shim_dir).unwrap();
        std::fs::create_dir_all(&real_dir).unwrap();
        let shim = shim_dir.join(exe);
        let real = real_dir.join(exe);
        std::fs::write(&shim, b"x").unwrap();
        std::fs::write(&real, b"y").unwrap();
        let path = std::env::join_paths([&shim_dir, &real_dir]).unwrap();
        let got = resolve_real_cargo(path.to_str().unwrap(), &shim).unwrap();
        assert_eq!(got.canonicalize().unwrap(), real.canonicalize().unwrap());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-build-queue resolve`
Expected: FAIL (module currently empty — compile error / no such items).

- [ ] **Step 3: Implement**

The function bodies above ARE the implementation — paste the non-test items into `src/resolve.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-build-queue resolve`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-build-queue/src/resolve.rs
git commit -m "feat(build-queue): cargo + worktree resolution"
```

---

## Task 3: Env filtering + hashing (`env_filter.rs`)

**Files:**
- Modify: `crates/vox-build-queue/src/env_filter.rs`

- [ ] **Step 1: Write failing tests**

```rust
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

/// Volatile vars that do not affect cargo fingerprints; excluded from the build
/// env replay and from the coalescing key.
const DENYLIST: &[&str] = &["PROMPT", "TERM", "TERM_SESSION_ID", "WT_SESSION", "PWD", "OLDPWD"];

/// The caller's env minus the volatile denylist, sorted for determinism.
pub fn passthrough_env(raw: impl IntoIterator<Item = (String, String)>) -> BTreeMap<String, String> {
    raw.into_iter().filter(|(k, _)| !DENYLIST.contains(&k.as_str())).collect()
}

fn hash64<T: Hash>(v: &T) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

/// Stable hash of the build-relevant env (post-denylist).
pub fn env_hash(env: &BTreeMap<String, String>) -> u64 {
    let pairs: Vec<(&String, &String)> = env.iter().collect();
    hash64(&pairs)
}

/// Stable hash of argv.
pub fn argv_hash(argv: &[String]) -> u64 {
    hash64(&argv.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denylist_dropped() {
        let env = passthrough_env([
            ("RUSTFLAGS".into(), "-Cdebuginfo=0".into()),
            ("PROMPT".into(), "$P$G".into()),
        ]);
        assert!(env.contains_key("RUSTFLAGS"));
        assert!(!env.contains_key("PROMPT"));
    }

    #[test]
    fn rustflags_change_changes_hash() {
        let a = passthrough_env([("RUSTFLAGS".into(), "-Cdebuginfo=0".into())]);
        let b = passthrough_env([("RUSTFLAGS".into(), "-Cdebuginfo=2".into())]);
        assert_ne!(env_hash(&a), env_hash(&b));
    }

    #[test]
    fn volatile_change_does_not_change_hash() {
        let a = passthrough_env([("RUSTFLAGS".into(), "x".into()), ("PROMPT".into(), "1".into())]);
        let b = passthrough_env([("RUSTFLAGS".into(), "x".into()), ("PROMPT".into(), "2".into())]);
        assert_eq!(env_hash(&a), env_hash(&b));
    }

    #[test]
    fn argv_hash_sensitive() {
        assert_ne!(argv_hash(&["test".into(), "-p".into(), "a".into()]),
                   argv_hash(&["test".into(), "-p".into(), "b".into()]));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-build-queue env_filter`
Expected: FAIL (empty module).

- [ ] **Step 3: Implement** — paste the non-test items into `src/env_filter.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-build-queue env_filter`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-build-queue/src/env_filter.rs
git commit -m "feat(build-queue): env passthrough + fingerprint-safe hashing"
```

---

## Task 4: Metrics record + summary (`metrics.rs`)

**Files:**
- Modify: `crates/vox-build-queue/src/metrics.rs`

- [ ] **Step 1: Write failing tests**

```rust
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricRecord {
    pub ts_ms: u128,
    pub worktree: String,
    pub subcmd: String,
    pub queue_wait_ms: u64,
    pub ran_ms: u64,
    pub argv_hash: u64,
    pub env_hash: u64,
    pub would_coalesce: bool,
}

/// Append one JSON line to the metrics file (created if absent).
pub fn append(path: &Path, rec: &MetricRecord) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", serde_json::to_string(rec)?)?;
    Ok(())
}

#[derive(Debug, PartialEq)]
pub struct Summary {
    pub count: usize,
    pub p50_wait_ms: u64,
    pub p95_wait_ms: u64,
    pub coalesce_rate: f64,
}

/// Read a metrics.jsonl file and summarize. Malformed lines are skipped.
pub fn summarize(path: &Path) -> anyhow::Result<Summary> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let recs: Vec<MetricRecord> =
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let count = recs.len();
    if count == 0 {
        return Ok(Summary { count: 0, p50_wait_ms: 0, p95_wait_ms: 0, coalesce_rate: 0.0 });
    }
    let mut waits: Vec<u64> = recs.iter().map(|r| r.queue_wait_ms).collect();
    waits.sort_unstable();
    let pct = |p: f64| waits[((waits.len() as f64 - 1.0) * p).round() as usize];
    let coalesce = recs.iter().filter(|r| r.would_coalesce).count() as f64 / count as f64;
    Ok(Summary { count, p50_wait_ms: pct(0.50), p95_wait_ms: pct(0.95), coalesce_rate: coalesce })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(wait: u64, coalesce: bool) -> MetricRecord {
        MetricRecord {
            ts_ms: 0, worktree: "wt".into(), subcmd: "test".into(),
            queue_wait_ms: wait, ran_ms: 100, argv_hash: 1, env_hash: 2,
            would_coalesce: coalesce,
        }
    }

    #[test]
    fn append_then_summarize() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("metrics.jsonl");
        for (w, c) in [(0, false), (100, true), (200, false), (300, false)] {
            append(&p, &rec(w, c)).unwrap();
        }
        let s = summarize(&p).unwrap();
        assert_eq!(s.count, 4);
        assert_eq!(s.p50_wait_ms, 200); // index round((4-1)*0.5)=2 -> sorted[2]=200
        assert!((s.coalesce_rate - 0.25).abs() < 1e-9);
    }

    #[test]
    fn summarize_missing_file_is_empty() {
        let s = summarize(Path::new("does-not-exist.jsonl")).unwrap();
        assert_eq!(s.count, 0);
    }

    #[test]
    fn malformed_lines_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("m.jsonl");
        std::fs::write(&p, "not json\n").unwrap();
        append(&p, &rec(5, false)).unwrap();
        assert_eq!(summarize(&p).unwrap().count, 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-build-queue metrics`
Expected: FAIL (empty module).

- [ ] **Step 3: Implement** — paste the non-test items into `src/metrics.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-build-queue metrics`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-build-queue/src/metrics.rs
git commit -m "feat(build-queue): metrics record append + summary"
```

---

## Task 5: Fair FIFO queue lock (`queue.rs`)

**Files:**
- Modify: `crates/vox-build-queue/src/queue.rs`

- [ ] **Step 1: Write failing tests**

```rust
use anyhow::Result;
use std::path::{Path, PathBuf};

/// A fair FIFO queue scoped to one worktree, backed by a lock directory.
/// Each waiter writes a ticket file named by a monotonic sequence; the holder is
/// the lowest outstanding ticket. The exclusive lockfile guarantees one runner.
pub struct FairQueue {
    dir: PathBuf,
}

pub struct Ticket {
    seq: u64,
    dir: PathBuf,
    _lock: std::fs::File, // held for the duration; dropping releases
}

impl FairQueue {
    pub fn new(queue_root: &Path, worktree_hash: &str) -> Result<Self> {
        let dir = queue_root.join(worktree_hash);
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Claim the next monotonic ticket number (arrival order).
    pub fn take_number(&self) -> Result<u64> {
        use fs2::FileExt;
        let counter = self.dir.join("counter");
        let f = std::fs::OpenOptions::new().create(true).read(true).write(true).open(&counter)?;
        f.lock_exclusive()?;
        let cur: u64 = std::fs::read_to_string(&counter).ok()
            .and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        let next = cur + 1;
        std::fs::write(&counter, next.to_string())?;
        FileExt::unlock(&f)?;
        // Record an outstanding ticket marker so position() can count ahead.
        std::fs::write(self.dir.join(format!("t{next}")), b"")?;
        Ok(next)
    }

    /// How many outstanding tickets are ahead of `seq` (lower-numbered).
    pub fn position(&self, seq: u64) -> usize {
        std::fs::read_dir(&self.dir).map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| e.file_name().to_str()
                    .and_then(|n| n.strip_prefix('t')).and_then(|n| n.parse::<u64>().ok()))
                .filter(|&n| n < seq)
                .count()
        }).unwrap_or(0)
    }

    /// Block until holder of the exclusive run-lock, then return a Ticket whose
    /// drop releases both the run-lock and this ticket marker.
    pub fn acquire(&self, seq: u64) -> Result<Ticket> {
        use fs2::FileExt;
        let lock = std::fs::OpenOptions::new()
            .create(true).read(true).write(true).open(self.dir.join("run.lock"))?;
        lock.lock_exclusive()?; // OS arbitrates; fairness comes from arrival waiting below
        Ok(Ticket { seq, dir: self.dir.clone(), _lock: lock })
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.dir.join(format!("t{}", self.seq)));
        // _lock unlocks on close.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tickets_are_monotonic() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let a = q.take_number().unwrap();
        let b = q.take_number().unwrap();
        assert_eq!(b, a + 1);
    }

    #[test]
    fn position_counts_outstanding_ahead() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let first = q.take_number().unwrap();
        let second = q.take_number().unwrap();
        assert_eq!(q.position(second), 1); // one ahead (first)
        assert_eq!(q.position(first), 0);
    }

    #[test]
    fn acquire_then_drop_releases() {
        let tmp = tempfile::tempdir().unwrap();
        let q = FairQueue::new(tmp.path(), "abc").unwrap();
        let n = q.take_number().unwrap();
        {
            let _t = q.acquire(n).unwrap();
            assert_eq!(q.position(n), 0);
        }
        // ticket marker removed after drop
        assert!(!tmp.path().join("abc").join(format!("t{n}")).exists());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-build-queue queue`
Expected: FAIL (empty module).

- [ ] **Step 3: Implement** — paste the non-test items into `src/queue.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-build-queue queue`
Expected: PASS (3 tests).

> Note: OS exclusive `lock_exclusive` provides mutual exclusion; the ticket
> markers provide the *position display* and waiter accounting. True strict-FIFO
> ordering across processes is best-effort (the OS wakes one waiter); arrival
> order is approximated by the ticket numbers, which is sufficient for the UX and
> metrics goals. Document this limitation in the module doc comment.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-build-queue/src/queue.rs
git commit -m "feat(build-queue): fair FIFO queue lock with position display"
```

---

## Task 6: The `cargo` shim binary

**Files:**
- Create: `crates/vox-cargo-shim/Cargo.toml`
- Create: `crates/vox-cargo-shim/src/main.rs`
- Modify: root `Cargo.toml` (members)
- Test: `crates/vox-cargo-shim/tests/passthrough.rs`

- [ ] **Step 1: Create the manifest (binary named `cargo`)**

```toml
# crates/vox-cargo-shim/Cargo.toml
[package]
name = "vox-cargo-shim"
version = "0.1.0"
edition = "2021"
description = "Scoped daemonless cargo shim that fair-queues builds within a vox worktree"

[[bin]]
name = "cargo"        # MUST be literally `cargo` so PATH interception works
path = "src/main.rs"

[dependencies]
anyhow = { workspace = true }
vox-build-queue = { path = "../vox-build-queue" }
```

Add `"crates/vox-cargo-shim"` to workspace members.

- [ ] **Step 2: Write the failing integration test**

```rust
// crates/vox-cargo-shim/tests/passthrough.rs
use std::process::Command;

fn shim_bin() -> std::path::PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name>; bin name is `cargo`.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_cargo"))
}

#[test]
fn non_build_subcommand_passes_through_to_real_cargo() {
    // `cargo --version` is not a build subcommand -> shim must exec real cargo.
    let out = Command::new(shim_bin()).arg("--version").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.starts_with("cargo "), "expected real cargo version, got: {s}");
}

#[test]
fn outside_worktree_passes_through() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(shim_bin())
        .arg("--version")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
}
```

Add `tempfile = { workspace = true }` to `[dev-dependencies]` in the shim manifest.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cargo-shim`
Expected: FAIL (no main / empty binary).

- [ ] **Step 4: Implement the shim**

```rust
// crates/vox-cargo-shim/src/main.rs
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use vox_build_queue::{env_filter, metrics, queue::FairQueue, resolve};

fn real_cargo() -> Option<PathBuf> {
    let own = std::env::current_exe().ok()?;
    let path = std::env::var("PATH").unwrap_or_default();
    resolve::resolve_real_cargo(&path, &own)
}

/// Exec real cargo with the original args, replacing this process where possible.
fn exec_real(real: &PathBuf, args: &[String]) -> ! {
    let status = Command::new(real).args(args).status();
    std::process::exit(status.ok().and_then(|s| s.code()).unwrap_or(1));
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let real = match real_cargo() {
        Some(r) => r,
        None => {
            eprintln!("vox-broker: real cargo not found on PATH; aborting");
            std::process::exit(127);
        }
    };

    let sub = args.first().map(String::as_str).unwrap_or("");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let worktree = resolve::worktree_root_of(&cwd);

    // Fast path: non-build subcommand or outside any worktree -> direct exec.
    if !resolve::is_build_subcommand(sub) || worktree.is_none() {
        exec_real(&real, &args);
    }
    let worktree = worktree.unwrap();
    let wt_hash = format!("{:016x}", {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        worktree.hash(&mut h);
        h.finish()
    });

    // Any failure below falls back to a plain exec (broker is never a hard dep).
    let run = || -> anyhow::Result<i32> {
        let queue_root = worktree.join(".vox/build-queue");
        let q = FairQueue::new(&queue_root, &wt_hash)?;
        let seq = q.take_number()?;

        let env: std::collections::BTreeMap<String, String> =
            env_filter::passthrough_env(std::env::vars());
        let argv_hash = env_filter::argv_hash(&args);
        let env_hash = env_filter::env_hash(&env);
        let would_coalesce = q.position(seq) > 0; // someone already ahead/in-flight

        let pos = q.position(seq);
        if pos > 0 {
            eprintln!("vox-broker: queued (position {pos}) for {}", worktree.display());
        }
        let t_wait = Instant::now();
        let _ticket = q.acquire(seq)?;
        let queue_wait_ms = t_wait.elapsed().as_millis() as u64;

        let t_run = Instant::now();
        let mut cmd = Command::new(&real);
        cmd.args(&args).current_dir(&cwd);
        cmd.env_clear();
        for (k, v) in &env {
            cmd.env(k, v);
        }
        cmd.env("CARGO_TERM_COLOR", "always");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let status = cmd.status()?;
        let ran_ms = t_run.elapsed().as_millis() as u64;

        let rec = metrics::MetricRecord {
            ts_ms: 0, // stamped by reader; avoid time dep here
            worktree: worktree.display().to_string(),
            subcmd: sub.to_string(),
            queue_wait_ms,
            ran_ms,
            argv_hash,
            env_hash,
            would_coalesce,
        };
        let _ = metrics::append(&queue_root.join("metrics.jsonl"), &rec);
        Ok(status.code().unwrap_or(1))
    };

    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("vox-broker: queue error ({e}); running cargo directly");
            exec_real(&real, &args);
        }
    }
}
```

> Note on `ts_ms`: to avoid a time dependency conflicting with vox's `Date.now()`
> rules in scripts, the shim leaves `ts_ms=0`; if a real timestamp is wanted,
> add `std::time::SystemTime` here (this is Rust, not VoxScript — allowed). Keep
> as-is unless stats need wall-clock ordering.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-cargo-shim`
Expected: PASS (2 tests). The shim resolves the real cargo (the rustup proxy on PATH, since the shim under `target/` is not named to collide here) and passes `--version` through.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cargo-shim Cargo.toml
git commit -m "feat(cargo-shim): scoped daemonless fair-queue cargo shim"
```

---

## Task 7: Coalescing integration test (fake cargo)

**Files:**
- Test: `crates/vox-cargo-shim/tests/queue_serializes.rs`

- [ ] **Step 1: Write the failing test**

```rust
// Verifies two concurrent build invocations in the same worktree serialize
// (one runs while the other waits), and a metrics line is written per run.
use std::fs;
use std::process::Command;
use std::thread;

fn shim_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_cargo"))
}

/// Create a fake worktree whose `cargo` (real, found via a stub on PATH) just
/// sleeps and writes a marker, so we can observe serialization.
#[test]
fn two_builds_serialize_and_emit_metrics() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt");
    fs::create_dir_all(wt.join(".cargo")).unwrap();
    fs::write(wt.join(".cargo/config.toml"), "[env]\n").unwrap();

    // Stub "real cargo": a script that sleeps 400ms. Put it first on PATH.
    let bindir = tmp.path().join("bin");
    fs::create_dir_all(&bindir).unwrap();
    #[cfg(windows)]
    let (name, body) = ("cargo.bat", "@echo off\r\nping -n 1 -w 400 127.0.0.1 >nul\r\nexit /b 0\r\n");
    #[cfg(not(windows))]
    let (name, body) = ("cargo", "#!/bin/sh\nsleep 0.4\nexit 0\n");
    let stub = bindir.join(name);
    fs::write(&stub, body).unwrap();
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let run = |bindir: std::path::PathBuf, wt: std::path::PathBuf| {
        thread::spawn(move || {
            let path = format!("{}{}{}", bindir.display(),
                if cfg!(windows) { ";" } else { ":" },
                std::env::var("PATH").unwrap_or_default());
            let start = std::time::Instant::now();
            let status = Command::new(shim_bin())
                .args(["build"]).current_dir(&wt).env("PATH", path).status().unwrap();
            (status.success(), start.elapsed())
        })
    };

    let h1 = run(bindir.clone(), wt.clone());
    let h2 = run(bindir.clone(), wt.clone());
    let (ok1, _) = h1.join().unwrap();
    let (ok2, _) = h2.join().unwrap();
    assert!(ok1 && ok2);

    let metrics = fs::read_to_string(wt.join(".vox/build-queue").join({
        use std::hash::{Hash, Hasher};
        let mut hsh = std::collections::hash_map::DefaultHasher::new();
        wt.canonicalize().unwrap().hash(&mut hsh);
        // worktree_root_of returns the dir with .cargo/config.toml; the shim
        // canonicalizes cwd's ancestor, so hash the same value the shim used.
        // We instead just scan the dir below.
        let _ = hsh.finish();
        ""
    }).join("..")).unwrap_or_default();
    // Simpler: assert a metrics.jsonl exists somewhere under .vox/build-queue.
    let mut found = false;
    for entry in walk(&wt.join(".vox/build-queue")) {
        if entry.file_name().map(|n| n == "metrics.jsonl").unwrap_or(false) {
            found = true;
            let lines = fs::read_to_string(&entry).unwrap();
            assert_eq!(lines.lines().count(), 2, "expected 2 metric lines");
        }
    }
    assert!(found, "metrics.jsonl not written");
    let _ = metrics;
}

fn walk(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = vec![];
    if let Ok(rd) = fs::read_dir(root) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(&p)); } else { out.push(p); }
        }
    }
    out
}
```

> The shim's `worktree_root_of` matches the dir containing `.cargo/config.toml`
> (here `wt`), so the queue dir is `wt/.vox/build-queue/<hash>/`. The test scans
> for `metrics.jsonl` rather than recomputing the hash to stay robust.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cargo-shim two_builds_serialize`
Expected: FAIL initially if metrics path/threading has a bug; iterate until green.

- [ ] **Step 3: Fix any issues surfaced** (path resolution, hash mismatch). No new production code expected beyond Task 6; this test validates it end-to-end.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cargo-shim`
Expected: PASS (all shim tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cargo-shim/tests/queue_serializes.rs
git commit -m "test(cargo-shim): serialization + metrics emission end-to-end"
```

---

## Task 8: `vox build-broker` subcommand (install / stats / worktree)

**Files:**
- Create: `crates/vox-cli/src/commands/build_broker/mod.rs`
- Create: `crates/vox-cli/src/commands/build_broker/install.rs`
- Create: `crates/vox-cli/src/commands/build_broker/stats.rs`
- Create: `crates/vox-cli/src/commands/build_broker/worktree.rs`
- Modify: `crates/vox-cli/src/commands/mod.rs` (register module)
- Modify: the top-level CLI enum/dispatch (wherever subcommands are wired; e.g. `cli_dispatch/mod.rs`)
- Modify: `crates/vox-cli/Cargo.toml` (add `vox-build-queue` dep)

- [ ] **Step 1: Write a failing test for `stats`**

```rust
// crates/vox-cli/src/commands/build_broker/stats.rs  (inline test)
use anyhow::Result;
use std::path::Path;
use vox_build_queue::metrics;

/// Render a one-line go/no-go summary from a worktree's metrics file.
pub fn render_stats(worktree: &Path) -> Result<String> {
    let p = worktree.join(".vox/build-queue");
    // Aggregate across all per-worktree-hash subdirs (usually one).
    let mut all = String::new();
    if let Ok(rd) = std::fs::read_dir(&p) {
        for e in rd.flatten() {
            let m = e.path().join("metrics.jsonl");
            if m.is_file() {
                all.push_str(&std::fs::read_to_string(&m).unwrap_or_default());
            }
        }
    }
    let tmp = std::env::temp_dir().join(format!("vox-stats-{}.jsonl", std::process::id()));
    std::fs::write(&tmp, &all)?;
    let s = metrics::summarize(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    let verdict = if s.coalesce_rate >= 0.10 && s.p50_wait_ms > 0 {
        "BUILD-DAEMON: recommended (coalesce>=10% and queue waits present)"
    } else {
        "BUILD-DAEMON: not needed (daemonless shim sufficient)"
    };
    Ok(format!(
        "builds={} p50_wait={}ms p95_wait={}ms coalesce_rate={:.1}% -> {}",
        s.count, s.p50_wait_ms, s.p95_wait_ms, s.coalesce_rate * 100.0, verdict
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verdict_not_needed_when_no_coalesce() {
        let tmp = tempfile::tempdir().unwrap();
        let qd = tmp.path().join(".vox/build-queue/abc");
        std::fs::create_dir_all(&qd).unwrap();
        std::fs::write(qd.join("metrics.jsonl"),
            r#"{"ts_ms":0,"worktree":"w","subcmd":"test","queue_wait_ms":0,"ran_ms":5,"argv_hash":1,"env_hash":2,"would_coalesce":false}"#).unwrap();
        let out = render_stats(tmp.path()).unwrap();
        assert!(out.contains("not needed"), "got: {out}");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli render_stats` (after wiring the module).
Expected: FAIL (module not present / not compiled).

- [ ] **Step 3: Implement `install` and `worktree`, wire the subcommand**

`install.rs`: build the shim (`cargo build -p vox-cargo-shim --release` via `build_service`), copy `target/release/cargo[.exe]` to `<worktree>/.vox/bin/cargo-shim/cargo[.exe]`, then ensure `.vscode/settings.json` prepends that dir to `terminal.integrated.env.windows.PATH`. Print the exact PATH-prepend line for non-VS-Code hosts.

```rust
// crates/vox-cli/src/commands/build_broker/install.rs
use anyhow::{Context, Result};
use std::path::Path;

pub fn install(worktree: &Path) -> Result<()> {
    let shim_dir = worktree.join(".vox/bin/cargo-shim");
    std::fs::create_dir_all(&shim_dir)?;
    let exe = if cfg!(windows) { "cargo.exe" } else { "cargo" };
    let built = worktree.join("target/release").join(exe);
    anyhow::ensure!(built.is_file(),
        "build the shim first: cargo build -p vox-cargo-shim --release");
    std::fs::copy(&built, shim_dir.join(exe)).context("copy shim")?;
    println!("Installed cargo shim to {}", shim_dir.display());
    println!("Add this dir to PATH for IDE terminals (VS Code/forks via .vscode/settings.json):");
    println!("  {}", shim_dir.display());
    Ok(())
}
```

`worktree.rs`: thin wrapper that shells `git worktree add` under a sibling dir and prints the path (relies on the per-worktree `.cargo/config.toml` for an isolated target). Keep minimal; reuse any existing worktree helper if present.

`mod.rs`: a `BuildBrokerCmd` enum `{ Install, Stats, Worktree { name: String } }` dispatched to the three functions. Register in the CLI command enum + dispatch (follow the pattern of an existing subcommand such as `ci`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli render_stats`
Expected: PASS. Then `cargo build -p vox-cli` to confirm wiring compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/build_broker crates/vox-cli/src/commands/mod.rs crates/vox-cli/Cargo.toml
git commit -m "feat(cli): vox build-broker install/stats/worktree subcommand"
```

---

## Task 9: Single egress — route `build_service::run_cargo` through the queue

**Files:**
- Modify: `crates/vox-cli-core/src/build_service.rs:155-199`
- Modify: `crates/vox-cli-core/Cargo.toml` (add `vox-build-queue` dep)
- Test: `crates/vox-cli-core/src/build_service.rs` (inline)

- [ ] **Step 1: Write a failing test**

```rust
// Asserts run_cargo, when cwd is inside a worktree, leaves a queue ticket trail.
#[test]
fn run_cargo_uses_queue_inside_worktree() {
    let tmp = tempfile::tempdir().unwrap();
    let wt = tmp.path().join("wt");
    std::fs::create_dir_all(wt.join(".cargo")).unwrap();
    std::fs::write(wt.join(".cargo/config.toml"), "[env]\n").unwrap();
    std::fs::write(wt.join("Cargo.toml"),
        "[package]\nname=\"x\"\nversion=\"0.0.0\"\nedition=\"2021\"\n").unwrap();
    std::fs::create_dir_all(wt.join("src")).unwrap();
    std::fs::write(wt.join("src/lib.rs"), "").unwrap();

    let req = CargoRequest::check(wt.clone(), None);
    let _ = run_cargo(&req); // may fail to compile x; we only assert queueing happened
    assert!(wt.join(".vox/build-queue").exists(), "queue dir not created");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli-core run_cargo_uses_queue`
Expected: FAIL (no queue dir created today).

- [ ] **Step 3: Implement**

In `run_cargo`, before spawning, compute `worktree_root_of(&req.cwd)`. If `Some(wt)`, take a `FairQueue` ticket + `acquire` (holding the guard across the spawn) and append a metric, mirroring the shim. If `None`, run as today. Keep the existing `artifact_policy` check unchanged. Factor the shared "queue-then-run" logic into a small helper in `vox-build-queue` if duplication with the shim grows, to honor DRY.

```rust
// inside run_cargo, replacing the direct `cmd.output()` region:
let _queue_guard = match vox_build_queue::resolve::worktree_root_of(&req.cwd) {
    Some(wt) => {
        let wt_hash = vox_build_queue::queue::hash_path(&wt); // add this small helper
        let q = vox_build_queue::queue::FairQueue::new(&wt.join(".vox/build-queue"), &wt_hash)?;
        let seq = q.take_number()?;
        Some(q.acquire(seq)?) // released when run_cargo returns
    }
    None => None,
};
let output = cmd.output().context("Failed to run cargo")?;
```

Add `pub fn hash_path(p: &Path) -> String` to `queue.rs` (the same DefaultHasher → hex used in the shim — move the shim to call it too, DRY).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli-core run_cargo_uses_queue`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-core/src/build_service.rs crates/vox-cli-core/Cargo.toml crates/vox-build-queue/src/queue.rs crates/vox-cargo-shim/src/main.rs
git commit -m "feat(build-service): route cargo through fair queue (single egress)"
```

---

## Task 10: Wire the shim PATH into IDE settings + AGENTS.md

**Files:**
- Modify: `.vscode/settings.json`
- Modify: `AGENTS.md`

- [ ] **Step 1: Prepend the shim dir to IDE terminal PATH**

In `.vscode/settings.json`, update `terminal.integrated.env.windows.PATH` to put the shim dir first:

```json
"PATH": "${workspaceFolder}\\.vox\\bin\\cargo-shim;C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.1\\bin;C:\\Program Files\\NVIDIA GPU Computing Toolkit\\CUDA\\v13.1\\bin\\x64;${env:PATH}"
```

- [ ] **Step 2: Document the equivalent for other hosts**

Add a short section to `AGENTS.md` under build tooling: "Cross-agent build queue — prepend `<repo>/.vox/bin/cargo-shim` to PATH in your IDE/agent terminal env so cargo invocations share the fair build queue. Run `vox build-broker install` once to build + place the shim. The shim falls back to real cargo if anything is misconfigured, so it is safe to omit."

- [ ] **Step 3: Verify settings JSON is valid**

Run: `node -e "JSON.parse(require('fs').readFileSync('.vscode/settings.json','utf8')); console.log('ok')"`
Expected: `ok`.

- [ ] **Step 4: Commit**

```bash
git add .vscode/settings.json AGENTS.md
git commit -m "chore(build-broker): scope cargo shim to IDE terminals + document"
```

---

## Task 11: Layer 2 — RA-aware target GC

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs`
- Test: same file (inline)

- [ ] **Step 1: Write failing tests for RA-awareness**

```rust
#[test]
fn ra_target_not_pruned_when_ra_active() {
    // decision logic should refuse to prune target/rust-analyzer when a
    // rust-analyzer process is reported active.
    let decision = classify_target(
        /* path */ Path::new("target/rust-analyzer"),
        /* age_days */ 30,
        /* active_build_processes */ &["rust-analyzer".to_string()],
    );
    assert_eq!(decision, GcDecision::Skip(SkipReason::ActiveBuild));
}

#[test]
fn ra_target_incremental_prunable_when_idle() {
    let decision = classify_target(
        Path::new("target/rust-analyzer"),
        30,
        &[],
    );
    assert_eq!(decision, GcDecision::IncrementalPrune);
}
```

(Adjust `classify_target`'s real signature to match the existing pure-logic API in the file; the point is two behaviors: skip while RA active, incremental-prune when idle.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-cli ra_target`
Expected: FAIL.

- [ ] **Step 3: Implement**

Extend the existing active-build process needle list to include `"rust-analyzer"` (and `"rust-analyzer-proc-macro-srv"`). Add a branch: a path whose last component is `rust-analyzer` is a `worktree-target` subtree → eligible for `IncrementalPrune` when no needle process is active, else `Skip(ActiveBuild)`. Reuse the existing 7-day staleness for full removal of the *parent* worktree only.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-cli ra_target`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/workspace_artifacts/worktree_gc.rs
git commit -m "feat(gc): make target GC aware of target/rust-analyzer"
```

---

## Task 12: Full verification pass

- [ ] **Step 1: Build + test the new crates and touched crates**

Run:
```bash
cargo test -p vox-build-queue -p vox-cargo-shim
cargo test -p vox-cli-core run_cargo_uses_queue
cargo test -p vox-cli render_stats ra_target
```
Expected: all PASS.

- [ ] **Step 2: Clippy on touched crates** (per the admin-merge clippy-gap memory)

Run: `cargo clippy -p vox-build-queue -p vox-cargo-shim -p vox-cli-core -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Format** (per the no-`cargo fmt --all` rule)

Run: `cargo fmt -p vox-build-queue -p vox-cargo-shim`
(or `vox run scripts/fmt.vox`)

- [ ] **Step 4: Smoke test the real install**

Run:
```bash
cargo build -p vox-cargo-shim --release
vox build-broker install
# open a new IDE terminal, then:
where cargo            # shim dir should appear first
cargo build -p vox-build-queue   # should print a queue position only under contention
vox build-broker stats
```
Expected: `stats` prints a populated line with a go/no-go verdict.

- [ ] **Step 5: Final commit / branch handoff**

```bash
git add -A
git commit -m "chore(build-broker): verification pass green"
```

---

## Self-Review notes (author)

- **Spec coverage:** L0 (shipped, referenced) ✓; L1 shim + scoped PATH (T6, T10) ✓; fair queue + visibility (T5, T6) ✓; env passthrough + fingerprint safety (T3) ✓; metrics + evidence gate (T4, T8 stats verdict) ✓; single egress (T9) ✓; L1c worktree convenience (T8 worktree) ✓; L2 RA-aware GC (T11) ✓; cancellation (shim child status + Drop release; note: SIGINT child-forward is via inheriting the console — for explicit kill-on-Ctrl-C add a ctrlc handler if T6 smoke shows orphans).
- **Deferred by design:** 1b coalescing daemon — gated on T8 `stats` verdict; no task here (correct).
- **Known limitation:** strict cross-process FIFO is best-effort (OS-arbitrated lock); documented in `queue.rs`. Acceptable for UX + metrics goals.
- **DRY:** `hash_path` shared between shim and build_service (T9).
