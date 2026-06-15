//! `vox ci runner-scale` / `vox ci runner-preflight` — autoscaler for the
//! single-box self-hosted CI runner pool.
//!
//! **Model: ephemeral dispatched runners, scale 0 ↔ N.** Each runner container
//! registers with `--ephemeral`, takes exactly **one** job dispatched by
//! GitHub's queue, self-deregisters, and exits. The autoscaler is a reconcile
//! loop: each tick it counts **queued jobs** that match the pool's labels and
//! spawns `min(queued_jobs, max) - alive` new runners; exited containers are
//! removed the next tick. No restart policy is set, so a host/Docker restart
//! never resurrects a stale fleet that grabs every queued job at boot.
//!
//! Startup cost is mitigated by the shared `vox-ci-runner-cache` volume
//! (sccache) and an optional warm pool (`VOX_RUNNER_WARM_POOL`) that keeps N
//! idle runners registered for instant dispatch. Runners that registered but
//! never received a job (demand vanished, e.g. a cancelled run) are reaped
//! after a short idle grace window ([`DEFAULT_IDLE_REAP_SECS`]). Stale
//! **offline** GitHub registrations with no backing container are pruned.
//!
//! Knobs (env, optional): `VOX_RUNNER_MAX`, `VOX_RUNNER_IDLE_REAP_SECS`,
//! `VOX_RUNNER_WARM_POOL` — see `contracts/config/env-vars.v1.yaml`.
//!
//! Invoked periodically (Task Scheduler / `scripts/ci-runners-up.vox`).
//! `runner-scale` is **dry-run by default**; `--apply` mutates.

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};

use super::constants::REPO_SLUG;
const REPO_URL: &str = "https://github.com/vox-foundation/vox";
const RUNNER_IMAGE: &str = "vox-ci-runner-local:latest";
/// Name prefix for autoscaler-managed runner containers.
const MANAGED_PREFIX: &str = "vox-runner-auto-";
const RUNNER_LABELS: &str = "self-hosted,linux,x64,docker,browser";
const CACHE_VOLUME: &str = "vox-ci-runner-cache";

const CPUS_PER_RUNNER: &str = "4";
const MEM_PER_RUNNER: &str = "5000m";
/// Shared S3-compatible compile cache (MinIO container `vox-sccache-minio` on
/// this host; see docs/src/ci/shared-compile-cache.md). Runner containers reach
/// the host via Docker Desktop's `host.docker.internal`. The host-side probe
/// uses localhost. The bucket allows anonymous read/write on the LAN, so no
/// credentials are injected.
const SCCACHE_S3_BUCKET: &str = "vox-sccache";
const SCCACHE_S3_CONTAINER_ENDPOINT: &str = "http://host.docker.internal:9000";
const SCCACHE_S3_HOST_PROBE: &str = "127.0.0.1:9000";
/// Default ceiling on concurrent managed runners (6 runners × 4 cpu = 24 vCPU =
/// WSL2 processors cap; 6 × 5000m = 30 GB < 32 GB WSL2 memory cap).
/// Override: `VOX_RUNNER_MAX`.
pub const DEFAULT_MAX_RUNNERS: u32 = 6;
/// Reap a runner after this many seconds of continuous idle (registered but
/// never assigned a job — e.g. the queued run was cancelled). Ephemeral runners
/// exit on their own after their single job, so this is only a startup-grace
/// safety net, not the primary despawn path. Override: `VOX_RUNNER_IDLE_REAP_SECS`.
pub const DEFAULT_IDLE_REAP_SECS: i64 = 300;
/// Idle runners to keep registered for instant dispatch (0 = pure
/// scale-to-zero). Override: `VOX_RUNNER_WARM_POOL`.
pub const DEFAULT_WARM_POOL: u32 = 1;
/// Grace window before a phantom offline registration is deregistered from
/// GitHub. An offline runner with no backing container is assumed to be a
/// crashed ephemeral that never self-deregistered; after this window the
/// autoscaler removes its registration even when the fleet is busy.
/// Override: `VOX_RUNNER_PHANTOM_GRACE_SECS`.
pub const DEFAULT_PHANTOM_GRACE_SECS: i64 = 120;

/// Cap on workflow runs inspected per status when counting queued jobs.
const DEMAND_RUNS_PER_STATUS: u32 = 20;

// ---------------------------------------------------------------------------
// Config (env-overridable)
// ---------------------------------------------------------------------------

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn max_runners() -> u32 {
    env_u32("VOX_RUNNER_MAX", DEFAULT_MAX_RUNNERS)
}

fn idle_reap_secs() -> i64 {
    env_i64("VOX_RUNNER_IDLE_REAP_SECS", DEFAULT_IDLE_REAP_SECS)
}

fn warm_pool() -> u32 {
    env_u32("VOX_RUNNER_WARM_POOL", DEFAULT_WARM_POOL)
}

fn phantom_grace_secs() -> i64 {
    env_i64("VOX_RUNNER_PHANTOM_GRACE_SECS", DEFAULT_PHANTOM_GRACE_SECS)
}

// ---------------------------------------------------------------------------
// Pure logic (unit-tested)
// ---------------------------------------------------------------------------

/// Desired runner count: meet queued-job demand (capped at `max`), but never
/// drop below the warm pool (also capped at `max`).
pub fn desired_runner_count(demand: u32, max: u32, warm: u32) -> u32 {
    demand.min(max).max(warm.min(max))
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

/// Pick up to `count` idle runner names to reap when `total_keep > desired`.
/// Newest-idle runners go first (LIFO burst cleanup) so the longest-warm runner
/// is kept for `VOX_RUNNER_WARM_POOL`.
pub fn scale_down_reap_targets(idle: &[(String, i64)], count: u32) -> Vec<String> {
    if count == 0 || idle.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<_> = idle.to_vec();
    sorted.sort_by_key(|(_, since)| std::cmp::Reverse(*since));
    sorted
        .into_iter()
        .take(count as usize)
        .map(|(name, _)| name)
        .collect()
}

/// Count queued jobs whose label set the pool can serve. `label_lines` is one
/// job per line, each line a comma-separated label list (jq output); a job
/// matches when **every** label it requires is present on our runners.
pub fn count_matching_queued_jobs(label_lines: &str, runner_labels: &str) -> u32 {
    let pool: HashSet<&str> = runner_labels.split(',').map(str::trim).collect();
    label_lines
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| line.split(',').map(str::trim).all(|l| pool.contains(l)))
        .count() as u32
}

/// One registered runner as GitHub reports it: `(name, status, busy)`.
pub type RunnerRow = (String, String, bool);

/// Managed GitHub runner registrations that are **offline**, not busy, and have
/// no backing container, and whose first-seen-offline timestamp is older than
/// `grace_secs`. Returns `(name, first_seen)` pairs.
///
/// Unlike the old `stale_offline_registrations`, this function is *not*
/// gated on whether the fleet is busy — a phantom blocks a registration slot
/// regardless of load and must be pruned unconditionally once past grace.
pub fn phantom_offline_registrations<'a>(
    rows: &'a [RunnerRow],
    containers: &HashSet<String>,
    phantom_seen: &HashMap<String, i64>,
    now: i64,
    grace_secs: i64,
) -> Vec<(&'a str, i64)> {
    rows.iter()
        .filter(|(name, status, busy)| {
            name.starts_with(MANAGED_PREFIX)
                && status == "offline"
                && !busy
                && !containers.contains(name)
        })
        .filter_map(|(name, _, _)| {
            let first_seen = *phantom_seen.get(name.as_str()).unwrap_or(&now);
            if now - first_seen >= grace_secs {
                Some((name.as_str(), first_seen))
            } else {
                None
            }
        })
        .collect()
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
    #[allow(unused_mut)] // Windows-only mutation via creation_flags
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

/// CI demand = count of **queued jobs** (not workflow runs) that the pool's
/// label set can serve, across queued and in-progress runs. Early-exits once
/// `max` matching jobs are found — demand beyond the cap changes nothing.
fn query_queued_job_demand(max: u32) -> Result<u32> {
    let mut total = 0u32;
    for status in ["queued", "in_progress"] {
        let ids = gh_json(&[
            "api",
            &format!(
                "repos/{REPO_SLUG}/actions/runs?status={status}&per_page={DEMAND_RUNS_PER_STATUS}"
            ),
            "--jq",
            ".workflow_runs[].id",
        ])?;
        for id in ids.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let label_lines = gh_json(&[
                "api",
                &format!("repos/{REPO_SLUG}/actions/runs/{id}/jobs?per_page=100"),
                "--jq",
                ".jobs[]|select(.status==\"queued\")|(.labels|join(\",\"))",
            ])
            .unwrap_or_default();
            total = total.saturating_add(count_matching_queued_jobs(&label_lines, RUNNER_LABELS));
            if total >= max {
                return Ok(max);
            }
        }
    }
    Ok(total)
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

/// All registered runners as `(name, status, busy)` rows.
fn runner_rows() -> Result<Vec<RunnerRow>> {
    let raw = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runners"),
        "--paginate",
        "--jq",
        ".runners[]|\"\\(.name)\\t\\(.status)\\t\\(.busy)\"",
    ])?;
    let mut rows = Vec::new();
    for line in raw.lines() {
        let mut parts = line.split('\t');
        if let (Some(name), Some(status), Some(busy)) = (parts.next(), parts.next(), parts.next()) {
            rows.push((
                name.to_string(),
                status.trim().to_string(),
                busy.trim() == "true",
            ));
        }
    }
    Ok(rows)
}

/// `{name: busy}` for managed runners GitHub currently sees online.
fn managed_busy_map(rows: &[RunnerRow]) -> HashMap<String, bool> {
    rows.iter()
        .filter(|(name, status, _)| name.starts_with(MANAGED_PREFIX) && status == "online")
        .map(|(name, _, busy)| (name.clone(), *busy))
        .collect()
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

/// Spawn one **ephemeral** runner: it registers with `--ephemeral`, runs exactly
/// one dispatched job, self-deregisters, and exits. No restart policy — a
/// Docker/host restart must never resurrect a fleet that storms the queue.
/// Probe the shared MinIO compile cache from the host. Spawn-time check: if
/// the cache server is down, runners fall back to the per-host disk volume
/// (`SCCACHE_DIR=/cache/sccache` baked into the image) instead of failing
/// every compile against an unreachable S3 endpoint.
fn s3_cache_reachable() -> bool {
    use std::net::{SocketAddr, TcpStream};
    SCCACHE_S3_HOST_PROBE
        .parse::<SocketAddr>()
        .ok()
        .and_then(|addr| {
            TcpStream::connect_timeout(&addr, std::time::Duration::from_millis(800)).ok()
        })
        .is_some()
}

/// Env injected into runner containers to point sccache at the shared
/// S3-compatible cache. Empty when the cache is unreachable — the image's
/// disk-volume defaults then apply. `CARGO_INCREMENTAL=0` because sccache
/// cannot cache incremental compiles and ephemeral runners gain nothing from
/// incremental state anyway.
pub fn shared_cache_env(reachable: bool) -> Vec<(&'static str, String)> {
    if !reachable {
        return Vec::new();
    }
    vec![
        ("SCCACHE_BUCKET", SCCACHE_S3_BUCKET.to_string()),
        (
            "SCCACHE_ENDPOINT",
            SCCACHE_S3_CONTAINER_ENDPOINT.to_string(),
        ),
        ("SCCACHE_REGION", "us-east-1".to_string()),
        ("SCCACHE_S3_USE_SSL", "off".to_string()),
        ("SCCACHE_S3_NO_CREDENTIALS", "true".to_string()),
        ("CARGO_INCREMENTAL", "0".to_string()),
    ]
}

fn spawn_one(index: u32, tag: &str, dry_run: bool) -> Result<()> {
    let name = format!("{MANAGED_PREFIX}{tag}-{index}");
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
    let mut args: Vec<String> = vec![
        "run".into(),
        "-d".into(),
        // tini as PID 1 reaps zombie/orphaned job children (rustc, etc.) that the
        // actions runner (run.sh) would otherwise leave defunct after a cancelled
        // or crashed job.
        "--init".into(),
        "--name".into(),
        name.clone(),
        format!("--cpus={CPUS_PER_RUNNER}"),
        format!("--memory={MEM_PER_RUNNER}"),
        "-e".into(),
        format!("REPO_URL={REPO_URL}"),
        "-e".into(),
        format!("RUNNER_TOKEN={token}"),
        "-e".into(),
        format!("RUNNER_LABELS={RUNNER_LABELS}"),
        "-e".into(),
        format!("RUNNER_NAME={name}"),
        "-e".into(),
        "RUNNER_EPHEMERAL=1".into(),
        "-v".into(),
        // vox-arch-check: allow abs-path
        "/var/run/docker.sock:/var/run/docker.sock".into(),
        "-v".into(),
        format!("{CACHE_VOLUME}:/cache"),
    ];
    for (k, v) in shared_cache_env(s3_cache_reachable()) {
        args.push("-e".into());
        args.push(format!("{k}={v}"));
    }
    args.push(RUNNER_IMAGE.into());
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    docker(&arg_refs)?;
    println!("spawned ephemeral runner {name}");
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

// --- phantom-seen persistence -----------------------------------------------

fn phantom_state_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-phantom.json")
}

fn read_phantom_seen() -> HashMap<String, i64> {
    std::fs::read_to_string(phantom_state_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_phantom_seen(seen: &HashMap<String, i64>) {
    let p = phantom_state_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = serde_json::to_string_pretty(seen) {
        let _ = std::fs::write(p, s);
    }
}

// --- single-instance scale lock ---------------------------------------------

/// After this many seconds without a heartbeat the lock is considered stale
/// and a new instance may steal it. Covers Task Scheduler double-fire + host
/// clock jitter.
const LOCK_STALE_SECS: i64 = 90;

/// True when the lock file is older than [`LOCK_STALE_SECS`].
pub fn scale_lock_is_stale(written_at: i64, now: i64) -> bool {
    now - written_at >= LOCK_STALE_SECS
}

fn scale_lock_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-scale.lock")
}

/// RAII guard that holds the scale lock file for the duration of an apply run.
/// Constructed via [`ScaleLock::acquire`]; released (file removed) on drop.
pub struct ScaleLock {
    path: PathBuf,
}

impl ScaleLock {
    /// Try to acquire the lock. Returns `Ok(None)` when another instance holds
    /// a fresh lock (caller should exit early / skip the apply). Returns
    /// `Ok(Some(_))` when the lock was acquired (fresh or stale).
    pub fn acquire(now: i64) -> Result<Option<Self>> {
        let path = scale_lock_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        // Check for a live lock held by another instance.
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(written_at) = contents.trim().parse::<i64>() {
                if !scale_lock_is_stale(written_at, now) {
                    return Ok(None); // another instance holds a fresh lock
                }
            }
        }
        // Write our timestamp; best-effort (if it fails we skip locking but
        // don't fail the whole command — safer than blocking all autoscaling).
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = writeln!(f, "{now}");
        }
        Ok(Some(ScaleLock { path }))
    }
}

impl Drop for ScaleLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// --- durable decision log ---------------------------------------------------

/// Maximum number of lines to keep in the history file. Older lines are
/// rotated out when the file exceeds this cap.
const HISTORY_MAX_LINES: usize = 10_000;

/// Build the JSONL decision-log entry for one reconcile tick.
///
/// All 12 numeric fields are included so the log is self-contained for
/// downstream analysis without requiring the running binary.
#[allow(clippy::too_many_arguments)]
pub fn scale_event_json(
    ts: i64,
    dry_run: bool,
    queued_jobs: u32,
    keep: u32,
    desired: u32,
    spawned: u32,
    reaped_scale_down: u32,
    reaped_idle: u32,
    pruned_phantom: u32,
    cleaned_exited: u32,
    max: u32,
    warm: u32,
) -> String {
    format!(
        r#"{{"ts":{ts},"dry_run":{dry_run},"queued_jobs":{queued_jobs},"keep":{keep},"desired":{desired},"spawned":{spawned},"reaped_scale_down":{reaped_scale_down},"reaped_idle":{reaped_idle},"pruned_phantom":{pruned_phantom},"cleaned_exited":{cleaned_exited},"max":{max},"warm":{warm}}}"#
    )
}

/// Keep only the last `max_lines` lines from `content`.  Used to cap the
/// history file so it never grows unbounded.
pub fn rotate_keep_tail(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return content.to_string();
    }
    lines[lines.len() - max_lines..].join("\n") + "\n"
}

fn history_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-runner-history.jsonl")
}

fn append_history(entry: &str) {
    let p = history_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Read, rotate-if-needed, then overwrite atomically-ish (single write).
    let existing = std::fs::read_to_string(&p).unwrap_or_default();
    let new_content = if existing.is_empty() {
        format!("{entry}\n")
    } else {
        format!("{existing}{entry}\n")
    };
    let rotated = rotate_keep_tail(&new_content, HISTORY_MAX_LINES);
    let _ = std::fs::write(&p, rotated);
}

// ---------------------------------------------------------------------------
// Reconcile
// ---------------------------------------------------------------------------

/// `vox ci runner-scale` — reconcile the ephemeral pool to queued-job demand:
/// remove exited one-shot containers, reap never-assigned idle runners, prune
/// stale offline registrations, and spawn up to demand.
pub fn run_scale(apply: bool) -> Result<()> {
    let dry_run = !apply;
    let now = now_secs();

    // Acquire single-instance lock for apply runs. Dry-run never mutates state
    // so it is safe to run concurrently (useful for monitoring).
    let _lock = if apply {
        match ScaleLock::acquire(now)? {
            Some(lock) => Some(lock),
            None => {
                println!("runner-scale: another apply is in progress (lock held) — skipping");
                return Ok(());
            }
        }
    } else {
        None
    };

    let max = max_runners();
    let reap_secs = idle_reap_secs();
    let warm = warm_pool();
    let phantom_grace = phantom_grace_secs();
    let prev = read_state();
    let mut phantom_seen = read_phantom_seen();
    let rows = runner_rows().unwrap_or_default();
    let busy_map = managed_busy_map(&rows);

    // 1. Remove exited managed containers — the primary despawn path now that
    //    ephemeral runners exit after their single job. Deregister from GitHub
    //    first so the registration slot is freed before the container is removed.
    let mut dead = 0u32;
    for name in managed_containers("exited") {
        if !dry_run {
            deregister(&name);
            let _ = docker(&["rm", "-f", &name]);
        }
        dead += 1;
    }

    // 2. Classify running containers, then scale down / grace-reap idle runners.
    let running: Vec<String> = managed_containers("running");
    let mut busy_count = 0u32;
    let mut starting_count = 0u32;
    let mut idle_runners: Vec<(String, Option<i64>)> = Vec::new();

    for name in &running {
        match busy_map.get(name) {
            Some(true) => busy_count += 1,
            Some(false) => {
                let idle_since = next_idle_since(false, prev.get(name).copied(), now);
                idle_runners.push((name.clone(), idle_since));
            }
            None => starting_count += 1,
        }
    }

    let demand = query_queued_job_demand(max).unwrap_or(0);
    let desired = desired_runner_count(demand, max, warm);
    let total_keep = busy_count + idle_runners.len() as u32 + starting_count;

    let mut reaped_scale_down = 0u32;
    if total_keep > desired {
        let excess = total_keep - desired;
        let reap_budget = excess.min(idle_runners.len() as u32);
        let idle_with_since: Vec<(String, i64)> = idle_runners
            .iter()
            .map(|(name, since)| (name.clone(), since.unwrap_or(now)))
            .collect();
        let to_reap = scale_down_reap_targets(&idle_with_since, reap_budget);
        let reap_set: HashSet<String> = to_reap.into_iter().collect();
        idle_runners.retain(|(name, _)| !reap_set.contains(name));
        for name in reap_set {
            reap(&name, dry_run, "scale-down above desired");
            reaped_scale_down += 1;
        }
    }

    let mut new_state: HashMap<String, i64> = HashMap::new();
    let mut keep = busy_count + starting_count;
    let mut reaped = 0u32;
    for (name, idle_since) in &idle_runners {
        if should_reap_idle(*idle_since, now, reap_secs) {
            reap(name, dry_run, "idle > reap grace (never assigned)");
            reaped += 1;
        } else {
            if let Some(s) = idle_since {
                new_state.insert(name.clone(), *s);
            }
            keep += 1;
        }
    }

    // 3. Prune phantom offline GitHub registrations with no backing container —
    //    leftovers from crashed ephemeral runners that never self-deregistered.
    //    Unlike the old stale-offline check, phantoms are pruned regardless of
    //    fleet-busy state once the grace window has elapsed: a phantom blocks a
    //    registration slot no matter how loaded the fleet is.
    let mut containers: HashSet<String> = running.iter().cloned().collect();
    containers.extend(managed_containers("exited"));

    // Record first-seen timestamp for newly-detected offline phantoms.
    for (name, status, busy) in &rows {
        if name.starts_with(MANAGED_PREFIX)
            && status == "offline"
            && !busy
            && !containers.contains(name)
        {
            phantom_seen.entry(name.clone()).or_insert(now);
        }
    }
    // Evict entries that have a backing container again (container restarted etc.).
    phantom_seen.retain(|name, _| !containers.contains(name));

    let mut pruned = 0u32;
    let mut pruned_names: Vec<String> = Vec::new();
    for (name, _first_seen) in
        phantom_offline_registrations(&rows, &containers, &phantom_seen, now, phantom_grace)
    {
        if dry_run {
            println!("[dry-run] would prune phantom offline registration {name}");
        } else {
            eprintln!("[prune] phantom offline registration {name}");
            deregister(name);
        }
        pruned_names.push(name.to_string());
        pruned += 1;
    }
    // Remove pruned entries from phantom_seen so they don't linger.
    for name in &pruned_names {
        phantom_seen.remove(name);
    }

    if !dry_run {
        write_phantom_seen(&phantom_seen);
    }

    // 4. Scale up toward queued-job demand (plus warm pool).
    let spawn = spawn_count(desired, keep);
    let t = tag();
    for i in 0..spawn {
        spawn_one(i, &t, dry_run)?;
    }

    if !dry_run {
        write_state(&new_state);
    }

    // Append an immutable decision record before printing (both apply + dry-run).
    append_history(&scale_event_json(
        now,
        dry_run,
        demand,
        keep,
        desired,
        spawn,
        reaped_scale_down,
        reaped,
        pruned,
        dead,
        max,
        warm,
    ));

    println!(
        "runner-scale: dry_run={dry_run} queued_jobs={demand} keep={keep} desired={desired} \
         spawned={spawn} reaped_scale_down={reaped_scale_down} reaped_idle={reaped} \
         pruned_phantom={pruned} cleaned_exited={dead} \
         (max={max}, warm={warm}, idle_reap={reap_secs}s, ephemeral)"
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
        assert_eq!(desired_runner_count(0, 4, 0), 0);
        assert_eq!(desired_runner_count(2, 4, 0), 2);
        assert_eq!(desired_runner_count(99, 4, 0), 4);
    }

    #[test]
    fn warm_pool_floors_desired_but_respects_max() {
        assert_eq!(desired_runner_count(0, 4, 1), 1); // idle queue keeps 1 warm
        assert_eq!(desired_runner_count(3, 4, 1), 3); // demand above warm pool wins
        assert_eq!(desired_runner_count(0, 4, 9), 4); // warm pool capped at max
        assert_eq!(desired_runner_count(0, 4, 0), 0); // pure scale-to-zero default
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
        assert!(!should_reap_idle(None, 9999, 300)); // active (no idle stamp)
        assert!(!should_reap_idle(Some(1000), 1000 + 299, 300)); // not yet
        assert!(should_reap_idle(Some(1000), 1000 + 300, 300)); // at timeout
        assert!(should_reap_idle(Some(1000), 1000 + 5000, 300)); // well past
    }

    #[test]
    fn default_reap_is_short_grace_window() {
        // Ephemeral runners despawn by exiting after their job; the idle reap is
        // only a grace window for never-assigned runners. Minutes, not half-hours.
        assert_eq!(DEFAULT_IDLE_REAP_SECS, 300);
        let now = 100_000;
        let idle_4m = next_idle_since(false, Some(now - 240), now);
        assert!(!should_reap_idle(idle_4m, now, DEFAULT_IDLE_REAP_SECS));
        let idle_6m = next_idle_since(false, Some(now - 360), now);
        assert!(should_reap_idle(idle_6m, now, DEFAULT_IDLE_REAP_SECS));
    }

    #[test]
    fn queued_job_demand_counts_only_jobs_the_pool_can_serve() {
        let lines = "self-hosted,linux,x64\n\
                     self-hosted,linux,x64,docker\n\
                     self-hosted,linux,x64,browser\n\
                     self-hosted,linux,x64,gpu\n\
                     ubuntu-latest\n\
                     \n\
                     self-hosted,linux,x64";
        // gpu + ubuntu-latest are not serveable by this pool; blank line ignored.
        assert_eq!(count_matching_queued_jobs(lines, RUNNER_LABELS), 4);
        assert_eq!(count_matching_queued_jobs("", RUNNER_LABELS), 0);
    }

    #[test]
    fn queued_job_demand_requires_every_label() {
        // A job needing a label the pool lacks must not count.
        assert_eq!(
            count_matching_queued_jobs("self-hosted,linux,x64,gpu", RUNNER_LABELS),
            0
        );
        // Whitespace around labels is tolerated.
        assert_eq!(
            count_matching_queued_jobs(" self-hosted , linux , x64 ", RUNNER_LABELS),
            1
        );
    }

    #[test]
    fn phantom_pruned_after_grace_regardless_of_busy() {
        let rows: Vec<RunnerRow> = vec![
            // offline, no container, past grace → prune
            ("vox-runner-auto-aaa-0".into(), "offline".into(), false),
            // offline but container still exists → not a phantom
            ("vox-runner-auto-bbb-0".into(), "offline".into(), false),
            // online → never pruned
            ("vox-runner-auto-ccc-0".into(), "online".into(), false),
            // busy → never pruned even if reported offline mid-transition
            ("vox-runner-auto-ddd-0".into(), "offline".into(), true),
            // unmanaged names are never touched
            ("vox-runner-1".into(), "offline".into(), false),
        ];
        let containers: HashSet<String> = ["vox-runner-auto-bbb-0".to_string()].into();
        let now = 1_000_000i64;
        let grace = 120i64;
        // aaa-0 was first seen 200s ago (past grace)
        let mut phantom_seen: HashMap<String, i64> = HashMap::new();
        phantom_seen.insert("vox-runner-auto-aaa-0".to_string(), now - 200);
        let to_prune = phantom_offline_registrations(&rows, &containers, &phantom_seen, now, grace);
        let names: Vec<&str> = to_prune.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, vec!["vox-runner-auto-aaa-0"]);
    }

    #[test]
    fn phantom_held_one_tick_within_grace() {
        let rows: Vec<RunnerRow> = vec![("vox-runner-auto-aaa-0".into(), "offline".into(), false)];
        let containers: HashSet<String> = HashSet::new();
        let now = 1_000_000i64;
        let grace = 120i64;
        // aaa-0 was first seen only 60s ago (within grace)
        let mut phantom_seen: HashMap<String, i64> = HashMap::new();
        phantom_seen.insert("vox-runner-auto-aaa-0".to_string(), now - 60);
        let to_prune = phantom_offline_registrations(&rows, &containers, &phantom_seen, now, grace);
        assert!(to_prune.is_empty(), "within grace: should not prune yet");
    }

    #[test]
    fn idle_lifecycle_keeps_then_reaps() {
        let now = 100_000;
        let idle_2m = next_idle_since(false, Some(now - 120), now);
        assert!(!should_reap_idle(idle_2m, now, DEFAULT_IDLE_REAP_SECS));
        let idle_10m = next_idle_since(false, Some(now - 600), now);
        assert!(should_reap_idle(idle_10m, now, DEFAULT_IDLE_REAP_SECS));
    }

    #[test]
    fn scale_down_reaps_newest_idle_first() {
        let idle = vec![
            ("vox-runner-auto-a-0".into(), 100),
            ("vox-runner-auto-a-1".into(), 200),
            ("vox-runner-auto-a-2".into(), 300),
            ("vox-runner-auto-a-3".into(), 400),
        ];
        let reaped = scale_down_reap_targets(&idle, 3);
        assert_eq!(
            reaped,
            vec![
                "vox-runner-auto-a-3",
                "vox-runner-auto-a-2",
                "vox-runner-auto-a-1",
            ]
        );
    }

    #[test]
    fn scale_down_reap_zero_when_no_excess() {
        let idle = vec![("vox-runner-auto-a-0".into(), 100)];
        assert!(scale_down_reap_targets(&idle, 0).is_empty());
    }

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
    }

    #[test]
    fn shared_cache_env_empty_when_unreachable() {
        assert!(shared_cache_env(false).is_empty());
    }

    #[test]
    fn shared_cache_env_points_runners_at_host_minio_without_credentials() {
        let env = shared_cache_env(true);
        let get = |k: &str| {
            env.iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(get("SCCACHE_BUCKET"), "vox-sccache");
        // Containers must reach the host's MinIO, not their own loopback.
        assert!(get("SCCACHE_ENDPOINT").contains("host.docker.internal"));
        assert_eq!(get("SCCACHE_S3_NO_CREDENTIALS"), "true");
        // sccache cannot cache incremental compiles.
        assert_eq!(get("CARGO_INCREMENTAL"), "0");
        // Anonymous LAN bucket: no AWS credentials may be injected.
        assert!(!env.iter().any(|(k, _)| k.starts_with("AWS_")));
    }

    #[test]
    fn scale_lock_age_steal_policy() {
        let base = 1_000_000i64;
        // Fresh lock (written 30s ago) — must not be stolen.
        assert!(!scale_lock_is_stale(base - 30, base));
        // Lock written exactly at the stale boundary — stealable.
        assert!(scale_lock_is_stale(base - LOCK_STALE_SECS, base));
        // Very old lock — definitely stealable.
        assert!(scale_lock_is_stale(base - 3600, base));
    }

    #[test]
    fn scale_event_json_has_all_decision_fields() {
        let s = scale_event_json(
            1_000_000, // ts
            false,     // dry_run
            3,         // queued_jobs
            2,         // keep
            3,         // desired
            1,         // spawned
            0,         // reaped_scale_down
            0,         // reaped_idle
            0,         // pruned_phantom
            1,         // cleaned_exited
            6,         // max
            1,         // warm
        );
        // Every key must be present.
        for key in &[
            "\"ts\"",
            "\"dry_run\"",
            "\"queued_jobs\"",
            "\"keep\"",
            "\"desired\"",
            "\"spawned\"",
            "\"reaped_scale_down\"",
            "\"reaped_idle\"",
            "\"pruned_phantom\"",
            "\"cleaned_exited\"",
            "\"max\"",
            "\"warm\"",
        ] {
            assert!(s.contains(key), "missing key {key} in: {s}");
        }
        // Must be valid JSON.
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid JSON");
        assert_eq!(v["ts"], 1_000_000i64);
        assert_eq!(v["dry_run"], false);
        assert_eq!(v["spawned"], 1);
    }

    #[test]
    fn history_rotates_when_over_cap() {
        // Build a content block with 5 lines and cap at 3.
        let content = "line1\nline2\nline3\nline4\nline5\n";
        let rotated = rotate_keep_tail(content, 3);
        let lines: Vec<&str> = rotated.lines().collect();
        assert_eq!(lines.len(), 3, "should keep exactly 3 lines");
        assert_eq!(lines[0], "line3");
        assert_eq!(lines[2], "line5");

        // Under-cap: content unchanged.
        let small = "a\nb\n";
        assert_eq!(rotate_keep_tail(small, 10), small);
    }
}
