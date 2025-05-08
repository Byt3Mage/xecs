use criterion::{Criterion, black_box, criterion_group, criterion_main};
use xecs::component::ComponentDesc;
use xecs::storage::StorageType;
use xecs::world::World;

struct Position(i32);

fn bench_type_map(c: &mut Criterion) {
    let mut world = World::new();
    world.register::<Position>(ComponentDesc::new().storage(StorageType::Tables));

    let bob = world.new_entity();
    world.set_t(bob, Position(45)).unwrap();

    c.bench_function("test component get", |b| {
        b.iter(|| assert!(world.get_t::<Position>(bob).is_ok()));
    });
}

criterion_group!(benches, bench_type_map);
criterion_main!(benches);
