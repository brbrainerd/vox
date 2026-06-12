//! Interpreter memory/throughput micro-benchmarks (Phase 0 baseline for the
//! value-semantics optimization plan).
//!
//! These exercise the clone-cost cliffs that copy-on-write (`Rc` + `make_mut`)
//! collection payloads and cactus-stack closure scopes are meant to remove. See
//! `docs/src/architecture/vox-memory-model-audit-and-value-optimization-2026-06-05.md`.
//!
//! Each case drives the tree-walking interpreter end-to-end (run_module + call
//! "main") on a fresh `Interpreter`, so the timing reflects runtime value
//! copying — not lex/parse/lower (done once outside the timed loop).
//!
//! Run:
//!   cargo bench -p vox-compiler --bench eval_memory
//!
//! Capture a named baseline to compare against post-CoW:
//!   cargo bench -p vox-compiler --bench eval_memory -- --save-baseline pre-cow

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vox_compiler::eval::Interpreter;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

/// Generous step budget — these programs do real work (recursion, big loops).
const STEP_LIMIT: usize = 500_000_000;

// ── Fixtures: each targets a specific clone cliff ─────────────────────────────

/// Append-in-loop. Today `list.push` clones the whole `Vec` per step → O(n²)
/// memory traffic. CoW `make_mut` on a uniquely-held list makes this O(n).
const APPEND_LOOP: &str = r#"
fn build(n: int) to int {
    let mut acc: list[int] = []
    let mut i = 0
    while i < n {
        acc.push(i)
        i = i + 1
    }
    return len(acc)
}
fn main() to int {
    return build(1500)
}
"#;

/// Pass-by-value of a large list. Each `total(acc)` deep-copies all elements
/// today; an `Rc` clone makes the argument pass O(1).
const BIG_LIST_PASS: &str = r#"
fn total(nums: list[int]) to int {
    return len(nums)
}
fn main() to int {
    let mut acc: list[int] = []
    let mut i = 0
    while i < 3000 {
        acc.push(i)
        i = i + 1
    }
    let mut sum = 0
    let mut k = 0
    while k < 250 {
        sum = sum + total(acc)
        k = k + 1
    }
    return sum
}
"#;

/// Closure-heavy map/filter/fold pipeline — allocates intermediate lists and
/// invokes lambdas that capture scope.
const CLOSURE_PIPELINE: &str = r#"
fn pipeline(nums: list[int]) to int {
    let doubled = nums.map(fn(x: int) to int { x * 2 })
    let evens = doubled.filter(fn(x: int) to bool { x % 4 is 0 })
    return evens.fold(0, fn(acc: int, x: int) to int { acc + x })
}
fn main() to int {
    let mut acc: list[int] = []
    let mut i = 0
    while i < 3000 {
        acc.push(i)
        i = i + 1
    }
    return pipeline(acc)
}
"#;

/// Deep recursion — stresses scope frame push/pop and per-call cloning of the
/// environment / HIR body in `apply_closure`-style dispatch.
const DEEP_RECURSION: &str = r#"
fn fib(n: int) to int {
    if n < 2 { return n }
    return fib(n - 1) + fib(n - 2)
}
fn main() to int {
    return fib(24)
}
"#;

/// Object pass-by-value. Each `dist(p)` clones the record's field `Vec` today.
const OBJECT_PASS: &str = r#"
type Point { x: int, y: int }
fn dist(p: Point) to int {
    return p.x + p.y
}
fn main() to int {
    let p: Point = { x: 3, y: 4 }
    let mut sum = 0
    let mut i = 0
    while i < 60000 {
        sum = sum + dist(p)
        i = i + 1
    }
    return sum
}
"#;

// ── Harness ───────────────────────────────────────────────────────────────────

/// Lex/parse/lower once (outside the timed loop), then time only the interpreter
/// running the program on a fresh `Interpreter`.
fn bench_program(c: &mut Criterion, name: &str, src: &str) {
    let tokens = lex(src);
    let module = parse_script(tokens)
        .unwrap_or_else(|e| panic!("bench fixture `{name}` failed to parse: {e:?}"));
    let lowered = lower_module(&module);

    c.bench_function(name, |b| {
        b.iter(|| {
            let mut interp = Interpreter::new(STEP_LIMIT);
            interp
                .run_module(black_box(&lowered))
                .unwrap_or_else(|e| panic!("`{name}` run_module: {e:?}"));
            let out = interp
                .call("main", vec![])
                .unwrap_or_else(|e| panic!("`{name}` call main: {e:?}"));
            black_box(out);
        });
    });
}

fn bench_eval_memory(c: &mut Criterion) {
    bench_program(c, "eval_memory/append_loop", APPEND_LOOP);
    bench_program(c, "eval_memory/big_list_pass", BIG_LIST_PASS);
    bench_program(c, "eval_memory/closure_pipeline", CLOSURE_PIPELINE);
    bench_program(c, "eval_memory/deep_recursion", DEEP_RECURSION);
    bench_program(c, "eval_memory/object_pass", OBJECT_PASS);
}

criterion_group!(benches, bench_eval_memory);
criterion_main!(benches);
