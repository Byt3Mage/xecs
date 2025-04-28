use std::alloc::Layout;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_fill(c: &mut Criterion) {
    c.bench_function("fill_usize_max", |b| {
        b.iter(|| {});
    });
}

criterion_group!(benches, bench_fill);
criterion_main!(benches);
