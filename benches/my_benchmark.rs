use criterion::{Criterion, criterion_group, criterion_main};

fn bench_sparse_set(c: &mut Criterion) {
    c.bench_function("fill_usize_max", |b| {
        b.iter(|| {});
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
