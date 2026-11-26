use std::error::Error;

use xecs::{Ecs, bundle, schema};

schema! {
    component Name: &'static str;


}

#[test]
fn relation_add_get_remove() -> Result<(), Box<dyn Error>> {
    let mut ecs = Ecs::new();

    let bob = ecs.create(bundle!(Name: "bob"))?;
    let helmet = ecs.create(bundle!(Name: "dragon helmet"))?;
    let gauntlets = ecs.create(bundle!(Name: "iron gauntlets"))?;
    let boots = ecs.create(bundle!(Name: "leather boots"))?;

    let name = ecs.get::<Name>(bob)?;

    Ok(())
}
