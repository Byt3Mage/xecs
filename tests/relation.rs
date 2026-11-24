use xecs::{Acyclic, Ecs, Fixed, Many, Slot, bundle, schema};

schema! {
    component Name: &'static str;

    relation Item {
        targets: Fixed<3>,
        sources: Many,
        cycles: Acyclic,
        data: u32,
    }
}

const HEAD: Slot<3> = Slot::at::<0>();
const ARMS: Slot<3> = Slot::at::<1>();
const LEGS: Slot<3> = Slot::at::<2>();

#[test]
fn relation_add_get_remove() -> anyhow::Result<()> {
    let mut ecs = Ecs::new();

    let bob = ecs.create(bundle!(Name: "bob"))?;
    let helmet = ecs.create(bundle!(Name: "dragon helmet"))?;
    let gauntlets = ecs.create(bundle!(Name: "iron gauntlets"))?;
    let boots = ecs.create(bundle!(Name: "leather boots"))?;

    ecs.relate_at::<Item>(bob, helmet, HEAD, 100)?;
    ecs.relate_at::<Item>(bob, gauntlets, ARMS, 20)?;
    ecs.relate_at::<Item>(bob, boots, LEGS, 50)?;

    let name = ecs.get::<Name>(bob)?;

    ecs.targets::<Item>(bob, |id, rating| {
        if let Ok(item) = ecs.get::<Name>(id) {
            println!("{name} has item {item} with rating {rating}")
        }
    })?;

    Ok(())
}
