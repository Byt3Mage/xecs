use criterion::{Criterion, criterion_group, criterion_main};
use std::marker::PhantomData;
use xecs::query::{Context, QueryPlan, SelectStmt, WithStmt};
pub use xecs::{
    component::{ComponentBuilder, TagBuilder},
    storage::StorageType,
    type_traits::DataComponent,
    world::{World, WorldGet},
};
use xecs_macros::Component;

#[derive(Component)]
enum MyEnum {
    A(String),
    B(usize),
}

#[derive(Component)]
struct Test;

#[derive(Component)]
struct Likes;

#[derive(Component)]
struct Position(u8);

#[derive(Component)]
struct Velocity(u8);

#[derive(Component)]
struct Generic<T: DataComponent>(PhantomData<T>);

fn bench_sparse_set(c: &mut Criterion) {
    let mut world = World::new();
    let test = world.register::<Test>(TagBuilder::new().storage(StorageType::Tables));
    let likes = world.register::<Likes>(TagBuilder::new().storage(StorageType::Tables));
    let pos = world.register::<Position>(ComponentBuilder::new().storage(StorageType::Tables));
    let vel = world.register::<Velocity>(ComponentBuilder::new().storage(StorageType::Tables));
    let my_enum = world.register::<MyEnum>(ComponentBuilder::new().storage(StorageType::Tables));

    let bob = world.new_id();

    world.add::<Test>(bob).unwrap();
    world.set::<Position>(bob, Position(69));

    let select_stmt = SelectStmt::new().write(pos);
    let with_stmt = WithStmt::new().with(test);
    let mut plan = QueryPlan::new(select_stmt, with_stmt);
    plan.init_tables(&world);

    let mut ctx = Context::new(&world);

    c.bench_function("test sparse", |b| {
        b.iter(|| {
            let view = plan.next_table(&mut ctx);
            assert!(view.is_some());
        });
    });
}

criterion_group!(benches, bench_sparse_set);
criterion_main!(benches);
