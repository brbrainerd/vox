//! Scoped, daemonless `cargo` shim for the vox build broker.
//!
//! Placed on PATH (for IDE-spawned terminals only) ahead of the rustup cargo
//! proxy. For build subcommands inside a vox worktree it takes a fair per-worktree
//! queue ticket, runs the real cargo, and records a metric. For everything else —
//! or on any error — it transparently runs the real cargo. The broker is never a
//! hard dependency: a misconfiguration degrades to plain cargo.
//!
//! See `docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use vox_build_queue::{env_filter, metrics, queue, resolve};

/// Env var tracking shim nesting depth; a hard backstop against fork bombs.
const DEPTH_VAR: &str = "VOX_BROKER_DEPTH";

/// Resolve the real cargo, skipping this shim and any sibling copy. Prefers the
/// rustup proxy under `$CARGO_HOME`/`~/.cargo/bin` (never a shim copy), then
/// falls back to a PATH scan. None if not found.
fn real_cargo() -> Option<PathBuf> {
    let own = std::env::current_exe().ok()?;
    let own_canon = own.canonicalize().ok();
    if let Some(proxy) = resolve::cargo_home_proxy() {
        if proxy.canonicalize().ok() != own_canon {
            return Some(proxy);
        }
    }
    let path = std::env::var("PATH").unwrap_or_default();
    resolve::resolve_real_cargo(&path, &own)
}

/// Run real cargo with the original args, propagating its exit code. Never returns.
fn exec_real(real: &PathBuf, args: &[String], depth: u32) -> ! {
    let code = Command::new(real)
        .args(args)
        .env(DEPTH_VAR, (depth + 1).to_string())
        .status()
        .ok()
        .and_then(|s| s.code())
        .unwrap_or(1);
    std::process::exit(code);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Hard recursion guard: if we are already nested past one level, resolution
    // must have picked a shim copy. Abort rather than fork-bomb.
    let depth: u32 = std::env::var(DEPTH_VAR)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if depth >= 2 {
        eprintln!("vox-broker: recursion detected (depth {depth}); aborting to avoid fork bomb");
        std::process::exit(70);
    }

    let real = match real_cargo() {
        Some(r) => r,
        None => {
            eprintln!("vox-broker: real cargo not found on PATH; aborting");
            std::process::exit(127);
        }
    };

    // Escape hatch for diagnosing resolution without running a build.
    if std::env::var_os("VOX_BROKER_DEBUG").is_some() {
        eprintln!(
            "vox-broker-debug: own={:?} real={:?} args={:?}",
            std::env::current_exe(),
            real,
            args
        );
        std::process::exit(0);
    }

    let sub = args.first().map(String::as_str).unwrap_or("");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let worktree = resolve::worktree_root_of(&cwd);

    // Fast path: non-build subcommand or outside any worktree -> direct exec.
    if !resolve::is_build_subcommand(sub) || worktree.is_none() {
        exec_real(&real, &args, depth);
    }
    let worktree = worktree.unwrap();

    // Queued path. Any error falls back to a plain exec.
    match run_queued(&real, &args, sub, &cwd, &worktree, depth) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("vox-broker: queue error ({e}); running cargo directly");
            exec_real(&real, &args, depth);
        }
    }
}

fn run_queued(
    real: &PathBuf,
    args: &[String],
    sub: &str,
    cwd: &std::path::Path,
    worktree: &std::path::Path,
    depth: u32,
) -> anyhow::Result<i32> {
    let queue_root = worktree.join(".vox/build-queue");
    let wt_hash = queue::hash_path(worktree);
    let q = queue::FairQueue::new(&queue_root, &wt_hash)?;
    // Logs live in the same per-worktree hash dir as the queue files, so
    // `metrics::summarize_worktree` (which scans `<hash>/metrics.jsonl`) sees them.
    let log_dir = queue_root.join(&wt_hash);

    let env = env_filter::passthrough_env(std::env::vars());
    let argv_hash = env_filter::argv_hash(args);
    let env_hash = env_filter::env_hash(&env);
    let key = format!("{argv_hash:016x}-{env_hash:016x}");

    let seq = q.take_ticket(&key)?;
    let would_coalesce = q.coalesce_opportunity(seq, &key);

    let pos = q.position(seq);
    if pos > 0 {
        eprintln!(
            "vox-broker: queued (position {pos}) for {}",
            worktree.display()
        );
    }

    let t_wait = Instant::now();
    let _ticket = q.acquire(seq)?;
    let queue_wait_ms = t_wait.elapsed().as_millis() as u64;

    let t_run = Instant::now();
    let mut cmd = Command::new(real);
    cmd.args(args).current_dir(cwd);
    cmd.env_clear();
    for (k, v) in &env {
        cmd.env(k, v);
    }
    cmd.env("CARGO_TERM_COLOR", "always");
    cmd.env(DEPTH_VAR, (depth + 1).to_string());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd.status()?;
    let ran_ms = t_run.elapsed().as_millis() as u64;

    let rec = metrics::MetricRecord {
        ts_ms: metrics::now_ms(),
        worktree: worktree.display().to_string(),
        subcmd: sub.to_string(),
        queue_wait_ms,
        ran_ms,
        argv_hash,
        env_hash,
        would_coalesce,
    };
    let _ = metrics::append(&log_dir.join("metrics.jsonl"), &rec);

    // Human-readable surface so the broker's effect is observable at a glance.
    // `waited` > 0 marks a contention event the queue absorbed instead of
    // letting cargo block opaquely on its target lock.
    let line = format!(
        "{ts} {sub:<6} waited={queue_wait_ms:>6}ms ran={ran_ms:>7}ms queued_behind={pos} coalesce={would_coalesce} exit={code}\n",
        ts = rec.ts_ms,
        code = status.code().unwrap_or(-1),
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("broker.log"))
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }

    Ok(status.code().unwrap_or(1))
}
