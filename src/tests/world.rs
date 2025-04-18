use crate::world::World;

struct MyStruct { x: usize }

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.x);
    }
}

macro_rules! entity {
    ($world: expr) => {
        { let id = $world.new_entity(); $crate::entity_view::EntityView::new(&mut $world, id) }
    };
}

#[test]
fn world_init() {
    let mut world = World::new();

    let id = match world.component_t::<MyStruct>() {
        Ok(b) => b.build(&mut world),
        Err(id) => id,
    };

    let mut bob = entity!(world);
    bob.set_t(MyStruct {x: 69}).unwrap();

    let bob  = bob.id();

    let mut alice = entity!(world);
    alice.set_t(MyStruct {x: 42}).unwrap();
    alice.add(bob).unwrap();
}