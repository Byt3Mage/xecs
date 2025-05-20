use crate::{component::ComponentDesc, error::EcsResult, storage::StorageType, world::World};

struct MyStruct {
    x: bool,
}

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.x);
    }
}

struct Position(u8);

#[test]
fn world_init() -> EcsResult<()> {
    let mut world = World::new();
    let pos = world.register::<u32>(ComponentDesc::new().storage(StorageType::Sparse));
    let bob = world.new_id();

    Ok(())
}
