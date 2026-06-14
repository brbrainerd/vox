//! Benchmarks for PEFT adapters

#![allow(missing_docs)]

use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_adapters(c: &mut Criterion) {
    // Benchmarks will be implemented as adapters are completed
    let group = c.benchmark_group("adapters");
    group.finish();
}

criterion_group!(benches, benchmark_adapters);
criterion_main!(benches);
