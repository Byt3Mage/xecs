use crate::{
    component::{Component, ComponentDesc, TagDesc},
    error::EcsResult,
    storage::StorageType,
    world::World,
};

struct MyStruct(bool);

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.0);
    }
}

struct Position(u8);

impl Component for MyStruct {}
impl Component for Position {}

#[test]
fn world_init() -> EcsResult<()> {
    let mut world = World::new();
    let pos = world.register::<Position>(ComponentDesc::new().storage(StorageType::Tables));
    let likes = world.new_tag(TagDesc::new().storage(StorageType::Sparse));
    let alice = world.new_id();
    let bob = world.new_id();

    unsafe { world.set_id(bob, (pos, likes), Position(125)).unwrap() };

    Ok(())
}
