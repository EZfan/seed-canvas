//! Benchmarks for the deterministic seed stream.
//!
//! Run with `cargo bench -p seed-canvas-core`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use seed_canvas_core::Seed;

fn bench_next_u64(c: &mut Criterion) {
    let mut s = Seed::from_string("cosmos");
    c.bench_function("seed/next_u64", |b| b.iter(|| black_box(s.next_u64())));
}

fn bench_fork(c: &mut Criterion) {
    let s = Seed::from_string("cosmos");
    c.bench_function("seed/fork", |b| b.iter(|| black_box(s.fork("color"))));
}

fn bench_construction(c: &mut Criterion) {
    c.bench_function("seed/from_string", |b| {
        b.iter(|| black_box(Seed::from_string("cosmos")))
    });
}

criterion_group!(benches, bench_next_u64, bench_fork, bench_construction);
criterion_main!(benches);
