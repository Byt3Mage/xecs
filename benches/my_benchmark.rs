use criterion::{Criterion, black_box, criterion_group, criterion_main};
use xecs::{component::ComponentDesc, storage::StorageType, world::World};

struct Position(i32);

fn bench_sparse_set(c: &mut Criterion) {
    let mut world = World::new();
    let time = world.register::<f32>(ComponentDesc::new().storage(StorageType::Sparse));
    let pos = world.register::<Position>(ComponentDesc::new().storage(StorageType::Sparse));
    let bob = world.new_id();

    world.set(bob, Position(69)).unwrap();

    c.bench_function("test sparse", |b| {
        b.iter(|| {
            assert!(world.get::<Position>(bob).is_ok());
        })
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
