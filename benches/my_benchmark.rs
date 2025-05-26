use criterion::{Criterion, black_box, criterion_group, criterion_main};
use xecs::{
    component::{Component, ComponentDesc, TagDesc},
    storage::StorageType,
    world::{World, WorldGet},
};
use xecs_macros::params;

#[derive(Default)]
pub struct Position(pub i32);

impl Component for Position {}

#[derive(Default)]
pub struct Velocity(pub i32);

impl Component for Velocity {}

fn bench_sparse_set(c: &mut Criterion) {
    let mut world = World::new();
    let pos = world.register::<Position>(ComponentDesc::new().storage(StorageType::Sparse));
    let vel = world.register::<Velocity>(ComponentDesc::new().storage(StorageType::Sparse));
    let likes = world.new_tag(TagDesc::new().storage(StorageType::Sparse));
    let alice = world.new_id();
    let bob = world.new_id();

    world.set::<Position>(bob, Position(42));

    use xecs_macros::params as p;

    c.bench_function("test sparse", |b| {
        b.iter(|| {
            assert!(
                world
                    .get::<p!(Position?, mut Velocity?)>(bob, |(p, v)| assert!(p.is_some()))
                    .is_ok()
            );
        })
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
