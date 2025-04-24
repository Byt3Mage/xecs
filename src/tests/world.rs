use crate::{error::EcsResult, view, world::World};

struct MyStruct {
    x: usize,
}

impl Drop for MyStruct {
    fn drop(&mut self) {
        println!("dropping MyStruct with value: {}", self.x);
    }
}

#[test]
fn world_init() -> EcsResult<()> {
    Ok(())
}
