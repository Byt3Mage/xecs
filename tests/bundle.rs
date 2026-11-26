use std::error::Error;

use xecs::{Ecs, bundle, schema};

schema! {
    pub component Health: f32;
    pub component Position: (u32, u32);
    pub component Velocity: (u32, u32);
    pub component Acceleration: (u32, u32);
}

#[test]
fn test_types() -> Result<(), Box<dyn Error>> {
    let mut ecs = Ecs::new();
    let bob = ecs.create(bundle!(Health: 5.5, Position: (5, 6)))?;

    let h = ecs.get::<Health>(bob)?;

    println!("bob's position is: {h:?}");

    Ok(())
}
