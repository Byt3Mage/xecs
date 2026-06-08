use criterion::{Criterion, criterion_group, criterion_main};

fn bench_sparse_set(c: &mut Criterion) {
    c.bench_function("test sparse", |b| {
        b.iter(|| {});
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
