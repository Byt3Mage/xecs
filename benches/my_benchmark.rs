use criterion::{Criterion, black_box, criterion_group, criterion_main};
use xecs::entity::Entity;
use xecs::types::TypeMap;

fn bench_type_map(c: &mut Criterion) {
    let mut map = TypeMap::new();
    map.insert::<usize>(Entity::NULL);
    map.insert::<String>(Entity::NULL);

    c.bench_function("get_type_map", |b| {
        b.iter(|| {});
    });
}

criterion_group!(benches, bench_type_map);
criterion_main!(benches);
