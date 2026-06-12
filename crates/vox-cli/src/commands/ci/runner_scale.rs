//! `vox ci runner-scale` / `vox ci runner-preflight` — autoscaler for the
//! single-box self-hosted CI runner pool.
//!
//! **Model: persistent runners + idle-reap.** Each runner is *persistent* (it
//! keeps its container alive and handles many jobs in a row), so the cargo
//! `target/` dir stays warm across jobs — the big win for a heavy Rust workspace,
//! where re-linking a cold target dominates per-job time. GitHub distributes
//! queued jobs across whichever runners are free, so N runners = N-way
//! parallelism for free.
//!
//! The pool scales **up** to demand (capped at [`MAX_RUNNERS`]) and **down** by
//! reaping any runner that has been **idle longer than [`IDLE_REAP_SECS`]** —
//! recouping the box when CI quiets, without paying a fresh startup for every
//! single job. Idle duration is tracked in a small JSON state file across ticks.
//!
//! Invoked periodically (Task Scheduler / `scripts/ci-runners-up.vox`).
//! `runner-scale` is **dry-run by default**; `--apply` mutates.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};

const REPO_SLUG: &str = "vox-foundation/vox";
const REPO_URL: &str = "https://github.com/vox-foundation/vox";
const RUNNER_IMAGE: &str = "vox-ci-runner-local:latest";
/// Name prefix for autoscaler-managed runner containers.
const MANAGED_PREFIX: &str = "vox-runner-auto-";
const RUNNER_LABELS: &str = "self-hosted,linux,x64,docker,browser";
const CACHE_VOLUME: &str = "vox-ci-runner-cache";

const CPUS_PER_RUNNER: &str = "6";
const MEM_PER_RUNNER: &str = "6500m";
/// Hard ceiling on concurrent managed runners (4 × 6 cpu = 24 of 32 threads).
pub const MAX_RUNNERS: u32 = 4;
/// Reap a runner after this many seconds of continuous idle (no job). 30 min:
/// long enough to amortize startup + reuse warm caches, short enough to free the
/// box when CI is quiet.
pub const IDLE_REAP_SECS: i64 = 1800;

// ---------------------------------------------------------------------------
// Pure logic (unit-tested)
// ---------------------------------------------------------------------------

/// Desired runner count for a given demand, capped at `max`.
pub fn desired_runner_count(demand: u32, max: u32) -> u32 {
    demand.min(max)
}

/// How many new runners to spawn this cycle (never negative).
pub fn spawn_count(desired: u32, keep: u32) -> u32 {
    desired.saturating_sub(keep)
}

/// Updated idle-since for a runner this tick: `None` if busy (resets the timer),
/// else the existing idle-since (or `now` if it just went idle).
pub fn next_idle_since(busy: bool, prev_idle_since: Option<i64>, now: i64) -> Option<i64> {
    if busy {
        None
    } else {
        Some(prev_idle_since.unwrap_or(now))
    }
}

/// True when an idle runner has been idle long enough to reap.
pub fn should_reap_idle(idle_since: Option<i64>, now: i64, timeout: i64) -> bool {
    match idle_since {
        Some(since) => now - since >= timeout,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// IO: GitHub + Docker
// ---------------------------------------------------------------------------

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Build a `Command` that never flashes a console window on Windows.
///
/// The autoscaler runs every tick on a schedule and shells out to `gh`/`docker`
/// many times per launch/reap cycle; without `CREATE_NO_WINDOW` each child pops a
/// blank console window on the desktop. No-op on non-Windows.
fn quiet_command(program: &str) -> Command {
    // vox-arch-check: allow git-exec
    // `mut` is only used under #[cfg(windows)] below; on other targets the binding
    // is never mutated, so silence the otherwise-correct unused_mut lint there.
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn gh_json(args: &[&str]) -> Result<String> {
    let out = quiet_command("gh")
        .args(args)
        .output()
        .context("run gh (is the GitHub CLI installed and authenticated?)")?;
    if !out.status.success() {
        return Err(anyhow!(
            "gh {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn docker(args: &[&str]) -> Result<String> {
    let out = quiet_command("docker")
        .args(args)
        .output()
        .context("run docker (is the daemon up?)")?;
    if !out.status.success() {
        return Err(anyhow!(
            "docker {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// CI demand = count of queued workflow runs.
fn query_demand() -> Result<u32> {
    let s = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runs?status=queued&per_page=1"),
        "--jq",
        ".total_count",
    ])?;
    Ok(s.parse::<u32>().unwrap_or(0))
}

/// Online self-hosted runners (any name) — for the preflight.
fn online_runner_count() -> Result<u32> {
    let s = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runners"),
        "--jq",
        "[.runners[]|select(.status==\"online\")]|length",
    ])?;
    Ok(s.parse::<u32>().unwrap_or(0))
}

/// `{name: busy}` for managed runners GitHub currently sees online.
fn managed_busy_map() -> Result<HashMap<String, bool>> {
    let raw = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runners"),
        "--paginate",
        "--jq",
        ".runners[]|select(.status==\"online\")|\"\\(.name)\\t\\(.busy)\"",
    ])?;
    let mut m = HashMap::new();
    for line in raw.lines() {
        if let Some((name, busy)) = line.split_once('\t') {
            if name.starts_with(MANAGED_PREFIX) {
                m.insert(name.to_string(), busy.trim() == "true");
            }
        }
    }
    Ok(m)
}

/// Deregister a runner from GitHub by name (best-effort).
fn deregister(name: &str) {
    if let Ok(id) = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runners"),
        "--jq",
        &format!(".runners[]|select(.name==\"{name}\")|.id"),
    ]) {
        if !id.is_empty() {
            let _ = gh_json(&[
                "api",
                "-X",
                "DELETE",
                &format!("repos/{REPO_SLUG}/actions/runners/{id}"),
            ]);
        }
    }
}

/// Names of managed containers in a given docker status (`running`/`exited`).
fn managed_containers(status: &str) -> Vec<String> {
    docker(&[
        "ps",
        "-a",
        "--filter",
        &format!("name={MANAGED_PREFIX}"),
        "--filter",
        &format!("status={status}"),
        "--format",
        "{{.Names}}",
    ])
    .map(|o| {
        o.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

/// Reap a managed runner: deregister from GitHub, then remove the container.
fn reap(name: &str, dry_run: bool, reason: &str) {
    if dry_run {
        println!("[dry-run] would reap {name} ({reason})");
        return;
    }
    eprintln!("[reap] {name} ({reason})");
    deregister(name);
    let _ = docker(&["rm", "-f", name]);
}

/// Spawn one **persistent** runner (handles many jobs; keeps a warm target dir).
fn spawn_one(index: u32, tag: &str, dry_run: bool) -> Result<()> {
    let name = format!("{MANAGED_PREFIX}{tag}-{index}");
    if dry_run {
        println!(
            "[dry-run] would spawn persistent runner {name} ({CPUS_PER_RUNNER} cpu, {MEM_PER_RUNNER})"
        );
        return Ok(());
    }
    let token = gh_json(&[
        "api",
        "-X",
        "POST",
        &format!("repos/{REPO_SLUG}/actions/runners/registration-token"),
        "--jq",
        ".token",
    ])?;
    docker(&[
        "run",
        "-d",
        // tini as PID 1 reaps zombie/orphaned job children (rustc, etc.) that the
        // actions runner (run.sh) would otherwise leave defunct after a cancelled
        // or crashed job.
        "--init",
        "--restart",
        "unless-stopped",
        "--name",
        &name,
        &format!("--cpus={CPUS_PER_RUNNER}"),
        &format!("--memory={MEM_PER_RUNNER}"),
        "-e",
        &format!("REPO_URL={REPO_URL}"),
        "-e",
        &format!("RUNNER_TOKEN={token}"),
        "-e",
        &format!("RUNNER_LABELS={RUNNER_LABELS}"),
        "-e",
        &format!("RUNNER_NAME={name}"),
        "-v",
        "/var/run/docker.sock:/var/run/docker.sock",
        "-v",
        &format!("{CACHE_VOLUME}:/cache"),
        RUNNER_IMAGE,
    ])?;
    println!("spawned persistent runner {name}");
    Ok(())
}

fn tag() -> String {
    format!("{:x}", now_secs())
}

// --- idle-state persistence ------------------------------------------------

fn state_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-idle.json")
}

fn read_state() -> HashMap<String, i64> {
    std::fs::read_to_string(state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_state(state: &HashMap<String, i64>) {
    let p = state_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(p, s);
    }
}

// ---------------------------------------------------------------------------
// Reconcile
// ---------------------------------------------------------------------------

/// `vox ci runner-scale` — reconcile the persistent pool to demand + reap idle.
pub fn run_scale(apply: bool) -> Result<()> {
    let dry_run = !apply;
    let now = now_secs();
    let prev = read_state();
    let busy_map = managed_busy_map().unwrap_or_default();

    // 1. Clean up any exited managed containers (crashed / stopped runners).
    let mut dead = 0u32;
    for name in managed_containers("exited") {
        if !dry_run {
            let _ = docker(&["rm", "-f", &name]);
        }
        dead += 1;
    }

    // 2. Walk running containers: keep active, reap long-idle, track idle timers.
    let mut new_state: HashMap<String, i64> = HashMap::new();
    let mut keep = 0u32;
    let mut reaped = 0u32;
    for name in managed_containers("running") {
        match busy_map.get(&name) {
            Some(true) => keep += 1, // active job
            Some(false) => {
                let idle_since = next_idle_since(false, prev.get(&name).copied(), now);
                if should_reap_idle(idle_since, now, IDLE_REAP_SECS) {
                    reap(&name, dry_run, "idle > reap timeout");
                    reaped += 1;
                } else {
                    if let Some(s) = idle_since {
                        new_state.insert(name.clone(), s);
                    }
                    keep += 1;
                }
            }
            None => keep += 1, // running but not yet registered (still starting)
        }
    }

    // 3. Scale up toward demand.
    let demand = query_demand().unwrap_or(0);
    let desired = desired_runner_count(demand, MAX_RUNNERS);
    let spawn = spawn_count(desired, keep);
    let t = tag();
    for i in 0..spawn {
        spawn_one(i, &t, dry_run)?;
    }

    if !dry_run {
        write_state(&new_state);
    }
    println!(
        "runner-scale: dry_run={dry_run} demand={demand} keep={keep} desired={desired} spawned={spawn} reaped_idle={reaped} cleaned_exited={dead} (max={MAX_RUNNERS}, idle_reap={IDLE_REAP_SECS}s)"
    );
    Ok(())
}

/// `vox ci runner-preflight` — error if no online self-hosted runner.
pub fn run_preflight() -> Result<()> {
    let online = online_runner_count().unwrap_or(0);
    if online == 0 {
        return Err(anyhow!(
            "no online self-hosted runner — the merge gate ({RUNNER_LABELS}) cannot run.\n\
             Bring the pool up:  vox run scripts/ci-runners-up.vox\n\
             Then re-check:      vox ci runner-preflight"
        ));
    }
    println!("runner-preflight: {online} self-hosted runner(s) online");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_capped_at_max() {
        assert_eq!(desired_runner_count(0, 4), 0);
        assert_eq!(desired_runner_count(2, 4), 2);
        assert_eq!(desired_runner_count(99, 4), 4);
    }

    #[test]
    fn spawn_is_delta_never_negative() {
        assert_eq!(spawn_count(4, 0), 4);
        assert_eq!(spawn_count(4, 2), 2);
        assert_eq!(spawn_count(2, 4), 0); // already at/over desired
    }

    #[test]
    fn idle_timer_resets_when_busy_and_persists_when_idle() {
        assert_eq!(next_idle_since(true, Some(100), 200), None); // busy → cleared
        assert_eq!(next_idle_since(false, None, 200), Some(200)); // just went idle
        assert_eq!(next_idle_since(false, Some(100), 200), Some(100)); // still idle
    }

    #[test]
    fn reap_only_after_timeout() {
        assert!(!should_reap_idle(None, 9999, 1800)); // active (no idle stamp)
        assert!(!should_reap_idle(Some(1000), 1000 + 1799, 1800)); // not yet
        assert!(should_reap_idle(Some(1000), 1000 + 1800, 1800)); // at timeout
        assert!(should_reap_idle(Some(1000), 1000 + 5000, 1800)); // well past
    }

    #[test]
    fn idle_lifecycle_keeps_then_reaps() {
        let now = 100_000;
        let idle_10m = next_idle_since(false, Some(now - 600), now);
        assert!(!should_reap_idle(idle_10m, now, IDLE_REAP_SECS));
        let idle_30m = next_idle_since(false, Some(now - 1800), now);
        assert!(should_reap_idle(idle_30m, now, IDLE_REAP_SECS));
    }
}
