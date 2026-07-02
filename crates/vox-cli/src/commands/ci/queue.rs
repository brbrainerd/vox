//! `vox ci queue` — run-centric CI queue snapshot, classification, clearing,
//! and the async failure signal.
//!
//! SSOT for the local-first CI contract
//! (docs/superpowers/specs/2026-07-02-local-first-ci-queue-design.md): agents
//! verify with local gates and never watch remote checks; this command is the
//! sanctioned way to read (`--json`/`--brief`) or clear (`--clear`) the queue,
//! and its snapshot carries recent run failures back to future sessions.
//! `--hook-guard` is the PreToolUse enforcement mode. Run-level only — the
//! autoscaler's job-label demand counting stays in `runner_scale.rs`.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use super::constants::REPO_SLUG;
use super::runner_scale::{gh_json, now_secs};

pub const DEFAULT_TTL_MINS: i64 = 45;
/// Blast-radius + POST-burst bound per sweep; remainder clears on later ticks.
pub const MAX_CANCELS_PER_SWEEP: usize = 50;
/// `--from-snapshot` refuses older snapshots (steady state is ~2 min via the tick).
const SNAPSHOT_STALE_SECS: i64 = 600;
const FAILURE_WINDOW_SECS: i64 = 86_400;
const FAILURE_CAP: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunClass {
    Active,
    Superseded,
    Stale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueRun {
    pub id: u64,
    /// Workflow file path (".github/workflows/x.yml") — display `name` can
    /// contain tabs and collide; `.path` cannot.
    pub workflow: String,
    /// "null" when the API reports no head branch — never supersedable.
    pub branch: String,
    /// head_repository.full_name — fork disambiguation in the supersede key.
    pub repo: String,
    pub event: String,
    /// queued | in_progress | pending | waiting
    pub status: String,
    pub run_attempt: u32,
    /// run_started_at when present (re-runs reset it), else created_at.
    pub started_epoch: i64,
    pub age_secs: i64,
    pub class: RunClass,
    pub exempt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedRun {
    pub id: u64,
    pub workflow: String,
    pub branch: String,
    /// failure | timed_out | startup_failure. `cancelled` is EXCLUDED so the
    /// auto-clear's own cancellations never echo back as failures.
    pub conclusion: String,
    pub head_sha: String,
    pub completed_epoch: i64,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueSnapshot {
    pub generated_at: i64,
    pub degraded: bool,
    /// queued + pending (both "not yet running").
    pub queued: u32,
    pub in_progress: u32,
    pub superseded: u32,
    pub stale: u32,
    pub fleet_alive: u32,
    pub fleet_max: u32,
    /// THE machine-readable signal: always present, says what to do next.
    pub advice: String,
    /// Async failure signal: last 24h, newest-first, cap 20, main included.
    pub failures: Vec<FailedRun>,
    /// Ids cancelled by the previous sweep — force-cancel escalation state.
    pub cancelled_last_sweep: Vec<u64>,
    pub runs: Vec<QueueRun>,
}

/// Release workflows trigger on `tags: v*`; their runs report the tag as
/// head_branch with event=push (live-verified: release-binaries → "v0.6.0").
fn is_tag_like(branch: &str) -> bool {
    let mut c = branch.chars();
    c.next() == Some('v') && c.next().is_some_and(|c| c.is_ascii_digit())
}

/// Fail-open exemption: a run is cancellable only when EVERY arm below passes.
/// Unknown events, re-runs, tag pushes, approval-gated runs are all exempt.
pub fn is_exempt(branch: &str, event: &str, run_attempt: u32, status: &str) -> bool {
    let cancellable_event = matches!(event, "push" | "pull_request");
    !cancellable_event
        || branch == "main"
        || branch == "null"
        || (event == "push" && is_tag_like(branch))
        || run_attempt > 1
        || status == "waiting"
}

/// One tab-separated line from JQ_RUN_LINE:
/// id \t path \t branch \t repo \t event \t started_epoch \t status \t run_attempt
pub fn parse_run_line(line: &str, now: i64) -> Option<QueueRun> {
    let mut p = line.split('\t');
    let id = p.next()?.trim().parse().ok()?;
    let workflow = p.next()?.trim().to_string();
    let branch = p.next()?.trim().to_string();
    let repo = p.next()?.trim().to_string();
    let event = p.next()?.trim().to_string();
    let started_epoch: i64 = p.next()?.trim().parse().ok()?;
    let status = p.next()?.trim().to_string();
    let run_attempt: u32 = p.next()?.trim().parse().ok()?;
    let exempt = is_exempt(&branch, &event, run_attempt, &status);
    Some(QueueRun {
        id,
        workflow,
        branch,
        repo,
        event,
        status,
        run_attempt,
        started_epoch,
        age_secs: now.saturating_sub(started_epoch),
        class: RunClass::Active,
        exempt,
    })
}

/// Superseded: strictly newer non-exempt run with the same
/// (workflow, repo, branch, event) key. Stale: queued/pending past TTL —
/// only while the fleet is alive (`stale_enabled`); a deep queue with zero
/// runners is an outage, and cancelling it would kill the async safety net
/// AND reset the health watchdog's queue_age signal.
///
/// O(n²) (nested `iter().any()`), accepted given the fetch cap of ~2000 runs
/// (4 statuses × 5 pages × 100, see MAX_PAGES) — revisit with a HashMap-grouped
/// pass if that cap ever grows materially.
pub fn classify_runs(runs: &mut [QueueRun], ttl_secs: i64, stale_enabled: bool) {
    for i in 0..runs.len() {
        if runs[i].exempt {
            runs[i].class = RunClass::Active;
            continue;
        }
        let newer = runs.iter().any(|o| {
            !o.exempt
                && o.id != runs[i].id
                && o.workflow == runs[i].workflow
                && o.repo == runs[i].repo
                && o.branch == runs[i].branch
                && o.event == runs[i].event
                && o.started_epoch > runs[i].started_epoch
        });
        runs[i].class = if newer {
            RunClass::Superseded
        } else if stale_enabled
            && matches!(runs[i].status.as_str(), "queued" | "pending")
            && runs[i].age_secs > ttl_secs
        {
            RunClass::Stale
        } else {
            RunClass::Active
        };
    }
}

/// Global advice (no branch context — the snapshot is written by the tick).
/// Branch-failure advice is layered at render time via `failure_advice`.
pub fn advice_for(
    active_queued: u32,
    capacity: u32,
    superseded: u32,
    stale: u32,
    fleet_alive: u32,
    degraded: bool,
) -> String {
    if degraded {
        return "degraded: gh unreachable or partial data; do not retry-loop — \
                proceed on local gates (`vox ci pre-push --complete`) and try `vox ci queue` later"
            .to_string();
    }
    if superseded + stale > 0 {
        return format!(
            "queued {active_queued} vs capacity {capacity}: run 'vox ci queue --clear' \
             (would cancel {superseded} superseded + {stale} stale)"
        );
    }
    if active_queued > capacity && fleet_alive == 0 {
        return format!(
            "queue backlog: {active_queued} active > capacity {capacity} with fleet at 0 — \
             outage, not backlog; stale sweep disabled; check 'vox ci runner-status'"
        );
    }
    if active_queued > capacity {
        return format!(
            "queue backlog: {active_queued} active queued > capacity {capacity}; \
             nothing clearable — this is real demand, do not add speculative pushes"
        );
    }
    format!("queue healthy: {active_queued} active queued ≤ capacity {capacity}")
}

/// The failure half of the signal — used when the CURRENT branch has a red run.
pub fn failure_advice(f: &FailedRun) -> String {
    format!(
        "CI FAILED for this branch (run {}): read {} or 'gh run view {} --log-failed', \
         fix locally, re-run local gates — do not push blind retries",
        f.id, f.url, f.id
    )
}

/// jq projections. `fromdateiso8601` is proven in this repo (ci-timings.yml:44
/// uses it inside `gh api --jq` in production). `.path` not `.name` (tab-safe).
const JQ_RUN_LINE: &str = ".workflow_runs[]|\"\\(.id)\\t\\(.path)\\t\\(.head_branch)\\t\\(.head_repository.full_name)\\t\\(.event)\\t\\((.run_started_at // .created_at)|fromdateiso8601)\\t\\(.status)\\t\\(.run_attempt)\"";
const JQ_FAIL_LINE: &str = ".workflow_runs[]|select(.conclusion==\"failure\" or .conclusion==\"timed_out\" or .conclusion==\"startup_failure\")|\"\\(.id)\\t\\(.path)\\t\\(.head_branch)\\t\\(.conclusion)\\t\\(.head_sha)\\t\\(.updated_at|fromdateiso8601)\\t\\(.html_url)\"";

/// The four live-ish statuses. `pending` = concurrency-group blocked (real in
/// this repo, live-probed); `waiting` = deployment approval (fetched for
/// visibility, exempt from cancellation via is_exempt).
const FETCH_STATUSES: &[&str] = &["queued", "in_progress", "pending", "waiting"];
/// Newest-first API + manual page loop (gh --paginate cannot be capped):
/// 5 pages × 100 bounds the flood case without going blind to the stale tail.
const MAX_PAGES: u32 = 5;

fn fetch_status_runs(status: &str, now: i64) -> Result<Vec<QueueRun>> {
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        let raw = gh_json(&[
            "api",
            &format!("repos/{REPO_SLUG}/actions/runs?status={status}&per_page=100&page={page}"),
            "--jq",
            JQ_RUN_LINE,
        ])?;
        let mut n = 0u32;
        for line in raw.lines().filter(|l| !l.trim().is_empty()) {
            if let Some(r) = parse_run_line(line, now) {
                out.push(r);
            }
            n += 1;
        }
        if n < 100 {
            break;
        }
    }
    Ok(out)
}

pub fn fetch_all_runs(now: i64) -> Result<Vec<QueueRun>> {
    let mut runs = Vec::new();
    for status in FETCH_STATUSES {
        runs.extend(fetch_status_runs(status, now)?);
    }
    Ok(runs)
}

pub fn parse_failed_line(line: &str) -> Option<FailedRun> {
    let mut p = line.split('\t');
    Some(FailedRun {
        id: p.next()?.trim().parse().ok()?,
        workflow: p.next()?.trim().to_string(),
        branch: p.next()?.trim().to_string(),
        conclusion: p.next()?.trim().to_string(),
        head_sha: p.next()?.trim().to_string(),
        completed_epoch: p.next()?.trim().parse().ok()?,
        url: p.next()?.trim().to_string(),
    })
}

pub fn filter_failures(mut failures: Vec<FailedRun>, now: i64) -> Vec<FailedRun> {
    failures.retain(|f| now.saturating_sub(f.completed_epoch) <= FAILURE_WINDOW_SECS);
    failures.sort_by_key(|f| std::cmp::Reverse(f.completed_epoch));
    failures.truncate(FAILURE_CAP);
    failures
}

pub fn fetch_recent_failures(now: i64) -> Result<Vec<FailedRun>> {
    let raw = gh_json(&[
        "api",
        &format!("repos/{REPO_SLUG}/actions/runs?status=completed&per_page=50"),
        "--jq",
        JQ_FAIL_LINE,
    ])?;
    Ok(filter_failures(
        raw.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(parse_failed_line)
            .collect(),
        now,
    ))
}

#[allow(clippy::too_many_arguments)]
pub fn build_snapshot(
    runs: Vec<QueueRun>,
    failures: Vec<FailedRun>,
    fleet_alive: u32,
    fleet_max: u32,
    degraded: bool,
    now: i64,
    cancelled_last_sweep: Vec<u64>,
) -> QueueSnapshot {
    let queued = runs
        .iter()
        .filter(|r| matches!(r.status.as_str(), "queued" | "pending"))
        .count() as u32;
    let in_progress = runs.iter().filter(|r| r.status == "in_progress").count() as u32;
    let superseded = runs
        .iter()
        .filter(|r| r.class == RunClass::Superseded)
        .count() as u32;
    let stale = runs.iter().filter(|r| r.class == RunClass::Stale).count() as u32;
    let active_queued = runs
        .iter()
        .filter(|r| matches!(r.status.as_str(), "queued" | "pending") && r.class == RunClass::Active)
        .count() as u32;
    let advice = advice_for(active_queued, fleet_max, superseded, stale, fleet_alive, degraded);
    QueueSnapshot {
        generated_at: now,
        degraded,
        queued,
        in_progress,
        superseded,
        stale,
        fleet_alive,
        fleet_max,
        advice,
        failures,
        cancelled_last_sweep,
        runs,
    }
}

fn snapshot_path() -> PathBuf {
    crate::fs_utils::user_home_dir()
        .join(".vox")
        .join("ci-queue-snapshot.json")
}

/// Atomic write (temp + rename): parallel agent sessions and the autoscaler
/// tick race on this file; a torn read must be impossible.
pub fn write_snapshot(snap: &QueueSnapshot) -> Result<()> {
    let p = snapshot_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(snap)?)
        .with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, &p).with_context(|| format!("rename into {}", p.display()))
}

pub fn read_snapshot() -> Option<QueueSnapshot> {
    std::fs::read_to_string(snapshot_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

pub fn snapshot_is_stale(generated_at: i64, now: i64) -> bool {
    now.saturating_sub(generated_at) > SNAPSHOT_STALE_SECS
}

fn current_branch() -> Option<String> {
    let out = super::runner_scale::quiet_command("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn failed_line(prefix: &str, f: &FailedRun, now: i64) -> String {
    format!(
        "FAILED on {prefix}: {} #{} ({}, {}m ago) -> {}",
        f.workflow,
        f.id,
        f.conclusion,
        now.saturating_sub(f.completed_epoch) / 60,
        f.url
    )
}

/// ≤7 lines; SessionStart hook stdout, injected into agent context.
/// Branch failure overrides the displayed advice (the failure IS the signal).
pub fn render_brief(snap: &QueueSnapshot, branch: Option<&str>, now: i64) -> String {
    let mut lines = vec![
        "CI queue (local-first: local gates = verdict for what they cover; never watch remote checks):".to_string(),
        format!(
            "queued {} / in-progress {} (superseded {}, stale {}); fleet {}/{}",
            snap.queued, snap.in_progress, snap.superseded, snap.stale,
            snap.fleet_alive, snap.fleet_max
        ),
    ];
    let branch_fail = branch.and_then(|b| snap.failures.iter().find(|f| f.branch == b));
    if let Some(f) = branch_fail {
        lines.push(failed_line(&f.branch.clone(), f, now));
    }
    if let Some(f) = snap.failures.iter().find(|f| f.branch == "main") {
        lines.push(failed_line("main", f, now));
    }
    lines.push(format!(
        "advice: {}",
        branch_fail
            .map(failure_advice)
            .unwrap_or_else(|| snap.advice.clone())
    ));
    lines.push("commands: `vox ci queue --json` | `vox ci queue --clear`".to_string());
    lines.join("\n")
}

fn render_table(snap: &QueueSnapshot, now: i64) -> String {
    let mut out = String::from(
        "RUN_ID      AGE_MIN  CLASS       STATUS       EVENT             BRANCH                    WORKFLOW\n",
    );
    for r in &snap.runs {
        let class = match (r.exempt, r.class) {
            (true, _) => "exempt",
            (_, RunClass::Active) => "active",
            (_, RunClass::Superseded) => "superseded",
            (_, RunClass::Stale) => "stale",
        };
        out.push_str(&format!(
            "{:<11} {:>7} {:<11} {:<12} {:<17} {:<25} {}\n",
            r.id, r.age_secs / 60, class, r.status, r.event, r.branch, r.workflow
        ));
    }
    if !snap.failures.is_empty() {
        out.push_str("\nFAILED (24h):\n");
        for f in &snap.failures {
            out.push_str(&format!("  {}\n", failed_line(&f.branch.clone(), f, now)));
        }
    }
    out.push_str(&format!("\nadvice: {}\n", snap.advice));
    out
}

/// Managed fleet counts (alive containers, configured max). Best-effort —
/// (0, max) when docker is down, which conservatively disables stale-cancel.
fn fleet_counts() -> (u32, u32) {
    let alive = super::runner_scale::managed_running_count().unwrap_or(0);
    (alive, super::runner_scale::max_runners())
}

pub struct QueueArgs {
    pub json: bool,
    pub brief: bool,
    pub from_snapshot: bool,
    pub clear: bool,
    pub dry_run: bool,
    pub ttl_mins: Option<i64>,
    pub hook_guard: bool,
}

/// Live snapshot: fetch runs + failures, classify (stale gated on fleet
/// health), persist atomically.
pub fn live_snapshot(ttl_mins: i64, now: i64, cancelled_last_sweep: Vec<u64>) -> Result<QueueSnapshot> {
    let mut runs = fetch_all_runs(now)?;
    let (alive, max) = fleet_counts();
    classify_runs(&mut runs, ttl_mins * 60, alive > 0);
    let failures = fetch_recent_failures(now).unwrap_or_default();
    let snap = build_snapshot(runs, failures, alive, max, false, now, cancelled_last_sweep);
    write_snapshot(&snap)?;
    Ok(snap)
}

pub fn run(args: QueueArgs) -> Result<()> {
    if args.hook_guard {
        return hook_guard_main();
    }
    if args.clear && args.from_snapshot {
        return Err(anyhow!(
            "--clear requires live data; refusing to cancel from a snapshot up to 10 min old"
        ));
    }
    let now = now_secs();
    let ttl = args.ttl_mins.unwrap_or(DEFAULT_TTL_MINS);

    let snap = if args.from_snapshot {
        match read_snapshot() {
            Some(s) if !snapshot_is_stale(s.generated_at, now) => s,
            _ => {
                println!("queue snapshot unavailable/stale — run `vox ci queue` for live state");
                return Ok(());
            }
        }
    } else {
        // Preserve the prior sweep's cancelled ids — a read-only invocation must
        // never clobber the force-cancel escalation state auto_clear_and_snapshot
        // depends on (that would silently defeat two-tick escalation for any run
        // still shielded by always()/post steps).
        let prev_cancelled = read_snapshot().map(|s| s.cancelled_last_sweep).unwrap_or_default();
        match live_snapshot(ttl, now, prev_cancelled) {
            Ok(s) => s,
            Err(e) if args.clear => return Err(e).context("--clear needs live gh data"),
            Err(e) => {
                eprintln!("queue: gh query failed: {e:#}");
                build_snapshot(Vec::new(), Vec::new(), 0, 0, true, now, Vec::new())
            }
        }
    };

    if args.clear {
        return clear_runs(&snap, args.dry_run);
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&snap)?);
    } else if args.brief {
        println!("{}", render_brief(&snap, current_branch().as_deref(), now));
    } else {
        println!("{}", render_table(&snap, now));
    }
    Ok(())
}

/// Runs `--clear` cancels: non-exempt, non-Active, in a cancellable status,
/// capped per sweep (blast-radius + POST-burst bound).
pub fn clear_plan(snap: &QueueSnapshot) -> Vec<&QueueRun> {
    let mut v: Vec<&QueueRun> = snap
        .runs
        .iter()
        .filter(|r| {
            !r.exempt
                && r.class != RunClass::Active
                && matches!(r.status.as_str(), "queued" | "in_progress" | "pending")
        })
        .collect();
    v.truncate(MAX_CANCELS_PER_SWEEP);
    v
}

fn cancel_run(id: u64, force: bool) -> Result<String> {
    let tail = if force { "force-cancel" } else { "cancel" };
    gh_json(&[
        "api",
        "-X",
        "POST",
        &format!("repos/{REPO_SLUG}/actions/runs/{id}/{tail}"),
    ])
}

/// Best-effort sweep: a 409 means the run completed meanwhile — the race
/// resolved itself; log and continue, never abort.
fn clear_runs(snap: &QueueSnapshot, dry_run: bool) -> Result<()> {
    let plan = clear_plan(snap);
    if plan.is_empty() {
        println!("queue clear: nothing cancellable ({})", snap.advice);
        return Ok(());
    }
    let mut cancelled_ids = Vec::new();
    let mut failed = 0u32;
    for r in &plan {
        let tag = format!("{} ({} / {} / {:?})", r.id, r.workflow, r.branch, r.class);
        if dry_run {
            println!("would cancel {tag}");
            continue;
        }
        match cancel_run(r.id, false) {
            Ok(_) => {
                println!("cancelled {tag}");
                cancelled_ids.push(r.id);
            }
            Err(e) => {
                eprintln!("cancel {tag} failed (continuing): {e:#}");
                failed += 1;
            }
        }
    }
    if !dry_run {
        println!(
            "queue clear: cancelled {}, failed {failed}, of {} planned",
            cancelled_ids.len(),
            plan.len()
        );
        let now = now_secs();
        if let Ok(s) = live_snapshot(DEFAULT_TTL_MINS, now, cancelled_ids) {
            println!("post-clear: {}", s.advice);
        }
    }
    Ok(())
}

/// Normalized substring match on the command an agent is about to run.
/// Normalization (lowercase + whitespace collapse) kills the `gh  pr  checks`
/// evasion class. `gh run view --watch` is deliberately NOT an arm: the flag
/// does not exist (`-w` is `--web`), and matching it only produced false
/// positives on compound commands. Known collateral: a banned phrase inside a
/// quoted string still blocks — acceptable; the deny message names the
/// sanctioned alternatives.
pub fn hook_guard_matches(cmd: &str) -> bool {
    let c: String = cmd.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ");
    let has = |s: &str| c.contains(s);
    has("gh pr checks")
        || has("gh run watch")
        || (has("gh api") && (has("check-runs") || has("check_runs")))
        || has("ci watch-run")
        // Hand-rolled watch loop from allowed primitives.
        || ((has("while ") || has("until ") || has("for "))
            && has("sleep")
            && (has("gh pr") || has("gh run") || has("gh api")))
        // Alias evasion.
        || (has("gh alias set") && (has("pr checks") || has("run watch")))
}

const HOOK_GUARD_DENY: &str = "Local-first CI: remote check-watching is disabled.\n\
- Verdict: run local gates (`vox ci pre-push --complete`); green = done, push and move on.\n\
- Queue + failures: `vox ci queue --json` (the `advice` field tells you what to do).\n\
- Read one failure's logs: `gh run list --branch <b>` then `gh run view <id> --log-failed` (allowed).\n\
- Clear backlog: `vox ci queue --clear`.";

/// PreToolUse mode: read the Claude Code hook JSON from stdin, extract
/// `tool_input.command` (Bash and PowerShell tools both use `command`), exit 2
/// (block; stderr fed to the model) on a banned pattern. Everything else —
/// including unparseable input — exits 0: fail-open on infrastructure,
/// fail-closed only on the banned patterns. Purely local, no network.
///
/// `VOX_HOOK_GUARD_DISABLE=1` in the HOOK PROCESS env (session-level export,
/// not settable from inside a guarded command string) short-circuits to allow
/// — for maintainer sessions working on the guard itself.
fn hook_guard_main() -> Result<()> {
    if std::env::var("VOX_HOOK_GUARD_DISABLE").as_deref() == Ok("1") {
        return Ok(());
    }
    let mut input = String::new();
    use std::io::Read;
    if std::io::stdin().read_to_string(&mut input).is_err() {
        return Ok(());
    }
    let cmd = serde_json::from_str::<serde_json::Value>(&input)
        .ok()
        .and_then(|v| {
            v.get("tool_input")
                .and_then(|t| t.get("command"))
                .and_then(|c| c.as_str())
                .map(str::to_string)
        });
    if let Some(cmd) = cmd {
        if hook_guard_matches(&cmd) {
            eprintln!("{HOOK_GUARD_DENY}");
            std::process::exit(2);
        }
    }
    Ok(())
}

/// Autoscaler-tick entry: clear cancellable runs (apply mode), escalate to
/// force-cancel any run still in_progress that the PREVIOUS sweep cancelled
/// (shielded by always()/post steps — same two-tick pattern as
/// zombies_for_force_cancel), then persist the snapshot. Returns ACTUAL
/// (cleared_superseded, cleared_stale) — 0/0 on dry-run, which logs the
/// clearable counts to stdout instead so the ledger never claims un-done work.
pub fn auto_clear_and_snapshot(dry_run: bool, now: i64) -> Result<(u32, u32)> {
    let prev_cancelled = read_snapshot()
        .map(|s| s.cancelled_last_sweep)
        .unwrap_or_default();
    let mut runs = fetch_all_runs(now)?;
    let (alive, max) = fleet_counts();
    classify_runs(&mut runs, DEFAULT_TTL_MINS * 60, alive > 0);
    let failures = fetch_recent_failures(now).unwrap_or_default();
    let snap = build_snapshot(runs, failures, alive, max, false, now, Vec::new());

    let plan = clear_plan(&snap);
    let mut sup = 0u32;
    let mut stale = 0u32;
    let mut cancelled_ids = Vec::new();

    if dry_run {
        if !plan.is_empty() {
            println!(
                "runner-scale (dry-run): {} clearable runs (not cancelled)",
                plan.len()
            );
        }
    } else {
        for r in &plan {
            // Escalate if the previous sweep already cancelled this id.
            let force = r.status == "in_progress" && prev_cancelled.contains(&r.id);
            if cancel_run(r.id, force).is_err() {
                continue; // 409: completed meanwhile — next tick self-corrects
            }
            cancelled_ids.push(r.id);
            match r.class {
                RunClass::Superseded => sup += 1,
                RunClass::Stale => stale += 1,
                RunClass::Active => {}
            }
        }
    }
    let final_snap = QueueSnapshot {
        cancelled_last_sweep: cancelled_ids,
        ..snap
    };
    write_snapshot(&final_snap)?;
    Ok((sup, stale))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u64, wf: &str, br: &str, repo: &str, ev: &str, started: i64, st: &str, attempt: u32) -> String {
        format!("{id}\t{wf}\t{br}\t{repo}\t{ev}\t{started}\t{st}\t{attempt}")
    }

    fn run(id: u64, br: &str, ev: &str, st: &str, started: i64, now: i64) -> QueueRun {
        parse_run_line(&line(id, "ci.yml", br, "vox-foundation/vox", ev, started, st, 1), now).unwrap()
    }

    fn failed(id: u64, br: &str, completed: i64) -> FailedRun {
        parse_failed_line(&format!(
            "{id}\tci.yml\t{br}\tfailure\tabc123\t{completed}\thttps://g/{id}"
        ))
        .unwrap()
    }

    #[test]
    fn parse_run_line_roundtrip() {
        let r = run(42, "feat/x", "push", "queued", 1000, 1600);
        assert_eq!((r.id, r.age_secs, r.run_attempt), (42, 600, 1));
        assert_eq!(r.repo, "vox-foundation/vox");
        assert!(!r.exempt);
        assert!(parse_run_line("garbage", 0).is_none());
        assert!(parse_run_line("1\tci.yml\tonly-three", 0).is_none());
    }

    #[test]
    fn exemption_event_allowlist_fails_open() {
        // Only push/pull_request are cancellable; unknown events are exempt.
        for ev in [
            "merge_group",
            "schedule",
            "workflow_dispatch",
            "workflow_run",
            "dynamic",
            "some_future_event",
        ] {
            assert!(is_exempt("feat/x", ev, 1, "queued"), "{ev} must be exempt");
        }
        assert!(!is_exempt("feat/x", "push", 1, "queued"));
        assert!(!is_exempt("feat/x", "pull_request", 1, "queued"));
    }

    #[test]
    fn exemption_branch_tag_attempt_waiting() {
        assert!(is_exempt("main", "push", 1, "queued"));
        assert!(is_exempt("null", "push", 1, "queued")); // API null head_branch
        assert!(is_exempt("v0.6.0", "push", 1, "queued")); // tag push (release-binaries, live-verified)
        assert!(!is_exempt("very-cool-branch", "push", 1, "queued")); // 'v' prefix alone is not a tag
        assert!(!is_exempt("v-experiment", "push", 1, "queued"));
        assert!(is_exempt("feat/x", "push", 2, "queued")); // re-run = explicit human request
        assert!(is_exempt("feat/x", "push", 1, "waiting")); // deployment approval gate
    }

    #[test]
    fn superseded_key_includes_repo_and_event() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now),
            run(2, "feat/x", "push", "queued", 3000, now), // same key, newer -> 1 superseded
            // push/PR siblings for the same commit (mobile-eas-build shape): must NOT cancel each other
            run(3, "feat/x", "pull_request", "queued", 3001, now),
            // fork collision: same branch name, different repo -> independent
            parse_run_line(
                &line(4, "ci.yml", "patch-1", "forkA/vox", "pull_request", 1000, "queued", 1),
                now,
            )
            .unwrap(),
            parse_run_line(
                &line(5, "ci.yml", "patch-1", "forkB/vox", "pull_request", 3000, "queued", 1),
                now,
            )
            .unwrap(),
        ];
        classify_runs(&mut runs, 3600 * 24, true);
        assert_eq!(runs[0].class, RunClass::Superseded);
        assert_eq!(runs[1].class, RunClass::Active);
        assert_eq!(runs[2].class, RunClass::Active, "event is in the key");
        assert_eq!(runs[3].class, RunClass::Active, "repo is in the key");
        assert_eq!(runs[4].class, RunClass::Active);
    }

    #[test]
    fn superseded_ties_and_exempt_never() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 2000, now),
            run(2, "feat/x", "push", "queued", 2000, now), // equal started: keep both
            run(3, "main", "push", "queued", 1000, now),
            run(4, "main", "push", "queued", 9000, now),
        ];
        classify_runs(&mut runs, 3600 * 24, true);
        assert!(runs.iter().all(|r| r.class == RunClass::Active));
        assert!(runs[2].exempt && runs[3].exempt);
    }

    #[test]
    fn stale_ttl_and_fleet_gate() {
        let now = 10_000;
        let ttl = 2700;
        let mk = |id, st: &str, started| run(id, &format!("b{id}"), "push", st, started, now);
        let mut runs = vec![
            mk(1, "queued", now - ttl),      // exactly TTL: not stale
            mk(2, "queued", now - ttl - 1),  // past TTL: stale
            mk(3, "pending", now - ttl - 1), // pending counts (concurrency-blocked)
            mk(4, "in_progress", 0),         // in_progress: never stale
        ];
        classify_runs(&mut runs, ttl, true);
        assert_eq!(runs[0].class, RunClass::Active);
        assert_eq!(runs[1].class, RunClass::Stale);
        assert_eq!(runs[2].class, RunClass::Stale);
        assert_eq!(runs[3].class, RunClass::Active);
        // Fleet down: stale sweep disabled entirely (outage != abandonment).
        let mut runs2 = vec![mk(5, "queued", 0)];
        classify_runs(&mut runs2, ttl, false);
        assert_eq!(runs2[0].class, RunClass::Active);
    }

    #[test]
    fn advice_phrasings() {
        assert!(advice_for(3, 4, 0, 0, 2, false).contains("healthy"));
        let clearable = advice_for(14, 4, 9, 3, 2, false);
        assert!(clearable.contains("vox ci queue --clear"));
        assert!(clearable.contains("9 superseded") && clearable.contains("3 stale"));
        let outage = advice_for(9, 4, 0, 0, 0, false);
        assert!(outage.contains("outage") && outage.contains("runner-status"));
        let backlog = advice_for(9, 4, 0, 0, 2, false);
        assert!(backlog.contains("real demand"));
        let deg = advice_for(0, 4, 0, 0, 0, true);
        assert!(deg.contains("degraded") && deg.contains("local gates"));
    }

    #[test]
    fn failure_advice_leads() {
        let f = FailedRun {
            id: 123,
            workflow: "ci.yml".into(),
            branch: "feat/x".into(),
            conclusion: "failure".into(),
            head_sha: "abc".into(),
            completed_epoch: 0,
            url: "https://g/123".into(),
        };
        let a = failure_advice(&f);
        assert!(a.contains("123") && a.contains("--log-failed") && a.contains("do not push blind retries"));
    }

    #[test]
    fn parse_failed_line_roundtrip() {
        let f = failed(7, "feat/x", 5000);
        assert_eq!((f.id, f.completed_epoch), (7, 5000));
        assert!(parse_failed_line("garbage").is_none());
    }

    #[test]
    fn failures_window_and_cap() {
        let now = 200_000;
        let mut fs: Vec<FailedRun> = (0..30).map(|i| failed(i, "b", now - 100)).collect();
        fs.push(failed(99, "old", now - FAILURE_WINDOW_SECS - 1));
        let kept = filter_failures(fs, now);
        assert_eq!(kept.len(), FAILURE_CAP);
        assert!(kept.iter().all(|f| f.id != 99));
    }

    #[test]
    fn snapshot_roundtrip_and_brief() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now),
            run(2, "feat/x", "push", "queued", 3000, now),
        ];
        classify_runs(&mut runs, 2700, true);
        let fails = vec![failed(9, "feat/x", now - 60), failed(10, "main", now - 120)];
        let snap = build_snapshot(runs, fails, 2, 4, false, now, vec![]);
        assert_eq!((snap.queued, snap.superseded), (2, 1));
        let back: QueueSnapshot =
            serde_json::from_str(&serde_json::to_string(&snap).unwrap()).unwrap();
        assert_eq!(back.advice, snap.advice);

        let brief = render_brief(&back, Some("feat/x"), now);
        assert!(brief.contains("FAILED on feat/x: ci.yml #9"));
        assert!(brief.contains("FAILED on main: ci.yml #10"));
        assert!(brief.contains("do not push blind retries")); // failure advice leads
        assert!(brief.lines().count() <= 7);

        let clean = build_snapshot(vec![], vec![], 2, 4, false, now, vec![]);
        let brief2 = render_brief(&clean, Some("feat/x"), now);
        assert!(!brief2.contains("FAILED"));
        assert!(brief2.contains("advice:"));
    }

    #[test]
    fn snapshot_staleness() {
        assert!(!snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS));
        assert!(snapshot_is_stale(1000, 1000 + SNAPSHOT_STALE_SECS + 1));
    }

    #[test]
    fn clear_plan_selects_only_cancellable_and_caps() {
        let now = 10_000;
        let mut runs = vec![
            run(1, "feat/x", "push", "queued", 1000, now), // old + superseded by 2
            run(2, "feat/x", "push", "queued", 9500, now), // recent (age 500 < ttl) + newest: active
            run(3, "feat/y", "push", "queued", 1, now),    // stale
            run(4, "main", "push", "queued", 1, now),      // exempt
        ];
        classify_runs(&mut runs, 2700, true);
        let snap = build_snapshot(runs, vec![], 2, 4, false, now, vec![]);
        let ids: Vec<u64> = clear_plan(&snap).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 3]);

        // Cap: 60 stale runs -> plan holds MAX_CANCELS_PER_SWEEP.
        let mut many: Vec<QueueRun> = (0..60)
            .map(|i| run(i, &format!("b{i}"), "push", "queued", 1, now))
            .collect();
        classify_runs(&mut many, 2700, true);
        let snap2 = build_snapshot(many, vec![], 2, 4, false, now, vec![]);
        assert_eq!(clear_plan(&snap2).len(), MAX_CANCELS_PER_SWEEP);
    }

    #[test]
    fn hook_guard_patterns() {
        // Banned.
        assert!(hook_guard_matches("gh pr checks 431 --watch"));
        assert!(hook_guard_matches("gh pr checks 431")); // one-shot too (contract: snapshot is the channel)
        assert!(hook_guard_matches("gh  pr   checks")); // whitespace collapse
        assert!(hook_guard_matches("GH RUN WATCH 12345")); // case
        assert!(hook_guard_matches("gh api repos/o/r/commits/abc/check-runs"));
        assert!(hook_guard_matches("gh api repos/o/r/check_runs --paginate"));
        assert!(hook_guard_matches("vox ci watch-run --sha abc"));
        assert!(hook_guard_matches("cargo run -p vox-cli -- ci watch-run"));
        // Loop heuristic: hand-rolled watchers from allowed one-shots.
        assert!(hook_guard_matches("while true; do gh run list --branch x; sleep 15; done"));
        assert!(hook_guard_matches(
            "for i in $(seq 40); do gh pr view 4 --json statusCheckRollup; sleep 30; done"
        ));
        assert!(hook_guard_matches(
            "until false; do gh api repos/o/r/actions/runs; sleep 9; done"
        ));
        // Alias evasion.
        assert!(hook_guard_matches("gh alias set pc 'pr checks'"));
        // Allowed: one-shot reads, failure logs, our own commands.
        assert!(!hook_guard_matches("gh run list --status queued"));
        assert!(!hook_guard_matches("gh run view 12345 --log-failed"));
        assert!(!hook_guard_matches("gh run view 12345 --log && pnpm vitest --watch")); // rev-1 FP, arm dropped
        assert!(!hook_guard_matches("gh pr view 431 --json statusCheckRollup")); // one-shot
        assert!(!hook_guard_matches("gh pr view 431"));
        assert!(!hook_guard_matches("vox ci queue --json"));
        assert!(!hook_guard_matches("cargo test && sleep 5 && gh run list")); // sleep without loop keyword
        assert!(!hook_guard_matches("git push"));
    }
}
