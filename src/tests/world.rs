use crate::{error::EcsResult, view, world::World};

struct MyStruct { x: usize }

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.x);
    }
}

#[test]
fn world_init() -> EcsResult<()>{
    let mut world = World::new();

    match world.component_t::<MyStruct>() {
        Ok(b) => b.build(&mut world),
        Err(id) => id,
    };

    let bob = world.new_entity();
    let alice = world.new_entity();

    let alice = view!{
        @from(world, alice)
        .add(bob)
        .set_t(MyStruct {x: 42})?
    }?;

    let alice = view!{
        @use(alice)
        .set_t(MyStruct{x: 69}) | "unable to set MyStruct"
    };

    Ok(())
}