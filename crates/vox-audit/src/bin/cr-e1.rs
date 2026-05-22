//! CR-E1 cold-start benchmark — runs the compiler pipeline N times on a
//! "hello world" fixture and writes p50/p95/p99 latency to
//! `contracts/reports/perf/cr-e1/<UTC>.json`. Exits non-zero if p99 > 50ms.
//!
//! Per `docs/superpowers/specs/2026-05-21-v1-honest-completion-plan.md` §5.4,
//! this is the canonical CR-E1 measurement. Invoked locally as
//! `cargo run -p vox-audit --bin cr-e1`; CI calls it on every PR.
//!
//! The threshold (50ms) tracks the v1-release-criteria §3 CR-E1 number.
//! The measurement intentionally re-loads the source string each iteration
//! to capture honest cold-path costs (parser + HIR lowering + typeck),
//! not warm-cache costs.

use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_module;

const HELLO: &str = r#"fn greet(name: str) to str { return "Hello, " + name }"#;

/// CR-E1 budget. Honest plan §5.4 / v1-release-criteria CR-E1.
const P99_BUDGET_MS: f64 = 50.0;

/// Number of measured iterations. Big enough to stabilize p99, small
/// enough to run on every PR.
const ITERATIONS: usize = 200;

/// Warmup iterations (results discarded). Smooths the cold-cache spike on
/// the first few runs.
const WARMUP: usize = 20;

fn main() {
    eprintln!("CR-E1: running {ITERATIONS} pipeline iterations on `hello world`");

    // Warmup
    for _ in 0..WARMUP {
        let _ = run_pipeline_once();
    }

    // Measured
    let mut samples_ms: Vec<f64> = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let elapsed = run_pipeline_once();
        samples_ms.push(elapsed * 1000.0);
    }

    samples_ms.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = percentile(&samples_ms, 50.0);
    let p95 = percentile(&samples_ms, 95.0);
    let p99 = percentile(&samples_ms, 99.0);
    let min_ms = *samples_ms.first().unwrap_or(&0.0);
    let max_ms = *samples_ms.last().unwrap_or(&0.0);

    let met = p99 <= P99_BUDGET_MS;
    eprintln!("CR-E1 results:");
    eprintln!("  min/p50/p95/p99/max = {min_ms:.2}/{p50:.2}/{p95:.2}/{p99:.2}/{max_ms:.2} ms");
    eprintln!("  budget p99 ≤ {P99_BUDGET_MS:.0} ms — met: {met}");

    let artifact = json!({
        "schema_version": 1,
        "criterion": "CR-E1",
        "measured_at": chrono::Utc::now().to_rfc3339(),
        "fixture": "hello_world",
        "iterations": ITERATIONS,
        "warmup": WARMUP,
        "samples_ms": {
            "min": min_ms,
            "p50": p50,
            "p95": p95,
            "p99": p99,
            "max": max_ms,
        },
        "budget_ms": P99_BUDGET_MS,
        "threshold": {
            "target_p99_ms": P99_BUDGET_MS,
            "met": met,
        },
    });

    let body = serde_json::to_string_pretty(&artifact).expect("serialize");
    let workspace = vox_audit::workspace_root();
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let out_dir = workspace
        .join("contracts")
        .join("reports")
        .join("perf")
        .join("cr-e1");
    std::fs::create_dir_all(&out_dir).expect("create perf dir");
    let out_path = out_dir.join(format!("{date}.json"));
    std::fs::write(&out_path, body).expect("write artifact");
    eprintln!("artifact: {}", out_path.display());

    if !met {
        std::process::exit(1);
    }
}

fn run_pipeline_once() -> f64 {
    let started = Instant::now();
    let tokens = lex(HELLO);
    let ast = parse(tokens).expect("parse hello.vox");
    let _ = typecheck_module(&ast, HELLO);
    let _ = lower_module(&ast);
    started.elapsed().as_secs_f64()
}

/// Linear-interpolation percentile over a sorted slice of f64s.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (p / 100.0) * (sorted.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        return sorted[lo];
    }
    let frac = rank - lo as f64;
    sorted[lo] + (sorted[hi] - sorted[lo]) * frac
}
