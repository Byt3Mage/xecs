use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use xecs::world::World;
use std::collections::HashMap;

struct Component { id: usize }

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut map = HashMap::new();
    map.insert(0, Component { id: 42 });
    map.insert(1, Component { id: 69 });

    c.bench_with_input(BenchmarkId::new("map_access", "test"), &map, |b, map| {
        b.iter(|| {
            // Force the compiler to treat all operations as "unpredictable"
            let idx = black_box(1);
            if let Some(comp) = black_box(map).get(black_box(&idx)) {
                black_box(comp.id == black_box(69))
            }
            else {
                black_box(false)
            }
            
        });
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);