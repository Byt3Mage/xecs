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
    
    let start  = std::time::Instant::now();

    let bob_rec = world.entity_index.get_location(bob);
    let alice_rec = world.entity_index.get_location(alice);

    let end = start.elapsed();

    println!("{}", bob_rec.unwrap().0);
    println!("{}", alice_rec.unwrap().0);

    println!("get record took {:?}", end);
    Ok(())
}