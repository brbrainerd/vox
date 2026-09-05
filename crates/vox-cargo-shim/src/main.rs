//! Daemonless `cargo` shim for the vox build broker.
//!
//! Placed on PATH ahead of the rustup cargo proxy. For build subcommands it
//! acquires a slot from a **machine-wide concurrency cap** (so N agents across
//! many worktrees can't all build at once and saturate the machine), runs the
//! real cargo, and records a metric to a single global log. For everything else —
//! or on any error — it transparently runs the real cargo. The broker is never a
//! hard dependency: a misconfiguration degrades to plain cargo.
//!
//! See `docs/superpowers/specs/2026-06-18-unified-build-broker-design.md`.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use vox_build_queue::{env_filter, global, metrics, resolve};

/// Env var tracking shim nesting depth; a hard backstop against fork bombs.
const DEPTH_VAR: &str = "VOX_BROKER_DEPTH";

/// Ask rustup for the active toolchain's cargo (honors `rust-toolchain.toml` /
/// overrides via cwd, and an explicit toolchain via `RUSTUP_TOOLCHAIN`). Used
/// when the shim is installed AS the cargo proxy, so the real cargo can't be
/// found by name on PATH (that name is us).
fn rustup_which_cargo(toolchain: Option<&str>) -> Option<PathBuf> {
    let mut c = Command::new("rustup");
    if let Some(tc) = toolchain {
        c.env("RUSTUP_TOOLCHAIN", tc);
    }
    let out = c.args(["which", "cargo"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
    p.is_file().then_some(p)
}

/// Resolve the real cargo. Two install modes:
/// - PATH-prepend: a DISTINCT rustup proxy exists at `~/.cargo/bin` → delegate
///   to it (it handles toolchain selection).
/// - Proxy-shadow: the shim IS `~/.cargo/bin/cargo.exe` → ask rustup for the
///   toolchain cargo. `RUSTUP_TOOLCHAIN` (set by the caller for `+toolchain`)
///   makes both modes honor an explicit toolchain.
fn real_cargo(toolchain: Option<&str>) -> Option<PathBuf> {
    let own = std::env::current_exe().ok()?;
    let own_canon = own.canonicalize().ok();
    if let Some(proxy) = resolve::cargo_home_proxy()
        && proxy.canonicalize().ok() != own_canon
    {
        return Some(proxy); // PATH-prepend mode
    }
    if let Some(c) = rustup_which_cargo(toolchain) {
        return Some(c); // proxy-shadow mode
    }
    let path = std::env::var("PATH").unwrap_or_default();
    resolve::resolve_real_cargo(&path, &own)
}

/// Run real cargo with the given args, propagating its exit code. Never returns.
/// `toolchain` (from a `+toolchain` arg) is applied via `RUSTUP_TOOLCHAIN`, which
/// both the rustup proxy and a toolchain-direct cargo respect.
fn exec_real(real: &PathBuf, args: &[String], depth: u32, toolchain: Option<&str>) -> ! {
    let mut c = Command::new(real);
    c.args(args).env(DEPTH_VAR, (depth + 1).to_string());
    if let Some(tc) = toolchain {
        c.env("RUSTUP_TOOLCHAIN", tc);
    }
    let code = c.status().ok().and_then(|s| s.code()).unwrap_or(1);
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

    // Split off a leading `+toolchain` so we can resolve/run the right cargo even
    // in proxy-shadow mode (where we can't just forward `+tc` to ourselves).
    let (toolchain, rest): (Option<String>, Vec<String>) =
        match args.first().and_then(|a| a.strip_prefix('+')) {
            Some(tc) => (Some(tc.to_string()), args[1..].to_vec()),
            None => (None, args.clone()),
        };
    let tc = toolchain.as_deref();

    let real = match real_cargo(tc) {
        Some(r) => r,
        None => {
            eprintln!("vox-broker: real cargo not found (rustup unavailable?); aborting");
            std::process::exit(127);
        }
    };

    // Escape hatch for diagnosing resolution without running a build.
    if std::env::var_os("VOX_BROKER_DEBUG").is_some() {
        eprintln!(
            "vox-broker-debug: own={:?} real={:?} toolchain={:?} args={:?}",
            std::env::current_exe(),
            real,
            toolchain,
            rest
        );
        std::process::exit(0);
    }

    let sub = rest.first().map(String::as_str).unwrap_or("");

    // Fast path: non-build subcommand -> direct exec, no coordination.
    if !resolve::is_build_subcommand(sub) {
        exec_real(&real, &rest, depth, tc);
    }

    // Coordinated path: machine-wide concurrency cap. Any error falls back to a
    // plain exec so the broker is never a hard dependency.
    match run_global(&real, &rest, sub, depth, tc) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("vox-broker: coordinator error ({e}); running cargo directly");
            exec_real(&real, &rest, depth, tc);
        }
    }
}

fn run_global(
    real: &PathBuf,
    args: &[String],
    sub: &str,
    depth: u32,
    toolchain: Option<&str>,
) -> anyhow::Result<i32> {
    let root = global::global_root();
    let n = global::effective_max_concurrent();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let env = env_filter::passthrough_env(std::env::vars());
    let argv_hash = env_filter::argv_hash(args);
    let env_hash = env_filter::env_hash(&env);
    // Identity includes cwd so the same command in the same dir coalesces, but
    // the same command in different worktrees counts as distinct work.
    let key = format!("{argv_hash:016x}-{env_hash:016x}-{}", cwd.display());

    let (_inflight, would_coalesce) = global::register_inflight(&root, &key)?;

    let t_wait = Instant::now();
    let (_slot, _waited_ms, busy) = global::acquire_slot(&root, n)?;
    let queue_wait_ms = t_wait.elapsed().as_millis() as u64;
    if busy > 0 {
        eprintln!(
            "vox-broker: queued (cap {n}, {busy} build(s) ahead) — {sub} in {}",
            cwd.display()
        );
    }

    let t_run = Instant::now();
    let mut cmd = Command::new(real);
    cmd.args(args).current_dir(&cwd);
    cmd.env_clear();
    for (k, v) in &env {
        cmd.env(k, v);
    }
    cmd.env("CARGO_TERM_COLOR", "always");
    cmd.env(DEPTH_VAR, (depth + 1).to_string());
    if let Some(tc) = toolchain {
        cmd.env("RUSTUP_TOOLCHAIN", tc);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd.status()?;
    let ran_ms = t_run.elapsed().as_millis() as u64;

    let worktree = resolve::worktree_root_of(&cwd).unwrap_or_else(|| cwd.clone());
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
    // One global metrics + log location (outside any repo → git-proof, and a
    // single place to observe ALL worktrees' build activity).
    let _ = metrics::append(&root.join("metrics.jsonl"), &rec);
    let line = format!(
        "{ts} {sub:<6} wait={queue_wait_ms:>6}ms ran={ran_ms:>7}ms ahead={busy} cap={n} coalesce={would_coalesce} exit={code} {wt}\n",
        ts = rec.ts_ms,
        code = status.code().unwrap_or(-1),
        wt = worktree.display(),
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("broker.log"))
    {
        use std::io::Write;
        let _ = f.write_all(line.as_bytes());
    }

    Ok(status.code().unwrap_or(1))
}
