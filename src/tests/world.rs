use crate::world::World;

struct MyStruct { x: usize }

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.x);
    }
}

#[test]
fn world_init() {
    let mut world = World::new();

    let _ = match world.component_t::<MyStruct>() {
        Ok(view) => view.id(),
        Err(b) => b.build(&mut world).id(),
    };

    let mut bob = world.new_entity();
    bob.set_t(MyStruct {x: 69}).unwrap();

    let bob  = bob.id();

    let mut alice = world.new_entity();
    alice.set_t(MyStruct {x: 42}).unwrap();
    alice.add(bob).unwrap();
}