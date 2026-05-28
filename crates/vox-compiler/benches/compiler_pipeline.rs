//! Criterion benchmarks for the core compiler pipeline (v1.0 CR-E1 gate).
//!
//! Measures lex → parse → HIR-lower → typeck → TS-codegen on a representative
//! "Hello World" and a mid-complexity CRUD program so CI can track regressions
//! against the v1.0 cold-start target of <50 ms.
//!
//! Run:
//!   cargo bench -p vox-compiler --bench compiler_pipeline

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use vox_codegen::codegen_ts::generate;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_module;

// ── Fixtures ──────────────────────────────────────────────────────────────────

const HELLO: &str = r#"
fn greet(name: str) to str {
    ret "Hello, " + name
}
"#;

const CRUD_API: &str = r#"
@table type Task {
    id: int
    title: str
    done: bool
}

@query
fn list_tasks() to list[Task] {
    ret db.Task.find_all()
}

@mutation
fn create_task(title: str) to Task {
    ret db.Task.insert({ title: title, done: false })
}

@mutation
fn complete_task(id: int) to Task {
    ret db.Task.update(id, { done: true })
}

@mutation
fn delete_task(id: int) to Unit {
    db.Task.delete(id)
}
"#;

// ── Individual pipeline stages ────────────────────────────────────────────────

fn bench_lex(c: &mut Criterion) {
    let mut g = c.benchmark_group("lex");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        g.bench_with_input(BenchmarkId::new("lex", name), src, |b, src| {
            b.iter(|| lex(black_box(src)));
        });
    }
    g.finish();
}

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        let tokens = lex(src);
        g.bench_with_input(BenchmarkId::new("parse", name), &tokens, |b, tokens| {
            b.iter(|| parse(black_box(tokens.clone())).expect("bench parse should succeed"));
        });
    }
    g.finish();
}

fn bench_lower(c: &mut Criterion) {
    let mut g = c.benchmark_group("hir_lower");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        let module = parse(lex(src)).expect("bench lower: parse");
        g.bench_with_input(BenchmarkId::new("lower", name), &module, |b, module| {
            b.iter(|| lower_module(black_box(module)));
        });
    }
    g.finish();
}

fn bench_typeck(c: &mut Criterion) {
    let mut g = c.benchmark_group("typeck");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        let module = parse(lex(src)).expect("bench typeck: parse");
        g.bench_with_input(BenchmarkId::new("typeck", name), &module, |b, module| {
            b.iter(|| typecheck_module(black_box(module), ""));
        });
    }
    g.finish();
}

fn bench_codegen(c: &mut Criterion) {
    let mut g = c.benchmark_group("codegen_ts");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        let hir = lower_module(&parse(lex(src)).expect("bench codegen: parse"));
        g.bench_with_input(BenchmarkId::new("codegen", name), &hir, |b, hir| {
            b.iter(|| generate(black_box(hir)).expect("bench codegen should succeed"));
        });
    }
    g.finish();
}

// ── Full end-to-end pipeline (CR-E1 proxy) ────────────────────────────────────

/// Full lex→parse→lower→typeck→codegen pipeline.
/// CR-E1 target: complete for a hello-world in <50 ms.
fn bench_full_pipeline(c: &mut Criterion) {
    let mut g = c.benchmark_group("full_pipeline");
    for (name, src) in [("hello", HELLO), ("crud_api", CRUD_API)] {
        g.bench_with_input(BenchmarkId::new("e2e", name), src, |b, src| {
            b.iter(|| {
                let tokens = lex(black_box(src));
                let module = parse(tokens).expect("e2e parse");
                let _diags = typecheck_module(&module, "");
                let hir = lower_module(&module);
                let _out = generate(&hir).expect("e2e codegen");
            });
        });
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_lex,
    bench_parse,
    bench_lower,
    bench_typeck,
    bench_codegen,
    bench_full_pipeline,
);
criterion_main!(benches);
