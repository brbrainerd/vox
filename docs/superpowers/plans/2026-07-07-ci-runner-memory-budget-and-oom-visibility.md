# CI Runner Memory Budget + OOM Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the self-hosted CI runner fleet's containers from being silently OOM-killed mid-job (a measured ~12GB build peak against a 5GB-per-container limit), and make any future recurrence instantly diagnosable on the affected PR instead of requiring manual `dmesg` archaeology.

**Architecture:** Two independent changes in `crates/vox-cli/src/commands/ci/runner_scale.rs` (and a new sibling module `oom_watch.rs`): (A) raise the per-runner memory budget to a measured-safe value and shrink max concurrency to fit the host, and (B) have the existing 2-minute host-side autoscaler tick also scan `dmesg` for new memcg OOM-kill events, correlate each to the PR/run that was running on the killed container, and post a comment there — since the killed job's own execution environment is gone and can't self-report.

**Tech Stack:** Rust (`vox-cli` crate), `regex` (already a workspace dependency), `serde_json` (already used in this file for state persistence), `gh` CLI (already used throughout this file via the existing `gh_json` helper), `docker` CLI (already used via the existing `docker` helper), `wsl.exe` (new: to reach the WSL2 kernel's `dmesg` from the Windows-native `vox` binary).

Background/evidence for every design decision below: `docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md`.

---

## File Structure

- **Modify:** `crates/vox-cli/src/commands/ci/runner_scale.rs` — bump `MEM_PER_RUNNER`/`DEFAULT_MAX_RUNNERS`, extend the existing `fleet_budget_fits_wsl2_ceiling` test, wire the new OOM scan into `run_scale`'s tick.
- **Create:** `crates/vox-cli/src/commands/ci/oom_watch.rs` — all new OOM-detection logic: dmesg parsing, dedup persistence, container-name resolution, GitHub job correlation, PR comment composition/posting, and the top-level orchestration function `run_scale` calls into. Kept as its own file (not added to the already-1478-line `runner_scale.rs`) because it's a genuinely separate responsibility — detecting and reporting a failure mode, not scaling the fleet — with its own pure/IO split worth reading independently.
- **Modify:** `crates/vox-cli/src/commands/ci/mod.rs` — register the new module.

No new crate dependencies: `regex` is already a workspace dependency available to `vox-cli` (`vox-cli/Cargo.toml:228`), and `serde_json` is already used in `runner_scale.rs` for the existing state-persistence files.

---

### Task 1: Memory budget fix

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs:41-42` (constants), `:57-60` (comment + `DEFAULT_MAX_RUNNERS`), `:1266-1279` (test)

- [ ] **Step 1: Update the failing-first test to assert the new floor**

Edit the existing `fleet_budget_fits_wsl2_ceiling` test at `runner_scale.rs:1266-1279` to add a floor assertion tied to the measured real-world peak, so it fails against the *current* constants (5000m/6) before you touch them:

```rust
    #[test]
    fn fleet_budget_fits_wsl2_ceiling() {
        // WSL2 .wslconfig caps: processors=24, memory=32GB.
        let cpus: u32 = CPUS_PER_RUNNER.parse().unwrap();
        let mem_mb: u32 = MEM_PER_RUNNER.trim_end_matches('m').parse().unwrap();
        assert!(
            DEFAULT_MAX_RUNNERS * cpus <= 24,
            "fleet vCPU must fit WSL2 24-cpu cap"
        );
        assert!(
            DEFAULT_MAX_RUNNERS * mem_mb <= 32_000,
            "fleet RAM must fit WSL2 32GB cap"
        );
        // Floor tied to a measured real-world peak (2026-07-07: `cargo doc
        // --workspace --exclude vox-gui --no-deps` peaked at ~12.06GB RSS in an
        // uncapped measurement run — see the design doc). A future edit must
        // not silently shrink the budget back below what a real build in this
        // workspace actually needs.
        assert!(
            mem_mb >= 12_000,
            "MEM_PER_RUNNER must stay above the measured ~12GB build peak"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib fleet_budget_fits_wsl2_ceiling -- --nocapture`
Expected: FAIL — `MEM_PER_RUNNER must stay above the measured ~12GB build peak` (current value is `5000m` = `5_000`, below the `12_000` floor).

- [ ] **Step 3: Bump the constants**

Edit `runner_scale.rs:41-42`:

```rust
const CPUS_PER_RUNNER: &str = "4";
const MEM_PER_RUNNER: &str = "14000m";
```

Edit the comment + constant at `runner_scale.rs:57-60`:

```rust
/// Default ceiling on concurrent managed runners. Chosen from a MEASURED
/// peak, not an even division of host RAM: `cargo doc --workspace --exclude
/// vox-gui --no-deps` peaked at ~12.06GB RSS in a real, uncapped measurement
/// run (2026-07-07) — 2.4x the old 5GB-per-runner budget, which is why
/// runners were being memcg-OOM-killed mid-build well before their job's own
/// `timeout-minutes` (see docs/superpowers/specs/2026-07-07-ci-runner-memory-
/// budget-and-oom-visibility-design.md). `2 runners × 14000m = 28GB`, leaving
/// ~3GB headroom for the WSL2 VM/Docker daemon on this 31GB host.
/// Override: `VOX_RUNNER_MAX`.
pub const DEFAULT_MAX_RUNNERS: u32 = 2;
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-cli --lib fleet_budget_fits_wsl2_ceiling -- --nocapture`
Expected: PASS (`2 × 4 = 8 <= 24`; `2 × 14_000 = 28_000 <= 32_000`; `14_000 >= 12_000`).

- [ ] **Step 5: Run the full file's test suite to confirm nothing else broke**

Run: `cargo test -p vox-cli --lib runner_scale::`
Expected: all existing tests in this file still PASS — none of them assert a specific value for `MEM_PER_RUNNER`/`DEFAULT_MAX_RUNNERS` other than the one just edited, so no other breakage is expected.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "fix(ci): raise per-runner memory budget to measured-safe value

MEM_PER_RUNNER 5000m -> 14000m, DEFAULT_MAX_RUNNERS 6 -> 2. The old
5GB-per-runner budget was derived by evenly dividing host RAM by max
concurrency, never by measuring what a real build needs -- a real
cargo doc pass on this workspace peaked at ~12.06GB, so every heavy
job (Lints' clippy+rustdoc, Audits) was getting hard-killed by the
container's own memory cgroup well before its declared timeout."
```

---

### Task 2: OOM dmesg-line parser

**Files:**
- Create: `crates/vox-cli/src/commands/ci/oom_watch.rs`
- Modify: `crates/vox-cli/src/commands/ci/mod.rs`

- [ ] **Step 1: Register the new module**

Edit `crates/vox-cli/src/commands/ci/mod.rs`, inserting alphabetically before `mod operations_catalog;` (currently line 13):

```rust
mod oom_watch;
mod operations_catalog;
```

- [ ] **Step 2: Create the file with the module doc comment and the failing test**

Create `crates/vox-cli/src/commands/ci/oom_watch.rs`:

```rust
//! Host-side detection of self-hosted CI runner containers hard-killed by
//! their own memory cgroup limit — and reporting that evidence directly on
//! the PR/run that was affected.
//!
//! Two things this exists to work around (see the design doc for the
//! evidence): a runner container cannot read `dmesg` itself (no `CAP_SYSLOG`
//! by default, correctly so), and a job that gets OOM-killed cannot run its
//! own `if: always()` report step (the runner agent process dies with the
//! container — there is no "after" for that same job). So detection and
//! reporting both live here, on the host-side autoscaler tick
//! (`vox ci runner-scale`, invoked every 2 minutes), not inside the job.
//!
//! Design: docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use regex::Regex;

use super::constants::REPO_SLUG;
use super::runner_scale::{gh_json, quiet_command};

/// One parsed `oom-kill:constraint=CONSTRAINT_MEMCG` kernel log line — the
/// single `dmesg` line that carries both the killed process name and the
/// container's full cgroup id together (`oom_memcg=/docker/<id>,...,task=<name>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OomEvent {
    /// Raw matched line, used as the dedup key (content-based, not
    /// timestamp-based — avoids parsing dmesg's locale-dependent date format).
    pub raw_line: String,
    /// Killed process name, e.g. "rustdoc".
    pub process: String,
    /// Full 64-char docker container/cgroup id.
    pub cgroup_id: String,
}

fn oom_line_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"oom_memcg=/docker/([0-9a-f]{64}).*?task=([^,]+)")
            .expect("static oom-kill regex must compile")
    })
}

/// Parse `dmesg` output for `oom-kill:constraint=CONSTRAINT_MEMCG` lines,
/// extracting the killed process name and the container's cgroup id from
/// each. Lines that don't match (the overwhelming majority of `dmesg`) are
/// skipped. Pure — no IO.
pub fn parse_oom_events(dmesg_text: &str) -> Vec<OomEvent> {
    let re = oom_line_regex();
    dmesg_text
        .lines()
        .filter(|l| l.contains("oom-kill:constraint=CONSTRAINT_MEMCG"))
        .filter_map(|line| {
            let caps = re.captures(line)?;
            Some(OomEvent {
                raw_line: line.to_string(),
                cgroup_id: caps.get(1)?.as_str().to_string(),
                process: caps.get(2)?.as_str().to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_captured_oom_line() {
        // Verbatim (container id truncated for readability, but kept a valid
        // 64-char hex string) shape of a real line captured via
        // `wsl -e dmesg -T` during the 2026-07-07 investigation.
        let line = "[Tue Jul  7 07:53:04 2026] oom-kill:constraint=CONSTRAINT_MEMCG,\
                     nodemask=(null),cpuset=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     mems_allowed=0,oom_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task_memcg=/docker/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,\
                     task=rustdoc,pid=15612,uid=0";
        let events = parse_oom_events(line);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].process, "rustdoc");
        assert_eq!(
            events[0].cgroup_id,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(events[0].raw_line, line);
    }

    #[test]
    fn ignores_unrelated_dmesg_lines() {
        let text = "[1.0] some unrelated boot message\n\
                     [2.0] another line entirely\n";
        assert!(parse_oom_events(text).is_empty());
    }

    #[test]
    fn parses_multiple_events_and_skips_noise_between_them() {
        let text = format!(
            "[1.0] noise\n\
             [2.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{a},task_memcg=/docker/{a},task=cargo,pid=1,uid=0\n\
             [3.0] Memory cgroup out of memory: Killed process 1 (cargo)\n\
             [4.0] more noise\n\
             [5.0] oom-kill:constraint=CONSTRAINT_MEMCG,oom_memcg=/docker/{b},task_memcg=/docker/{b},task=rustc,pid=2,uid=0\n",
            a = "a".repeat(64),
            b = "b".repeat(64),
        );
        let events = parse_oom_events(&text);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].process, "cargo");
        assert_eq!(events[1].process, "rustc");
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib oom_watch::`
Expected: PASS — 3 tests (`parses_real_captured_oom_line`, `ignores_unrelated_dmesg_lines`, `parses_multiple_events_and_skips_noise_between_them`).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): add OOM dmesg-line parser for runner containers

Pure parser for the oom-kill:constraint=CONSTRAINT_MEMCG dmesg line
that carries both the killed process name and the container's cgroup
id in one place. First piece of host-side OOM visibility -- see
docs/superpowers/specs/2026-07-07-ci-runner-memory-budget-and-oom-visibility-design.md"
```

---

### Task 3: Dedup persistence across ticks

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`

- [ ] **Step 1: Write the failing tests for the pure dedup logic**

Add to the `tests` module in `oom_watch.rs` (after the existing tests, before the closing `}`):

```rust
    #[test]
    fn new_events_filters_out_already_seen_lines() {
        let events = vec![
            OomEvent {
                raw_line: "line-a".to_string(),
                process: "cargo".to_string(),
                cgroup_id: "a".repeat(64),
            },
            OomEvent {
                raw_line: "line-b".to_string(),
                process: "rustc".to_string(),
                cgroup_id: "b".repeat(64),
            },
        ];
        let seen = vec!["line-a".to_string()];
        let fresh = new_events(&events, &seen);
        assert_eq!(fresh.len(), 1);
        assert_eq!(fresh[0].raw_line, "line-b");
    }

    #[test]
    fn new_events_returns_all_when_seen_is_empty() {
        let events = vec![OomEvent {
            raw_line: "line-a".to_string(),
            process: "cargo".to_string(),
            cgroup_id: "a".repeat(64),
        }];
        assert_eq!(new_events(&events, &[]).len(), 1);
    }

    #[test]
    fn append_seen_caps_to_max_dropping_oldest_first() {
        let seen: Vec<String> = (0..OOM_SEEN_MAX).map(|i| format!("old-{i}")).collect();
        let newly = vec!["new-1".to_string(), "new-2".to_string()];
        let updated = append_seen(seen, &newly);
        assert_eq!(updated.len(), OOM_SEEN_MAX);
        // The two oldest entries were dropped to make room.
        assert!(!updated.contains(&"old-0".to_string()));
        assert!(!updated.contains(&"old-1".to_string()));
        assert!(updated.contains(&"old-2".to_string()));
        // The new entries are present.
        assert!(updated.contains(&"new-1".to_string()));
        assert!(updated.contains(&"new-2".to_string()));
    }

    #[test]
    fn append_seen_under_cap_keeps_everything() {
        let seen = vec!["a".to_string()];
        let updated = append_seen(seen, &["b".to_string()]);
        assert_eq!(updated, vec!["a".to_string(), "b".to_string()]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail (functions don't exist yet)**

Run: `cargo test -p vox-cli --lib oom_watch:: 2>&1 | head -30`
Expected: FAIL to compile — `cannot find function 'new_events'`, `cannot find function 'append_seen'`, `cannot find value 'OOM_SEEN_MAX'` in this scope.

- [ ] **Step 3: Implement the dedup persistence logic**

Add to `oom_watch.rs`, after the `parse_oom_events` function and before the `#[cfg(test)]` module:

```rust
// --- dedup persistence across ticks -----------------------------------

fn oom_seen_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-oom-seen.json")
}

fn read_oom_seen() -> Vec<String> {
    std::fs::read_to_string(oom_seen_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_oom_seen(seen: &[String]) {
    let p = oom_seen_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(seen) {
        let _ = std::fs::write(p, s);
    }
}

/// Events from `events` whose raw line isn't already in `seen`. Pure.
pub fn new_events<'a>(events: &'a [OomEvent], seen: &[String]) -> Vec<&'a OomEvent> {
    events
        .iter()
        .filter(|e| !seen.iter().any(|s| s == &e.raw_line))
        .collect()
}

/// Cap on the seen-list so its state file never grows unbounded.
const OOM_SEEN_MAX: usize = 500;

/// Append newly-seen raw lines to the existing seen-list, capped to
/// [`OOM_SEEN_MAX`] most-recent entries (oldest dropped first). Pure.
pub fn append_seen(mut seen: Vec<String>, newly_seen: &[String]) -> Vec<String> {
    seen.extend(newly_seen.iter().cloned());
    if seen.len() > OOM_SEEN_MAX {
        let drop = seen.len() - OOM_SEEN_MAX;
        seen.drain(0..drop);
    }
    seen
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib oom_watch:: `
Expected: PASS — 7 tests total now (3 from Task 2 + 4 new).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): add OOM event dedup persistence across autoscaler ticks

Mirrors the existing ci-runner-idle.json/ci-runner-phantom.json state
persistence pattern in runner_scale.rs -- content-based dedup on the
raw dmesg line (avoids parsing dmesg's locale-dependent timestamp
format), capped at 500 entries."
```

---

### Task 4: Container name resolution

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs:37`

- [ ] **Step 1: Widen `MANAGED_PREFIX`'s visibility for cross-module use**

`oom_watch.rs` needs `MANAGED_PREFIX` to filter container names to only
managed runners, but it's currently a plain private `const` — only visible
within `runner_scale.rs` itself, not to a sibling module. Every other
cross-module helper this file already exposes (`gh_json`, `quiet_command`,
`now_secs`) uses `pub(crate)` for exactly this reason; do the same here.

Edit `runner_scale.rs:37`:

```rust
/// Name prefix for autoscaler-managed runner containers.
pub(crate) const MANAGED_PREFIX: &str = "vox-runner-auto-";
```

- [ ] **Step 2: Add the import to `oom_watch.rs`**

Edit the `use` block at the top of `oom_watch.rs`:

```rust
use super::constants::REPO_SLUG;
use super::runner_scale::{MANAGED_PREFIX, gh_json, quiet_command};
```

- [ ] **Step 3: Write the failing test for the pure docker-events parser**

Add to the `tests` module in `oom_watch.rs`:

```rust
    #[test]
    fn parses_container_names_from_real_docker_events_format() {
        // Verbatim shape of real docker events output captured during the
        // 2026-07-07 investigation (ids shortened-then-padded to stay valid
        // 64-char hex for the fixture).
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let text = format!(
            "2026-07-07T08:14:08.590272062-04:00 container kill {a} (image=vox-ci-runner-local:latest, name=vox-runner-auto-6a4cebb2-0)\n\
             2026-07-07T08:14:08.766164548-04:00 container die {a} (exitCode=137, name=vox-runner-auto-6a4cebb2-0)\n\
             2026-07-07T08:14:10.722839758-04:00 container start {b} (name=vox-runner-auto-6a4ced91-0)\n\
             2026-07-07T08:06:39.404509619-04:00 container exec_die cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc (name=vox_clickhouse)\n"
        );
        let names = parse_container_names(&text);
        assert_eq!(names.get(a.as_str()), Some(&"vox-runner-auto-6a4cebb2-0".to_string()));
        assert_eq!(names.get(b.as_str()), Some(&"vox-runner-auto-6a4ced91-0".to_string()));
        // Non-managed containers (no MANAGED_PREFIX) must be filtered out --
        // vox_clickhouse is real host traffic we don't care about here.
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn parse_container_names_empty_on_no_matches() {
        assert!(parse_container_names("no container events here\n").is_empty());
    }
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib oom_watch:: 2>&1 | head -20`
Expected: FAIL to compile — `cannot find function 'parse_container_names'`.

- [ ] **Step 5: Implement the container-events parser and fetch wrapper**

Add to `oom_watch.rs`, after the dedup-persistence block and before `#[cfg(test)]`:

```rust
// --- container name resolution ------------------------------------------

fn container_event_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"container \S+ ([0-9a-f]{64}) \(.*?name=([^,)]+)")
            .expect("static container-event regex must compile")
    })
}

/// Parse `docker events` text into an id→name map, restricted to managed
/// runner containers (`MANAGED_PREFIX`). Covers already-destroyed containers
/// too (the whole point — an OOM-killed runner is gone by the time the next
/// tick polls), since `docker events` is a historical log, not a live query.
/// Pure — no IO.
pub fn parse_container_names(events_text: &str) -> HashMap<String, String> {
    let re = container_event_regex();
    let mut map = HashMap::new();
    for line in events_text.lines() {
        let Some(caps) = re.captures(line) else {
            continue;
        };
        let (Some(id), Some(name)) = (caps.get(1), caps.get(2)) else {
            continue;
        };
        let name = name.as_str();
        if name.starts_with(MANAGED_PREFIX) {
            map.insert(id.as_str().to_string(), name.to_string());
        }
    }
    map
}

/// Window (seconds) of `docker events` history to fetch when resolving a
/// cgroup id to a container name. Comfortably covers the 2-minute autoscaler
/// tick cadence with margin for a slow tick.
const OOM_EVENTS_WINDOW_SECS: i64 = 600;

/// Fetch `docker events` for the last [`OOM_EVENTS_WINDOW_SECS`], bounded by
/// `--since`/`--until` (both unix seconds) so this returns immediately rather
/// than streaming.
fn fetch_recent_container_events(now: i64) -> Result<String> {
    let since = (now - OOM_EVENTS_WINDOW_SECS).to_string();
    let until = now.to_string();
    let out = quiet_command("docker")
        .args([
            "events",
            "--since",
            &since,
            "--until",
            &until,
            "--filter",
            "type=container",
        ])
        .output()
        .context("run docker events")?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib oom_watch::`
Expected: PASS — 9 tests total now.

- [ ] **Step 7: Run the runner_scale suite to confirm the visibility bump didn't break anything**

Run: `cargo test -p vox-cli --lib runner_scale::`
Expected: PASS — widening `MANAGED_PREFIX` from private to `pub(crate)` only relaxes visibility, so every existing use of it in `runner_scale.rs` keeps compiling unchanged.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): resolve OOM cgroup ids to runner container names

Parses docker events history (bounded --since/--until window) into an
id-to-name map covering already-destroyed containers, since an
OOM-killed runner is gone by the time the next 2-minute tick polls.
Widens MANAGED_PREFIX to pub(crate) so oom_watch.rs can filter to
managed runners only, matching gh_json/quiet_command/now_secs, which
are already pub(crate) for the same cross-module reason."
```

---

### Task 5: GitHub job/run correlation

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`

- [ ] **Step 1: Write the failing test for the pure job-matching logic**

Add to the `tests` module in `oom_watch.rs`:

```rust
    #[test]
    fn find_matching_job_returns_the_matching_row() {
        let rows = vec![
            ("vox-runner-auto-aaa-0".to_string(), "Lints (clippy + rustdoc)".to_string()),
            ("vox-runner-auto-bbb-0".to_string(), "Audits".to_string()),
        ];
        assert_eq!(
            find_matching_job(&rows, "vox-runner-auto-bbb-0"),
            Some("Audits")
        );
    }

    #[test]
    fn find_matching_job_none_when_no_row_matches() {
        let rows = vec![("vox-runner-auto-aaa-0".to_string(), "Lints".to_string())];
        assert_eq!(find_matching_job(&rows, "vox-runner-auto-zzz-9"), None);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib oom_watch:: 2>&1 | head -20`
Expected: FAIL to compile — `cannot find function 'find_matching_job'`.

- [ ] **Step 3: Implement the pure matcher and its IO wrapper**

Add to `oom_watch.rs`, after the container-name-resolution block and before `#[cfg(test)]`:

```rust
// --- GitHub job/run correlation ------------------------------------------

/// One (runner_name, job_name) row from a workflow run's jobs list. Pure —
/// testable without a live `gh` call, mirroring how `runner_scale::runner_rows`
/// separates the tab-parsing shape from the `gh api` call that produces it.
pub fn find_matching_job<'a>(job_rows: &'a [(String, String)], runner_name: &str) -> Option<&'a str> {
    job_rows
        .iter()
        .find(|(rn, _)| rn == runner_name)
        .map(|(_, name)| name.as_str())
}

/// A workflow run this OOM event corresponds to: run id, originating PR
/// number, and the job name that was executing on the killed runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerJobMatch {
    pub run_id: u64,
    pub pr_number: u64,
    pub job_name: String,
}

/// Cap on recent runs inspected per status when correlating a runner name to
/// a job — mirrors `runner_scale::DEMAND_RUNS_PER_STATUS`.
const CORRELATE_RUNS_PER_STATUS: u32 = 20;

/// Find which job (if any) was assigned to `runner_name`, scanning recent
/// in_progress then completed runs. GitHub Actions job objects expose a
/// `runner_name` field once a job is assigned to a runner.
fn find_run_for_runner(runner_name: &str) -> Result<Option<RunnerJobMatch>> {
    for status in ["in_progress", "completed"] {
        let runs = gh_json(&[
            "api",
            &format!(
                "repos/{REPO_SLUG}/actions/runs?status={status}&per_page={CORRELATE_RUNS_PER_STATUS}"
            ),
            "--jq",
            r#".workflow_runs[]|[.id, (.pull_requests[0].number // 0)]|@tsv"#,
        ])?;
        for line in runs.lines() {
            let mut parts = line.split('\t');
            let (Some(run_id_str), Some(pr_str)) = (parts.next(), parts.next()) else {
                continue;
            };
            let (Ok(run_id), Ok(pr_number)) =
                (run_id_str.parse::<u64>(), pr_str.parse::<u64>())
            else {
                continue;
            };
            if pr_number == 0 {
                continue; // not a PR-triggered run — no PR to comment on
            }
            let job_raw = gh_json(&[
                "api",
                &format!("repos/{REPO_SLUG}/actions/runs/{run_id}/jobs?per_page=100"),
                "--jq",
                r#".jobs[]|select(.runner_name != null)|[.runner_name, .name]|@tsv"#,
            ])?;
            let job_rows: Vec<(String, String)> = job_raw
                .lines()
                .filter_map(|l| {
                    let mut p = l.split('\t');
                    Some((p.next()?.to_string(), p.next()?.to_string()))
                })
                .collect();
            if let Some(job_name) = find_matching_job(&job_rows, runner_name) {
                return Ok(Some(RunnerJobMatch {
                    run_id,
                    pr_number,
                    job_name: job_name.to_string(),
                }));
            }
        }
    }
    Ok(None)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib oom_watch::`
Expected: PASS — 11 tests total now.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): correlate a killed runner container to its PR/run

find_run_for_runner scans recent in_progress/completed workflow runs'
jobs for a runner_name match, extracting the PR number so the OOM
comment can land on the affected PR directly. Pure job-matching logic
(find_matching_job) split from the gh api IO, same pattern as
runner_scale::runner_rows."
```

---

### Task 6: PR comment composition and posting

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`

- [ ] **Step 1: Write the failing test for the pure comment-body builder**

Add to the `tests` module in `oom_watch.rs`:

```rust
    #[test]
    fn oom_comment_body_includes_job_process_and_raw_evidence() {
        let event = OomEvent {
            raw_line: "the raw dmesg line".to_string(),
            process: "rustdoc".to_string(),
            cgroup_id: "a".repeat(64),
        };
        let body = oom_comment_body(&event, "Lints (clippy + rustdoc)", 28861698905);
        assert!(body.contains("Lints (clippy + rustdoc)"));
        assert!(body.contains("28861698905"));
        assert!(body.contains("rustdoc"));
        assert!(body.contains("the raw dmesg line"));
        assert!(body.contains("OOM"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli --lib oom_watch:: 2>&1 | head -20`
Expected: FAIL to compile — `cannot find function 'oom_comment_body'`.

- [ ] **Step 3: Implement the comment builder and posting wrapper**

Add to `oom_watch.rs`, after the GitHub-correlation block and before `#[cfg(test)]`:

```rust
// --- PR comment composition and posting ----------------------------------

/// Build the PR comment body for one detected OOM kill. Pure — testable
/// without a live `gh` call.
pub fn oom_comment_body(event: &OomEvent, job_name: &str, run_id: u64) -> String {
    format!(
        "**CI runner OOM-killed** — job `{job_name}` (run `{run_id}`) did not fail \
         normally: its runner container's process `{}` was killed by the kernel's \
         per-container memory cgroup limit, not a real `timeout-minutes` cutoff or an \
         external cancellation.\n\n\
         Evidence (`dmesg`):\n```\n{}\n```\n\n\
         Auto-detected by the host-side runner autoscaler (`vox ci runner-scale`) — \
         no action needed unless this recurs after a `MEM_PER_RUNNER` bump.",
        event.process, event.raw_line
    )
}

/// Post `body` as a comment on PR `pr_number`. No-op (prints instead) when
/// `dry_run` — mirrors this command's existing `--apply`-gated mutation
/// pattern (`reap`, `deregister` etc. in `runner_scale.rs`).
fn post_pr_comment(pr_number: u64, body: &str, dry_run: bool) -> Result<()> {
    if dry_run {
        println!("[dry-run] would comment on PR #{pr_number}:\n{body}");
        return Ok(());
    }
    gh_json(&[
        "api",
        "-X",
        "POST",
        &format!("repos/{REPO_SLUG}/issues/{pr_number}/comments"),
        "-f",
        &format!("body={body}"),
    ])?;
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-cli --lib oom_watch::`
Expected: PASS — 12 tests total now.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs
git commit -m "feat(ci): compose and post the OOM evidence PR comment

oom_comment_body is a pure, tested string builder; post_pr_comment is
a thin gh api wrapper gated on the same dry_run flag runner_scale.rs
already threads through every other mutation."
```

---

### Task 7: Top-level orchestration and wiring into the autoscaler tick

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/oom_watch.rs`
- Modify: `crates/vox-cli/src/commands/ci/runner_scale.rs:801-809` (insert point)

- [ ] **Step 1: Implement the orchestration function**

Add to `oom_watch.rs`, after the PR-comment block and before `#[cfg(test)]`:

```rust
// --- orchestration ---------------------------------------------------------

/// Scan for new OOM-kill events since the last tick, correlate each to the
/// PR/run that was executing on the killed container, and post a comment
/// there. Best-effort: any IO failure for one event is logged and skipped
/// rather than aborting the whole scan, matching this file's degraded-not-
/// fatal error handling elsewhere. Returns the count of successfully
/// reported events.
pub fn scan_and_report_oom_events(dry_run: bool, now: i64) -> Result<u32> {
    let dmesg_out = quiet_command("wsl")
        .args(["-e", "dmesg", "-T"])
        .output()
        .context("run dmesg via wsl (is WSL2 available on this host?)")?;
    let dmesg_text = String::from_utf8_lossy(&dmesg_out.stdout);
    let events = parse_oom_events(&dmesg_text);

    let seen = read_oom_seen();
    let fresh = new_events(&events, &seen);
    if fresh.is_empty() {
        return Ok(0);
    }

    let events_text = fetch_recent_container_events(now)?;
    let names = parse_container_names(&events_text);

    let mut reported = 0u32;
    let mut newly_seen = Vec::new();
    for event in &fresh {
        newly_seen.push(event.raw_line.clone());
        let Some(container_name) = names.get(&event.cgroup_id) else {
            eprintln!(
                "runner-scale: OOM event on cgroup {} (process {}) — no matching \
                 managed container name found in the last {OOM_EVENTS_WINDOW_SECS}s of \
                 docker events, skipping",
                event.cgroup_id, event.process
            );
            continue;
        };
        match find_run_for_runner(container_name) {
            Ok(Some(m)) => {
                let body = oom_comment_body(event, &m.job_name, m.run_id);
                match post_pr_comment(m.pr_number, &body, dry_run) {
                    Ok(()) => reported += 1,
                    Err(e) => eprintln!("runner-scale: OOM comment post failed (degraded): {e:#}"),
                }
            }
            Ok(None) => {
                eprintln!(
                    "runner-scale: OOM on {container_name} — no PR-triggered job match found \
                     in recent runs"
                );
            }
            Err(e) => {
                eprintln!("runner-scale: OOM job correlation failed (degraded): {e:#}");
            }
        }
    }

    let updated_seen = append_seen(seen, &newly_seen);
    if !dry_run {
        write_oom_seen(&updated_seen);
    }
    Ok(reported)
}
```

- [ ] **Step 2: Run the full file's tests to confirm the new code compiles cleanly**

Run: `cargo test -p vox-cli --lib oom_watch::`
Expected: PASS — same 12 tests as Task 6 (this step adds no new unit tests, since `scan_and_report_oom_events` is an IO-heavy orchestration function — same convention as `runner_scale::run_scale` itself, which also has no direct unit test; only its pure helpers do).

- [ ] **Step 3: Wire the scan into `run_scale`'s tick**

Edit `runner_scale.rs`. Immediately after the existing block at lines 801-809:

```rust
    // 0. Local-first CI: auto-clear superseded/stale runs and refresh the
    //    queue snapshot every tick (stale sweep self-disables at fleet 0).
    let (cleared_superseded, cleared_stale) = super::queue::auto_clear_and_snapshot(dry_run, now)
        .unwrap_or_else(|e| {
            eprintln!("runner-scale: queue auto-clear skipped (degraded): {e:#}");
            (0, 0)
        });

    if let Some(lock) = _lock.as_ref() {
        lock.refresh(now_secs());
    }
```

Insert a new step immediately after it (before the blank line that precedes `let max = max_runners();`):

```rust
    // 0.5. OOM-visibility: detect any runner container hard-killed by its own
    //      memory cgroup limit since the last tick, and comment on the
    //      affected PR/run directly — the job itself can't self-report, since
    //      its whole execution environment (the runner agent process) died
    //      with the container.
    let oom_reported = super::oom_watch::scan_and_report_oom_events(dry_run, now)
        .unwrap_or_else(|e| {
            eprintln!("runner-scale: OOM-visibility scan skipped (degraded): {e:#}");
            0
        });
    if oom_reported > 0 {
        println!("runner-scale: reported {oom_reported} OOM-killed job(s) this tick");
    }
```

- [ ] **Step 4: Run the runner_scale test suite to confirm the wiring compiles and nothing broke**

Run: `cargo test -p vox-cli --lib runner_scale::`
Expected: PASS — all existing tests still pass (the new step is called from `run_scale`, which has no direct unit test itself, matching its existing untested-orchestration status).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/oom_watch.rs crates/vox-cli/src/commands/ci/runner_scale.rs
git commit -m "feat(ci): wire OOM visibility into the autoscaler's 2-minute tick

scan_and_report_oom_events now runs every tick alongside the existing
queue auto-clear step, degraded-not-fatal on any IO failure. A future
OOM-killed job is now self-diagnosing on the affected PR within about
2 minutes instead of requiring manual dmesg archaeology."
```

---

### Task 8: Full-workspace sanity pass

**Files:** none (verification only)

- [ ] **Step 1: Run the full vox-cli test suite**

Run: `cargo test -p vox-cli --lib`
Expected: PASS — no regressions anywhere in the crate from either Task 1's constant changes or the new `oom_watch` module.

- [ ] **Step 2: Run clippy on the touched crate**

Run: `cargo clippy -p vox-cli --all-targets -- -D warnings`
Expected: no new warnings from `oom_watch.rs` or the `runner_scale.rs` edits. Fix any that appear before proceeding (e.g. an unused import, a needless clone) rather than suppressing them.

- [ ] **Step 3: Build the release-relevant binary and dry-run the real command locally**

Run: `cargo build -p vox-cli --bin vox` then `./target/debug/vox ci runner-scale` (no `--apply` — dry-run by default)
Expected: exits 0, prints a `runner-scale: dry_run=true ...` summary line same as before this change, and does **not** crash even if `wsl -e dmesg -T` or `docker events` produce zero matching OOM lines (the common case) — confirming the new step degrades to "0 reported" silently rather than erroring out a normal tick.

- [ ] **Step 4: Confirm the `ci_workflow_contract` guard test (if any references these files) still passes**

Run: `cargo test -p vox-cli --test ci_workflow_contract`
Expected: PASS — this plan doesn't touch `.github/workflows/ci.yml` at all (the fix is entirely in the host-side autoscaler binary, not the workflow file), so this guard should be unaffected; running it is a cheap confirmation there's no unexpected coupling.

- [ ] **Step 5: Final commit if any fixes were needed in Steps 1-4**

If clippy or the dry-run surfaced anything, fix it, re-run the relevant check from Steps 1-4, then:

```bash
git add -A
git commit -m "fix(ci): address clippy/sanity-pass findings in OOM visibility"
```

If nothing needed fixing, no commit is needed for this task — it was verification-only.
