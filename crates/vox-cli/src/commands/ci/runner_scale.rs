//! `vox ci runner-scale` / `vox ci runner-preflight` — ephemeral self-hosted
//! CI runner autoscaler for the single-box Docker fleet.
//!
//! Replaces the two always-on `vox-runner-1/2` containers with a pool that
//! scales **0 ↔ N** on demand: when CI work is queued, spin up `--ephemeral`
//! one-shot runners (each runs a single job then deregisters); when the queue is
//! empty they exit and the box returns to **0 runners = 0 CPU**. Each runner is
//! CPU/memory-capped and mounts a shared cache volume so cold starts stay warm.
//!
//! Designed to be invoked periodically (see `scripts/ci-runners-up.vox`).
//! `runner-scale` is **dry-run by default** — it only mutates with `--apply`, so
//! it can never silently spawn a runaway pool.
//!
//! `runner-preflight` is the fail-fast guard: it errors immediately when no
//! self-hosted runner can serve the gate, instead of letting work queue forever.

use std::process::Command;

use anyhow::{Context, Result, anyhow};

/// Repository the runners attach to.
const REPO_SLUG: &str = "vox-foundation/vox";
const REPO_URL: &str = "https://github.com/vox-foundation/vox";
/// Reproducible runner image (see `infra/ci-runner/Dockerfile`).
const RUNNER_IMAGE: &str = "vox-ci-runner-local:latest";
/// Name prefix for autoscaled ephemeral containers (distinct from legacy `vox-runner-N`).
const EPHEMERAL_PREFIX: &str = "vox-runner-eph-";
const RUNNER_LABELS: &str = "self-hosted,linux,x64,docker,browser";
/// Shared Docker volume holding sccache + cargo registry so ephemeral runners
/// don't rebuild the world on every cold start.
const CACHE_VOLUME: &str = "vox-ci-runner-cache";

/// Per-runner CPU cap. 4 runners × 6 = 24 of the host's 32 threads, leaving 8
/// for Windows + interactive work (the rest of `.wslconfig processors`).
const CPUS_PER_RUNNER: &str = "6";
const MEM_PER_RUNNER: &str = "6500m";
/// Hard ceiling on concurrently-running ephemeral runners.
pub const MAX_RUNNERS: u32 = 4;

// ---------------------------------------------------------------------------
// Pure scaling math (unit-tested)
// ---------------------------------------------------------------------------

/// Desired runner count for a given demand, capped at `max`.
pub fn desired_runner_count(demand: u32, max: u32) -> u32 {
    demand.min(max)
}

/// How many new runners to spawn this cycle (never negative; running runners are
/// never force-killed — ephemeral ones exit on their own after one job).
pub fn spawn_count(desired: u32, current: u32) -> u32 {
    desired.saturating_sub(current)
}

// ---------------------------------------------------------------------------
// IO: GitHub demand + Docker fleet
// ---------------------------------------------------------------------------

fn gh_json(args: &[&str]) -> Result<String> {
    // vox-arch-check: allow git-exec
    let out = Command::new("gh")
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

/// CI demand = count of queued workflow runs (a cheap proxy for "work waiting").
fn query_demand() -> Result<u32> {
    let s = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runs?status=queued&per_page=1"),
        "--jq",
        ".total_count",
    ])?;
    Ok(s.parse::<u32>().unwrap_or(0))
}

/// Online self-hosted runners (any name) — used by the preflight.
fn online_runner_count() -> Result<u32> {
    let s = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runners"),
        "--jq",
        "[.runners[]|select(.status==\"online\")]|length",
    ])?;
    Ok(s.parse::<u32>().unwrap_or(0))
}

fn docker(args: &[&str]) -> Result<String> {
    // vox-arch-check: allow git-exec
    let out = Command::new("docker")
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

/// Count running ephemeral runner containers.
fn running_ephemeral() -> Result<u32> {
    let out = docker(&[
        "ps",
        "--filter",
        &format!("name={EPHEMERAL_PREFIX}"),
        "--filter",
        "status=running",
        "--format",
        "{{.Names}}",
    ])?;
    Ok(out.lines().filter(|l| !l.trim().is_empty()).count() as u32)
}

/// Remove exited ephemeral containers (one-shot runners that finished a job).
fn reap_exited(dry_run: bool) -> Result<u32> {
    let out = docker(&[
        "ps",
        "-a",
        "--filter",
        &format!("name={EPHEMERAL_PREFIX}"),
        "--filter",
        "status=exited",
        "--format",
        "{{.Names}}",
    ])?;
    let names: Vec<&str> = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if !dry_run {
        for n in &names {
            let _ = docker(&["rm", "-f", n]);
        }
    }
    Ok(names.len() as u32)
}

/// Spawn one `--ephemeral` runner container with a fresh registration token.
fn spawn_one(index: u32, build_tag: &str, dry_run: bool) -> Result<()> {
    let name = format!("{EPHEMERAL_PREFIX}{build_tag}-{index}");
    if dry_run {
        println!(
            "[dry-run] would spawn ephemeral runner {name} ({CPUS_PER_RUNNER} cpu, {MEM_PER_RUNNER})"
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
        "--rm",
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
        "-e",
        "RUNNER_EPHEMERAL=1",
        "-v",
        "/var/run/docker.sock:/var/run/docker.sock",
        // Shared sccache dir → warm compiles across ephemeral runners (see
        // infra/ci-runner/Dockerfile: RUSTC_WRAPPER=sccache, SCCACHE_DIR=/cache/sccache).
        "-v",
        &format!("{CACHE_VOLUME}:/cache"),
        RUNNER_IMAGE,
    ])?;
    println!("spawned ephemeral runner {name}");
    Ok(())
}

/// A monotonic-ish tag so concurrently-spawned runners get unique names without
/// `Math::random`. Uses seconds-since-epoch (fine for naming).
fn build_tag() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs:x}")
}

/// `vox ci runner-scale` — reconcile the ephemeral pool to current demand.
pub fn run_scale(apply: bool) -> Result<()> {
    let dry_run = !apply;
    let demand = query_demand().unwrap_or(0);
    let current = running_ephemeral().unwrap_or(0);
    let desired = desired_runner_count(demand, MAX_RUNNERS);
    let spawn = spawn_count(desired, current);

    let reaped = reap_exited(dry_run)?;
    let tag = build_tag();
    for i in 0..spawn {
        spawn_one(i, &tag, dry_run)?;
    }

    println!(
        "runner-scale: dry_run={dry_run} demand={demand} current={current} desired={desired} spawned={spawn} reaped_exited={reaped} (max={MAX_RUNNERS})"
    );
    Ok(())
}

/// `vox ci runner-preflight` — error immediately if no online self-hosted runner
/// is available to serve the gate, so callers fail fast instead of queueing.
pub fn run_preflight() -> Result<()> {
    let online = online_runner_count().unwrap_or(0);
    if online == 0 {
        return Err(anyhow!(
            "no online self-hosted runner — the merge gate ({RUNNER_LABELS}) cannot run.\n\
             Bring the pool up:  vox run scripts/ci-runners-up.vox   (reconciles the ephemeral pool to demand)\n\
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
    fn desired_is_capped_at_max() {
        assert_eq!(desired_runner_count(0, 4), 0);
        assert_eq!(desired_runner_count(2, 4), 2);
        assert_eq!(desired_runner_count(4, 4), 4);
        assert_eq!(desired_runner_count(99, 4), 4);
    }

    #[test]
    fn spawn_is_delta_never_negative() {
        assert_eq!(spawn_count(4, 0), 4);
        assert_eq!(spawn_count(4, 2), 2);
        assert_eq!(spawn_count(2, 4), 0); // never force-kill running runners
        assert_eq!(spawn_count(0, 3), 0);
    }

    #[test]
    fn scale_to_zero_when_no_demand() {
        // No demand → desired 0 → spawn 0 regardless of current (existing
        // ephemeral runners exit on their own after their job).
        assert_eq!(spawn_count(desired_runner_count(0, MAX_RUNNERS), 3), 0);
    }
}
