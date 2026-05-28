//! Criterion benchmarks over all `examples/golden/*.vox` files.
//!
//! Tracks aggregate pipeline throughput across the full golden corpus — useful
//! for catching regressions introduced by parser or codegen changes that affect
//! diverse programs rather than a single fixture.
//!
//! Run:
//!   cargo bench -p vox-compiler --bench golden_examples

use std::path::PathBuf;

use criterion::{BatchSize, BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use vox_codegen::codegen_ts::generate;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_module;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn load_golden_sources() -> Vec<(String, String)> {
    let golden = repo_root().join("examples/golden");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&golden) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "vox")
                && let Ok(src) = std::fs::read_to_string(&p)
            {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_owned();
                out.push((name, src));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Benchmark: lex every golden file (corpus throughput).
fn bench_golden_lex(c: &mut Criterion) {
    let sources = load_golden_sources();
    if sources.is_empty() {
        return;
    }

    let mut g = c.benchmark_group("golden_lex");
    for (name, src) in &sources {
        g.bench_with_input(BenchmarkId::new("lex", name), src, |b, src| {
            b.iter(|| lex(black_box(src)));
        });
    }
    g.finish();
}

/// Benchmark: full pipeline over every golden file.
/// Highlights which golden files are slowest to compile.
fn bench_golden_full_pipeline(c: &mut Criterion) {
    let sources = load_golden_sources();
    if sources.is_empty() {
        return;
    }

    let mut g = c.benchmark_group("golden_full_pipeline");
    for (name, src) in &sources {
        g.bench_with_input(BenchmarkId::new("e2e", name), src, |b, src| {
            b.iter_batched(
                || src.clone(),
                |src| {
                    let tokens = lex(black_box(&src));
                    if let Ok(module) = parse(tokens) {
                        let _diags = typecheck_module(&module, "");
                        let hir = lower_module(&module);
                        let _out = generate(&hir);
                    }
                },
                BatchSize::SmallInput,
            );
        });
    }
    g.finish();
}

/// Aggregate throughput: compile all goldens as a single timed batch.
/// This is the closest proxy to `vox build` over a workspace.
fn bench_golden_batch_pipeline(c: &mut Criterion) {
    let sources = load_golden_sources();
    if sources.is_empty() {
        return;
    }

    c.bench_function("golden_batch_pipeline/all", |b| {
        b.iter(|| {
            for (_name, src) in &sources {
                let tokens = lex(black_box(src));
                if let Ok(module) = parse(tokens) {
                    let _diags = typecheck_module(&module, "");
                    let hir = lower_module(&module);
                    let _out = generate(&hir);
                }
            }
        });
    });
}

criterion_group!(
    benches,
    bench_golden_lex,
    bench_golden_full_pipeline,
    bench_golden_batch_pipeline,
);
criterion_main!(benches);
