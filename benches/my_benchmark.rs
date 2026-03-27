use criterion::{Criterion, criterion_group, criterion_main};
use std::marker::PhantomData;
use xecs::{
    component::Component,
    query::{Context, QueryPlan, SelectStmt, WithStmt},
    type_traits::Data,
};
pub use xecs::{
    component::{ComponentBuilder, TagBuilder},
    storage::StorageType,
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
struct Generic<T: Component<DataType = Data>>(PhantomData<T>);

macro_rules! register {
    ($world: expr, $type: path, $storage: tt) => {{
        $world.register::<$type>(
            <$type as xecs::component::Component>::DescType::new()
                .storage(xecs::storage::StorageType::$storage),
        )
    }};
}

fn bench_sparse_set(c: &mut Criterion) {
    let mut world = World::new();
    let test = register!(world, Test, Sparse);
    let likes = world.register::<Likes>(TagBuilder::new().storage(StorageType::Tables));
    let pos = world.register::<Position>(ComponentBuilder::new().storage(StorageType::Tables));
    let vel = world.register::<Velocity>(ComponentBuilder::new().storage(StorageType::Tables));
    let my_enum = world.register::<MyEnum>(ComponentBuilder::new().storage(StorageType::Tables));

    let bob = world.new_entity();

    world.add::<Test>(bob).unwrap();
    world.set::<(Likes, Position)>(bob, Position(69));

    let res = world.get::<&mut (Velocity, Position)>(bob);
    let id = world.id::<(Velocity, Likes)>();

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
