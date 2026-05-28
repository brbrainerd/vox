//! Warm-cache benchmark for `vox run`.
//!
//! Measures wall-clock latency for cold-cache (first compile) vs warm-cache
//! (binary already cached on disk) `vox run` executions.
//!
//! Run with:
//!   cargo test -p vox-cli --test run_benchmark -- --nocapture
//!
//! The test fails if the warm-cache pass takes longer than 500 ms — a regression
//! gate that keeps startup time Jai-class fast after subsequent writes.
//!
//! Note: this test calls the `vox` binary via `std::process::Command`, so the
//! workspace must be compiled first (`cargo build -p vox-cli`).

use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to the compiled `vox` binary for the current build profile.
fn vox_bin() -> PathBuf {
    // Prefer the dev binary; fall back to release.
    let candidates = [
        "target/debug/vox",
        "target/debug/vox.exe",
        "target/release/vox",
        "target/release/vox.exe",
    ];
    let workspace = workspace_root();
    for c in &candidates {
        let p = workspace.join(c);
        if p.exists() {
            return p;
        }
    }
    panic!("Could not find compiled `vox` binary. Run `cargo build -p vox-cli` first.");
}

fn workspace_root() -> PathBuf {
    // Walk upward from this file's manifest directory until we find Cargo.toml
    // with [workspace] (the root).
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            let content = std::fs::read_to_string(&candidate).unwrap_or_default();
            if content.contains("[workspace]") {
                return dir;
            }
        }
        if !dir.pop() {
            break;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Write a minimal Vox hello-world script to a temp file and return its path.
fn write_hello_script(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(
        &path,
        "fn main():\n    print(str(\"hello from vox run benchmark\"))\n",
    )
    .expect("write benchmark script");
    path
}

/// Remove script cache dirs so the next `vox run` measures a cold compile (there is no cache-clean subcommand on the CLI).
fn wipe_user_script_caches() {
    for wasi in [false, true] {
        let p = vox_config::paths::script_cache_dir(wasi);
        let _ = std::fs::remove_dir_all(&p);
    }
}

/// Run `vox run --mode script <script>` (requires `script-execution` feature on the binary).
fn timed_vox_run(script: &Path) -> (Duration, bool) {
    let vox = vox_bin();
    let start = Instant::now();
    let status = Command::new(&vox)
        .args(["run", "--mode", "script", script.to_str().unwrap()])
        .env("VOX_SCRIPT_RELEASE", "") // force dev profile for fairness
        .status();
    let elapsed = start.elapsed();
    let ok = status.map(|s| s.success()).unwrap_or(false);
    (elapsed, ok)
}

// ---------------------------------------------------------------------------
// Benchmark tests
// ---------------------------------------------------------------------------

/// Smoke-test that `vox run` can compile and execute a minimal script at all.
/// Asserts exit 0 only — no timing constraint (cold compile can take seconds).
#[test]
#[ignore = "requires compiled vox binary and cargo; run with --ignored — owner: vox-cli sunset: 2026-12-31"]
fn cold_cache_vox_run_succeeds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let script = write_hello_script(tmp.path(), "hello.vox");

    // Wipe script cache so we measure cold compile (no CLI subcommand for this).
    wipe_user_script_caches();

    let (elapsed, ok) = timed_vox_run(&script);

    println!("[bench] cold-cache vox run: {:.2?}", elapsed);
    assert!(ok, "cold-cache vox run must exit 0");
}

/// Assert that a second `vox run` (warm cache, binary already on disk) completes
/// within 500 ms.  This is the Jai-class startup-time regression gate.
#[test]
#[ignore = "requires compiled vox binary and cargo; run with --ignored — owner: vox-cli sunset: 2026-12-31"]
fn warm_cache_vox_run_under_500ms() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let script = write_hello_script(tmp.path(), "hello_warm.vox");

    // Pass 1 – build the cache (cold)
    let (cold, ok_cold) = timed_vox_run(&script);
    println!("[bench] cold-cache: {:.2?}", cold);
    assert!(ok_cold, "cold pass must succeed");

    // Pass 2 – use the cached binary (warm)
    let (warm, ok_warm) = timed_vox_run(&script);
    println!("[bench] warm-cache: {:.2?}", warm);
    assert!(ok_warm, "warm pass succeed");

    // Regression gate
    assert!(
        warm < Duration::from_millis(500),
        "warm-cache vox run took {:.2?} — exceeds 500 ms budget. \
         Check cache invalidation logic in fs_utils::gc_script_cache and \
         NativeBackend::compile.",
        warm,
    );

    vox_cli::benchmark_telemetry::record_opt_blocking(
        "vox_run_warm_ms",
        Some(warm.as_secs_f64() * 1000.0),
        None,
    );
}

/// Compare cold vs warm latency and log the speedup factor.
/// Does NOT fail on any timing — purely informational.
#[test]
#[ignore = "requires compiled vox binary and cargo; run with --ignored — owner: vox-cli sunset: 2026-12-31"]
fn benchmark_cold_vs_warm_speedup() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let script = write_hello_script(tmp.path(), "hello_speedup.vox");

    let (cold, _) = timed_vox_run(&script);
    let (warm, _) = timed_vox_run(&script);

    let speedup = cold.as_secs_f64() / warm.as_secs_f64().max(0.001);
    println!(
        "[bench] cold={:.2?}  warm={:.2?}  speedup={:.1}×",
        cold, warm, speedup
    );
}
